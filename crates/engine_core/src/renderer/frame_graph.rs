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

/// What kind of work a pass performs. We match on this instead of raw strings,
/// but keep the name field for debugging / logging.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PassKind {
    Sprite,
    SceneToBackbuffer,
    Gui,
}

/// Description of a single render pass in the DAG.
#[derive(Clone, Debug)]
pub struct PassDesc {
    pub name: &'static str,
    pub kind: PassKind,
    /// Resources read by this pass.
    pub reads: &'static [ResourceId],
    /// Resources written by this pass.
    pub writes: &'static [ResourceId],
}

/// Static description of the frame graph for this frame.
///
/// We now have:
///   - `SceneColor`  (off-screen color target)
///   - `Backbuffer`  (surface)
///   - Sprite pass            : writes `SceneColor`
///   - SceneToBackbuffer pass : reads  `SceneColor`, writes `Backbuffer`
///   - GUI pass               : reads + writes `Backbuffer`
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

    /// Off-screen scene color buffer (render target for SpritePass).
    pub const SCENE_COLOR: ResourceId = ResourceId(0);
    /// Final backbuffer (surface texture).
    pub const BACKBUFFER: ResourceId = ResourceId(1);
}

/// Static frame graph description for the current pipeline.
///
/// NOTE: All of this is per-frame, but the *topology* is static.
/// You can extend this with more resources + passes without
/// changing the outer API.
fn frame_graph_desc() -> FrameGraphDesc {
    use ids::*;

    // Start using alias_group for SCENE_COLOR. Right now it is the
    // only member of its group, but this sets the pattern for future
    // aliasable temporaries.
    const RESOURCES: &[ResourceDesc] = &[
        ResourceDesc {
            id: SCENE_COLOR,
            name: "SceneColor",
            kind: ResourceKind::Color,
            alias_group: Some(0),
        },
        ResourceDesc {
            id: BACKBUFFER,
            name: "Backbuffer",
            kind: ResourceKind::Color,
            alias_group: None, // surface is not aliasable in this design
        },
    ];

    const PASSES: &[PassDesc] = &[
        PassDesc {
            name: "SpritePass",
            kind: PassKind::Sprite,
            reads: &[],
            writes: &[SCENE_COLOR],
        },
        PassDesc {
            name: "SceneToBackbuffer",
            kind: PassKind::SceneToBackbuffer,
            reads: &[SCENE_COLOR],
            writes: &[BACKBUFFER],
        },
        PassDesc {
            name: "GuiPass",
            kind: PassKind::Gui,
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

        // Validate the graph before we touch the GPU (debug builds only).
        if cfg!(debug_assertions) {
            self.validate_graph(&desc);
        }

        // Acquire the backbuffer (physical resource backing our logical BACKBUFFER).
        let output = self.ctx.surface.get_current_texture()?;
        let backbuffer_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Off-screen SceneColor view (physical resource for SCENE_COLOR).
        let scene_view = &self.ctx.scene_color_view;

        // Single command encoder for the frame
        let mut encoder =
            self.ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("FrameGraph Encoder"),
                });

        // Execute passes in the order given by desc.passes.
        // (Ordering is currently manual but validated for basic data-flow issues.)
        for (pass_index, pass) in desc.passes.iter().enumerate() {
            match pass.kind {
                PassKind::Sprite => {
                    encoder.push_debug_group("SpritePass");
                    // SpritePass now renders into off-screen SceneColor
                    sprite_pass.draw(self.ctx, &mut encoder, scene_view, inputs.world);
                    encoder.pop_debug_group();
                }

                PassKind::SceneToBackbuffer => {
                    encoder.push_debug_group("SceneToBackbuffer");

                    // Full-texture copy: SceneColor → Backbuffer.
                    // This keeps the composite stage simple while giving us
                    // a true off-screen scene buffer.
                    let src = wgpu::ImageCopyTexture {
                        texture: &self.ctx.scene_color,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    };
                    let dst = wgpu::ImageCopyTexture {
                        texture: &output.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    };
                    let extent = wgpu::Extent3d {
                        width: self.ctx.config.width,
                        height: self.ctx.config.height,
                        depth_or_array_layers: 1,
                    };

                    encoder.copy_texture_to_texture(src, dst, extent);

                    encoder.pop_debug_group();
                }

                PassKind::Gui => {
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
                                            view: &backbuffer_view,
                                            resolve_target: None,
                                            ops: wgpu::Operations {
                                                // Load the result of SceneToBackbuffer copy
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
                        // GUI pass has no work this frame; topology is still valid.
                    }
                }

                // If we ever add a new PassKind but forget to handle it here,
                // this makes it obvious instead of silently doing nothing.
                other => {
                    panic!(
                        "FrameGraph: unhandled PassKind {:?} (pass index {}, name '{}')",
                        other, pass_index, pass.name
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
    /// - Ensure that any resource which is written and then read is not
    ///   read *before* its first write in the declared pass order.
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

        // Track first read and first write index per resource (by pass order).
        let mut first_read: HashMap<ResourceId, usize> = HashMap::new();
        let mut first_write: HashMap<ResourceId, usize> = HashMap::new();

        // Helper to update lifetime for any access (read or write).
        fn bump_lifetime(
            lifetimes: &mut HashMap<ResourceId, Lifetime>,
            rid: ResourceId,
            pass_idx: usize,
        ) {
            lifetimes
                .entry(rid)
                .and_modify(|lt| {
                    if pass_idx < lt.first {
                        lt.first = pass_idx;
                    }
                    if pass_idx > lt.last {
                        lt.last = pass_idx;
                    }
                })
                .or_insert(Lifetime {
                    first: pass_idx,
                    last: pass_idx,
                });
        }

        for (pass_idx, pass) in desc.passes.iter().enumerate() {
            // Reads
            for &rid in pass.reads {
                let _ = resource_index.get(&rid).unwrap_or_else(|| {
                    panic!(
                        "FrameGraph validation error: pass '{}' references unknown resource {:?} (read)",
                        pass.name, rid
                    )
                });

                bump_lifetime(&mut lifetimes, rid, pass_idx);

                first_read
                    .entry(rid)
                    .and_modify(|idx| {
                        if pass_idx < *idx {
                            *idx = pass_idx;
                        }
                    })
                    .or_insert(pass_idx);
            }

            // Writes
            for &rid in pass.writes {
                let _ = resource_index.get(&rid).unwrap_or_else(|| {
                    panic!(
                        "FrameGraph validation error: pass '{}' references unknown resource {:?} (write)",
                        pass.name, rid
                    )
                });

                bump_lifetime(&mut lifetimes, rid, pass_idx);

                first_write
                    .entry(rid)
                    .and_modify(|idx| {
                        if pass_idx < *idx {
                            *idx = pass_idx;
                        }
                    })
                    .or_insert(pass_idx);
            }
        }

        // Additional validation: any resource that is written and read in this graph
        // must not be read before its first write according to the declared pass order.
        for (&rid, &write_idx) in &first_write {
            if let Some(&read_idx) = first_read.get(&rid) {
                if read_idx < write_idx {
                    let r = &desc.resources[resource_index[&rid]];
                    panic!(
                        "FrameGraph validation error: resource {:?} ('{}') is first READ in pass index {} \
                         but first WRITE occurs later at pass index {}. \
                         Reorder your passes so writes happen before reads.",
                        rid, r.name, read_idx, write_idx
                    );
                }
            }
        }

        // Alias-group validation: resources in the same alias_group must not
        // have overlapping lifetimes. This is the foundation for transient
        // texture aliasing.
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
