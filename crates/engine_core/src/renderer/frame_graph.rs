// crates/engine_core/src/renderer/frame_graph.rs

use std::collections::HashMap;

use engine_ecs::World;

use crate::renderer::context::GraphicsContext;
use crate::renderer::sprite_pass::SpritePass;

/// Logical resource identifier for this frame.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ResourceId(pub u32);

/// Simple resource kind classification (can be extended later).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResourceKind {
    Color,
    Depth,
}

/// Per-frame logical resource description.
#[derive(Copy, Clone, Debug)]
pub struct ResourceDesc {
    pub id: ResourceId,
    pub name: &'static str,
    pub kind: ResourceKind,

    /// Optional alias group. Resources that share the same non-None group
    /// are *allowed* to alias, but only if their lifetimes do not overlap.
    /// This is the foundation for transient texture aliasing.
    pub alias_group: Option<u32>,
}

/// Description of a single render pass in the DAG.
#[derive(Clone, Debug)]
pub struct PassDesc {
    pub name: &'static str,
    /// Resources read by this pass.
    pub reads: &'static [ResourceId],
    /// Resources written by this pass.
    pub writes: &'static [ResourceId],
}

/// Static description of the frame graph for this frame.
/// For now:
///   - One color target (backbuffer)
///   - Sprite pass (writes backbuffer)
///   - GUI pass   (reads + writes backbuffer)
#[derive(Clone, Debug)]
pub struct FrameGraphDesc {
    pub resources: &'static [ResourceDesc],
    pub passes: &'static [PassDesc],
}

/// Inputs that the frame graph needs for one frame.
pub struct FrameInputs<'a> {
    pub world: &'a World,
    pub gui: Option<(
        &'a egui::Context,
        &'a Vec<egui::ClippedPrimitive>,
        &'a egui::TexturesDelta,
    )>,
}

/// Outputs for one frame. Empty for now, but we keep this
/// struct so we can add timing/profiling/attachments later.
pub struct FrameOutputs;

/// Minimal frame graph wrapper for your current passes.
/// Internally uses a small DAG-style description (resources + passes)
/// with validation hooks for alias groups and lifetime checking.
pub struct FrameGraph<'a> {
    pub ctx: &'a GraphicsContext,
}

/// Logical resource IDs used by the current graph.
mod ids {
    use super::ResourceId;

    /// Logical color target for the frame. Currently bound to the surface
    /// backbuffer, but later this could be an off-screen texture.
    pub const BACKBUFFER: ResourceId = ResourceId(0);
}

/// Static frame graph description for the current pipeline.
///
/// NOTE: All of this is per-frame, but the *topology* is static.
/// You can later extend this with more resources + passes without
/// changing the outer API.
fn frame_graph_desc() -> FrameGraphDesc {
    use ids::*;

    const RESOURCES: &[ResourceDesc] = &[ResourceDesc {
        id: BACKBUFFER,
        name: "Backbuffer",
        kind: ResourceKind::Color,
        alias_group: None, // Could be Some(0) when we introduce aliasable temps.
    }];

    const PASSES: &[PassDesc] = &[
        PassDesc {
            name: "SpritePass",
            reads: &[],
            writes: &[BACKBUFFER],
        },
        PassDesc {
            name: "GuiPass",
            reads: &[BACKBUFFER],
            writes: &[BACKBUFFER],
        },
    ];

    FrameGraphDesc {
        resources: RESOURCES,
        passes: PASSES,
    }
}

