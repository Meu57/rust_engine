// crates/game_plugin/src/state.rs

use engine_ecs::World;
use engine_shared::{
    CCamera, CPlayer, CSprite, CTransform, 
    input_types::{ActionId, ACTION_NOT_FOUND},
    plugin_api::HostInterface,
};
use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct MyGame {
    pub spawn_timer: f32,
    #[serde(default)]
    pub score: u32,
    #[serde(skip)]
    pub actions: [ActionId; 4],
    #[serde(skip)]
    pub spawn_fn: Option<extern "C" fn(*mut engine_shared::plugin_api::HostContext, f32, f32)>,
    // Track if we already set up the scene so we don't spawn duplicates on reload
    #[serde(skip)]
    pub scene_initialized: bool, 
}

impl Default for MyGame {
    fn default() -> Self {
        Self {
            spawn_timer: 2.0,
            score: 0,
            actions: [ACTION_NOT_FOUND; 4],
            spawn_fn: None,
            scene_initialized: false,
        }
    }
}

impl MyGame {
    pub fn bind_host_resources(&mut self, host: &HostInterface) {
        self.actions[0] = (host.get_action_id)(b"MoveUp".as_ptr(), b"MoveUp".len());
        self.actions[1] = (host.get_action_id)(b"MoveDown".as_ptr(), b"MoveDown".len());
        self.actions[2] = (host.get_action_id)(b"MoveLeft".as_ptr(), b"MoveLeft".len());
        self.actions[3] = (host.get_action_id)(b"MoveRight".as_ptr(), b"MoveRight".len());
        self.spawn_fn = Some(host.spawn_enemy);
    }
}

// ---------------------------------------------------------------------
// HELPER: Scene Setup
// ---------------------------------------------------------------------
pub fn setup_scene(world: &mut World) {
    // 1. Spawn Player
    let player = world.spawn();
    world.add_component(
        player,
        CTransform {
            pos: Vec2::new(400.0, 300.0), // Safe start position
            ..Default::default()
        },
    );
    world.add_component(player, CPlayer);
    world.add_component(player, CSprite::default());

    // 2. Spawn Camera
    let camera = world.spawn();
    world.add_component(camera, CTransform::default());
    world.add_component(camera, CCamera {
        zoom: 1.0,
        smoothness: 15.0, 
    });
}

// ---------------------------------------------------------------------
// SAFETY TESTS
// ---------------------------------------------------------------------
#[cfg(test)]
mod safety_tests {
    use super::*;
    use engine_shared::plugin_api::{CURRENT_SCHEMA_HASH, CURRENT_STATE_VERSION};

    #[test]
    fn test_layout_change_requires_version_ack() {
        let game = MyGame::default();
        let current_size =
            bincode::serialized_size(&game).expect("Serialization of MyGame must succeed");

        // NOTE: Size + bool (1 byte) + padding. 
        // 8 bytes (f32+u32) + 1 byte (bool) -> usually aligns to 12 bytes.
        // Update this constant if the test fails.
        const EXPECTED_SIZE: u64 = 13; 
        const EXPECTED_VERSION: u32 = 1;
        const EXPECTED_HASH: u64 = 0x0123_4567_89AB_CDEF;

        assert_eq!(
            current_size, EXPECTED_SIZE,
            "STRUCT LAYOUT CHANGED! MyGame serialized size is {}. Update EXPECTED_SIZE.",
            current_size
        );

        assert_eq!(
            CURRENT_STATE_VERSION, EXPECTED_VERSION,
            "VERSION MISMATCH! Bump version if needed."
        );

        assert_eq!(
            CURRENT_SCHEMA_HASH, EXPECTED_HASH,
            "HASH MISMATCH! Update hash if schema changed."
        );
    }
}