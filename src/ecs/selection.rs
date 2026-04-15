use bevy_ecs::prelude::Resource;

use super::camera::{
    CameraState, quarter_view_camera_pose, quarter_view_vertical_world_size,
};
use super::input::EcsInputSnapshot;
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
    camera: CameraState,
    viewport_width: u32,
    viewport_height: u32,
) {
    if !input.active
        || !input.focused
        || viewport_width == 0
        || viewport_height == 0
        || !camera.initialized
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

    let pose = quarter_view_camera_pose(camera);
    let aspect = viewport_width as f32 / viewport_height as f32;
    let half_height = quarter_view_vertical_world_size(camera) * 0.5;
    let half_width = half_height * aspect;
    let ndc_x = (cursor.0 as f32 / viewport_width as f32) * 2.0 - 1.0;
    let ndc_y = 1.0 - (cursor.1 as f32 / viewport_height as f32) * 2.0;
    let focal_point = add3(
        pose.target,
        add3(
            scale3(pose.basis.right, ndc_x * half_width),
            scale3(pose.basis.up, ndc_y * half_height),
        ),
    );
    let Some(direction) = normalize3(subtract3(focal_point, pose.eye)) else {
        selection.clear();
        return;
    };

    let hit = world.raycast_blocks(
        Ray3 {
            origin: pose.eye,
            direction,
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

fn subtract3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale3(vector: [f32; 3], scalar: f32) -> [f32; 3] {
    [vector[0] * scalar, vector[1] * scalar, vector[2] * scalar]
}

fn normalize3(value: [f32; 3]) -> Option<[f32; 3]> {
    let length_sq = value[0] * value[0] + value[1] * value[1] + value[2] * value[2];
    if length_sq <= f32::EPSILON {
        return None;
    }

    let inv_length = length_sq.sqrt().recip();
    Some([
        value[0] * inv_length,
        value[1] * inv_length,
        value[2] * inv_length,
    ])
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
            CameraState {
                quarter_turns: 0,
                smoothed_target: [3.0, 1.5, 3.0],
                desired_target: [3.0, 1.5, 3.0],
                vertical_world_size: 20.0,
                desired_vertical_world_size: 20.0,
                recenter_requested: false,
                recentering: false,
                initialized: true,
                ..CameraState::default()
            },
            800,
            600,
        );

        assert_eq!(selection.hovered_block, Some(WorldBlockCoord(2, 0, 3)));
        assert_eq!(selection.hovered_face, Some(BlockFace::PosY));
    }
}