impl<'a> FrameGraph<'a> {
    pub fn run(
        &self,
        sprite_pass: &mut SpritePass,
        gui_renderer: &mut egui_wgpu::Renderer,
        inputs: FrameInputs<'a>,
    ) -> Result<FrameOutputs, wgpu::SurfaceError> {
        // Build the logical graph description for this frame.
        let desc = frame_graph_desc();

        // Validate the graph before we touch the GPU.
        // Only in debug builds (no overhead in release).
        if cfg!(debug_assertions) {
            self.validate_graph(&desc);
        }

        // Acquire the backbuffer (physical resource backing our logical BACKBUFFER).
        // Any surface loss / timeout / etc. is reported via SurfaceError
        // and handled at the App level.
        let output = self.ctx.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Single command encoder for the frame
        let mut encoder =
            self.ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("FrameGraph Encoder"),
                });

        // Execute passes in the order given by desc.passes.
        // Later we can reorder based on dependencies; for now the ordering
        // matches the logical DAG topology (Sprite -> Gui).
        for (pass_index, pass) in desc.passes.iter().enumerate() {
            match pass.name {
                "SpritePass" => {
                    encoder.push_debug_group("SpritePass");
                    sprite_pass.draw(self.ctx, &mut encoder, &view, inputs.world);
                    encoder.pop_debug_group();
                }
                "GuiPass" => {
                    if let Some((ctx, primitives, delta)) = inputs.gui {
                        encoder.push_debug_group("GuiPass");

                        // Upload textures set this frame
                        for (id, image_delta) in &delta.set {
                            gui_renderer.update_texture(
                                &self.ctx.device,
                                &self.ctx.queue,
                                *id,
                                image_delta,
                            );
                        }

                        let screen_descriptor = egui_wgpu::ScreenDescriptor {
                            size_in_pixels: [self.ctx.config.width, self.ctx.config.height],
                            pixels_per_point: ctx.pixels_per_point(),
                        };

                        gui_renderer.update_buffers(
                            &self.ctx.device,
                            &self.ctx.queue,
                            &mut encoder,
                            primitives,
                            &screen_descriptor,
                        );

                        {
                            let mut gui_pass =
                                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                    label: Some("Gui Render Pass"),
                                    color_attachments: &[Some(
                                        wgpu::RenderPassColorAttachment {
                                            view: &view,
                                            resolve_target: None,
                                            ops: wgpu::Operations {
                                                // Load the result of the sprite pass
                                                load: wgpu::LoadOp::Load,
                                                store: wgpu::StoreOp::Store,
                                            },
                                        },
                                    )],
                                    depth_stencil_attachment: None,
                                    timestamp_writes: None,
                                    occlusion_query_set: None,
                                });

                            gui_renderer.render(&mut gui_pass, primitives, &screen_descriptor);
                        }

                        // Free any textures that egui asked us to drop
                        for id in &delta.free {
                            gui_renderer.free_texture(id);
                        }

                        encoder.pop_debug_group();
                    } else {
                        // GUI pass has no work this frame. This is allowed.
                        // The graph topology remains valid; we simply skip execution.
                    }
                }
                other => {
                    // For future extension: if we add new passes to the graph
                    // but forget to handle them here, fail loudly.
                    panic!(
                        "FrameGraph: pass index {} named '{}' has no execution handler",
                        pass_index, other
                    );
                }
            }
        }

        // Submit work and present
        self.ctx.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(FrameOutputs)
    }

    /// Validate the logical DAG before executing:
    ///
    /// - Ensure resource IDs referenced by passes exist.
    /// - Compute simple lifetimes (first/last pass index per resource).
    /// - Enforce alias-group safety (if/when alias_group is used).
    fn validate_graph(&self, desc: &FrameGraphDesc) {
        // Map ResourceId -> index in desc.resources
        let mut resource_index: HashMap<ResourceId, usize> = HashMap::new();
        for (idx, r) in desc.resources.iter().enumerate() {
            if resource_index.insert(r.id, idx).is_some() {
                panic!(
                    "FrameGraph validation error: duplicate ResourceId({:?}) for '{}'",
                    r.id, r.name
                );
            }
        }

        // Track lifetimes: for each resource, first and last pass index
        #[derive(Clone, Copy, Debug)]
        struct Lifetime {
            first: usize,
            last: usize,
        }

        let mut lifetimes: HashMap<ResourceId, Lifetime> = HashMap::new();

        for (pass_idx, pass) in desc.passes.iter().enumerate() {
            for &rid in pass.reads.iter().chain(pass.writes.iter()) {
                let _ = resource_index.get(&rid).unwrap_or_else(|| {
                    panic!(
                        "FrameGraph validation error: pass '{}' references unknown resource {:?}",
                        pass.name, rid
                    )
                });

                lifetimes
                    .entry(rid)
                    .and_modify(|lt| {
                        lt.last = pass_idx.max(lt.last);
                    })
                    .or_insert(Lifetime {
                        first: pass_idx,
                        last: pass_idx,
                    });
            }
        }

        // Alias-group validation: resources in the same alias_group must not
        // have overlapping lifetimes. This is the foundation for transient
        // texture aliasing. We enforce it even if we are not yet creating
        // separate physical allocations.
        let mut group_members: HashMap<u32, Vec<(ResourceId, Lifetime, &'static str)>> =
            HashMap::new();

        for r in desc.resources {
            if let Some(group) = r.alias_group {
                if let Some(lt) = lifetimes.get(&r.id) {
                    group_members
                        .entry(group)
                        .or_default()
                        .push((r.id, *lt, r.name));
                }
            }
        }

        for (group, members) in group_members {
            // Check all pairs for lifetime overlap
            for i in 0..members.len() {
                for j in (i + 1)..members.len() {
                    let (id_a, lt_a, name_a) = members[i];
                    let (id_b, lt_b, name_b) = members[j];

                    let overlaps =
                        lt_a.first <= lt_b.last && lt_b.first <= lt_a.last;

                    if overlaps {
                        panic!(
                            "FrameGraph aliasing violation in group {}: \
                             resources {:?} ('{}') and {:?} ('{}') have overlapping lifetimes \
                             ({}..={} vs {}..={}). They cannot safely alias the same memory.",
                            group,
                            id_a,
                            name_a,
                            id_b,
                            name_b,
                            lt_a.first,
                            lt_a.last,
                            lt_b.first,
                            lt_b.last
                        );
                    }
                }
            }
        }

        // If we get here, the logical graph is structurally sound for this frame.
    }
}
