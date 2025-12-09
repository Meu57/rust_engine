// crates/engine_core/src/engine_loop.rs

use std::time::Instant;

use crate::plugin_manager::PluginManager;
use engine_ecs::World;
use engine_shared::input_types::InputState;

/// Encapsulates fixed-timestep simulation bookkeeping (time, accumulator, limits).
/// Mirrors the original App::run behavior: accumulator, max steps, backlog drop.
pub struct EngineLoop {
    last_frame_time: Instant,
    sim_accumulator: f32,
    sim_dt: f32,
    max_steps_per_frame: u32,
}

impl EngineLoop {
    pub fn new(sim_dt: f32) -> Self {
        Self {
            last_frame_time: Instant::now(),
            sim_accumulator: 0.0,
            sim_dt,
            max_steps_per_frame: 5,
        }
    }

    /// Update the frame timer and return the clamped frame delta.
    /// Clamps to 0.25s to avoid giant spikes when dragging the window,
    /// hitting breakpoints, etc.
    pub fn tick_timer(&mut self) -> f32 {
        let now = Instant::now();
        let frame_dt = now
            .duration_since(self.last_frame_time)
            .as_secs_f32();
        self.last_frame_time = now;

        frame_dt.min(0.25)
    }

    /// Runs fixed-timestep simulation steps until the accumulator is caught up
    /// or we hit max_steps_per_frame. If the backlog still remains at the cap,
    /// we drop it, to avoid "chasing" an infinite backlog under heavy load.
    pub fn update_simulation(
        &mut self,
        frame_dt: f32,
        world: &mut World,
        plugin_manager: &mut PluginManager,
        input_state: &InputState,
    ) {
        self.sim_accumulator += frame_dt;

        let mut steps = 0;
        while self.sim_accumulator >= self.sim_dt && steps < self.max_steps_per_frame {
            plugin_manager.update(world, input_state, self.sim_dt);
            self.sim_accumulator -= self.sim_dt;
            steps += 1;
        }

        // Prevent unbounded backlog if we're constantly saturated.
        if steps == self.max_steps_per_frame && self.sim_accumulator >= self.sim_dt {
            self.sim_accumulator = 0.0;
        }
    }
}
