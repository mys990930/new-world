use bevy_ecs::prelude::Resource;

use super::camera::{
    quarter_view_basis, quarter_view_eye, QUARTER_VIEW_VERTICAL_WORLD_SIZE,
};
use super::input::EcsInputSnapshot;
use super::player::Transform;
use crate::world::{BlockFace, Ray3, WorldBlockCoord, WorldCore};

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct SelectionState {
    pub hovered_block: Option<WorldBlockCoord>,
    pub hovered_face: Option<BlockFace>,
    pub hit_point: Option<[f32; 3]>,
}

impl SelectionState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

pub fn update_selection_from_world(
    selection: &mut SelectionState,
    world: &WorldCore,
    input: &EcsInputSnapshot,
    camera_quarter_turns: u8,
    player_transform: Option<Transform>,
    viewport_width: u32,
    viewport_height: u32,
) {
    if !input.active
        || !input.focused
        || viewport_width == 0
        || viewport_height == 0
        || player_transform.is_none()
    {
        selection.clear();
        return;
    }

    let cursor = input.cursor_screen_pos;
    if cursor.0 < 0.0
        || cursor.1 < 0.0
        || cursor.0 > viewport_width as f64
        || cursor.1 > viewport_height as f64
    {
        selection.clear();
        return;
    }

    let target = player_transform
        .map(|transform| transform.translation)
        .unwrap_or([0.0, 0.0, 0.0]);
    let basis = quarter_view_basis(camera_quarter_turns);
    let eye = quarter_view_eye(target, camera_quarter_turns);
    let aspect = viewport_width as f32 / viewport_height as f32;
    let half_height = QUARTER_VIEW_VERTICAL_WORLD_SIZE * 0.5;
    let half_width = half_height * aspect;
    let ndc_x = (cursor.0 as f32 / viewport_width as f32) * 2.0 - 1.0;
    let ndc_y = 1.0 - (cursor.1 as f32 / viewport_height as f32) * 2.0;
    let origin = add3(
        eye,
        add3(
            scale3(basis.right, ndc_x * half_width),
            scale3(basis.up, ndc_y * half_height),
        ),
    );

    let hit = world.raycast_blocks(
        Ray3 {
            origin,
            direction: basis.forward,
        },
        1024.0,
    );

    match hit {
        Some(hit) => {
            selection.hovered_block = Some(hit.block);
            selection.hovered_face = Some(hit.face);
            selection.hit_point = Some(hit.point);
        }
        None => selection.clear(),
    }
}

fn add3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn scale3(vector: [f32; 3], scalar: f32) -> [f32; 3] {
    [vector[0] * scalar, vector[1] * scalar, vector[2] * scalar]
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::world::{BlockRegistry, ChunkCoord, ChunkData, LocalBlockCoord, WorldMeta};

    #[test]
    fn center_cursor_hits_top_face_of_block_under_player() {
        let registry = Arc::new(BlockRegistry::load_default().expect("default registry should load"));
        let mut world = WorldCore::new(WorldMeta::default(), registry);
        let mut chunk = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        chunk.set_block(LocalBlockCoord::new(2, 0, 3).unwrap(), crate::world::BlockId::GRASS)
            .unwrap();
        world.insert_chunk(ChunkCoord(0, 0, 0), chunk);

        let mut selection = SelectionState::default();
        update_selection_from_world(
            &mut selection,
            &world,
            &EcsInputSnapshot {
                cursor_screen_pos: (400.0, 300.0),
                focused: true,
                active: true,
                ..EcsInputSnapshot::default()
            },
            0,
            Some(Transform {
                translation: [3.0, 1.5, 3.0],
            }),
            800,
            600,
        );

        assert_eq!(selection.hovered_block, Some(WorldBlockCoord(2, 0, 3)));
        assert_eq!(selection.hovered_face, Some(BlockFace::PosY));
    }
}
