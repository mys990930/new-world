use crate::ecs::{PlayerBody, Transform};
use crate::renderer::RenderCameraState;
use crate::world::{WorldBlockCoord, WorldCore};

pub(crate) const MAX_PLAYER_OCCLUSION_BLOCKS: usize = 64;
const MAX_OCCLUSION_DISTANCE_BLOCKS: f32 = 96.0;
const PLAYER_OCCLUSION_SAMPLE_COUNT: usize = 5;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct OccludingBlockSet {
    blocks: Vec<WorldBlockCoord>,
}

impl OccludingBlockSet {
    pub(crate) fn blocks(&self) -> &[WorldBlockCoord] {
        &self.blocks
    }

    fn push_unique(&mut self, block: WorldBlockCoord) {
        if self.blocks.len() >= MAX_PLAYER_OCCLUSION_BLOCKS {
            return;
        }
        if !self.blocks.contains(&block) {
            self.blocks.push(block);
        }
    }
}

pub(crate) fn collect_player_occlusion_blocks(
    world: &WorldCore,
    player_transform: Option<Transform>,
    player_body: Option<PlayerBody>,
    camera: &RenderCameraState,
) -> OccludingBlockSet {
    let Some(transform) = player_transform else {
        return OccludingBlockSet::default();
    };
    let body = player_body.unwrap_or_default();
    let samples = player_occlusion_samples(transform, body);
    let mut occluding = OccludingBlockSet::default();

    for sample in samples {
        collect_solid_blocks_on_segment(world, sample, camera.eye, &mut occluding);
        if occluding.blocks.len() >= MAX_PLAYER_OCCLUSION_BLOCKS {
            break;
        }
    }

    occluding
}

fn player_occlusion_samples(
    transform: Transform,
    body: PlayerBody,
) -> [[f32; 3]; PLAYER_OCCLUSION_SAMPLE_COUNT] {
    let center = transform.translation;
    let shoulder_y = body.half_extents[1] * 0.42;
    let head_y = body.half_extents[1] * 0.76;
    let side_x = body.half_extents[0] * 0.42;
    let side_z = body.half_extents[2] * 0.42;

    [
        [center[0], center[1] + head_y, center[2]],
        [center[0], center[1] + shoulder_y, center[2]],
        [center[0] + side_x, center[1] + shoulder_y, center[2]],
        [center[0] - side_x, center[1] + shoulder_y, center[2]],
        [center[0], center[1] + shoulder_y, center[2] + side_z],
    ]
}

fn collect_solid_blocks_on_segment(
    world: &WorldCore,
    from: [f32; 3],
    to: [f32; 3],
    occluding: &mut OccludingBlockSet,
) {
    let delta = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let distance = length3(delta).min(MAX_OCCLUSION_DISTANCE_BLOCKS);
    if distance <= f32::EPSILON {
        return;
    }
    let direction = [
        delta[0] / distance,
        delta[1] / distance,
        delta[2] / distance,
    ];
    let mut block = WorldBlockCoord(
        from[0].floor() as i32,
        from[1].floor() as i32,
        from[2].floor() as i32,
    );
    let step_x = step_sign(direction[0]);
    let step_y = step_sign(direction[1]);
    let step_z = step_sign(direction[2]);

    let mut t_max_x = first_boundary_distance(from[0], direction[0], block.0, step_x);
    let mut t_max_y = first_boundary_distance(from[1], direction[1], block.1, step_y);
    let mut t_max_z = first_boundary_distance(from[2], direction[2], block.2, step_z);
    let t_delta_x = axis_delta(direction[0]);
    let t_delta_y = axis_delta(direction[1]);
    let t_delta_z = axis_delta(direction[2]);
    let mut traveled = 0.0_f32;
    let max_steps = (distance.ceil() as usize).saturating_mul(6).max(1);

    for _ in 0..max_steps {
        if traveled > distance || occluding.blocks.len() >= MAX_PLAYER_OCCLUSION_BLOCKS {
            break;
        }
        if world
            .get_block(block)
            .is_some_and(|block_id| world.block_registry().is_solid(block_id))
        {
            occluding.push_unique(block);
        }

        if t_max_x <= t_max_y && t_max_x <= t_max_z {
            traveled = t_max_x;
            block.0 += step_x;
            t_max_x += t_delta_x;
        } else if t_max_y <= t_max_z {
            traveled = t_max_y;
            block.1 += step_y;
            t_max_y += t_delta_y;
        } else {
            traveled = t_max_z;
            block.2 += step_z;
            t_max_z += t_delta_z;
        }
    }
}

fn length3(vector: [f32; 3]) -> f32 {
    (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt()
}

fn step_sign(axis: f32) -> i32 {
    if axis < 0.0 { -1 } else { 1 }
}

fn axis_delta(axis: f32) -> f32 {
    if axis.abs() <= f32::EPSILON {
        f32::INFINITY
    } else {
        axis.abs().recip()
    }
}

fn first_boundary_distance(
    origin_axis: f32,
    direction_axis: f32,
    block_axis: i32,
    step: i32,
) -> f32 {
    if direction_axis.abs() <= f32::EPSILON {
        return f32::INFINITY;
    }

    let boundary = if step >= 0 {
        block_axis as f32 + 1.0
    } else {
        block_axis as f32
    };
    ((boundary - origin_axis) / direction_axis).max(0.0)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::world::{BlockId, BlockRegistry, ChunkCoord, ChunkData, LocalBlockCoord, WorldMeta};

    #[test]
    fn occlusion_collection_finds_solid_between_player_and_camera() {
        let mut world = test_world();
        let mut chunk = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        chunk
            .set_block(LocalBlockCoord::new(4, 3, 4).unwrap(), BlockId::STONE)
            .unwrap();
        world.insert_chunk(ChunkCoord(0, 0, 0), chunk);

        let blocks = collect_player_occlusion_blocks(
            &world,
            Some(Transform {
                translation: [2.5, 2.0, 2.5],
            }),
            Some(PlayerBody::default()),
            &RenderCameraState {
                eye: [8.0, 5.0, 8.0],
                target: [2.5, 2.0, 2.5],
                ..RenderCameraState::default()
            },
        );

        assert!(blocks.blocks().contains(&WorldBlockCoord(4, 3, 4)));
    }

    #[test]
    fn occlusion_collection_ignores_missing_chunks() {
        let world = test_world();
        let blocks = collect_player_occlusion_blocks(
            &world,
            Some(Transform {
                translation: [2.5, 2.0, 2.5],
            }),
            Some(PlayerBody::default()),
            &RenderCameraState {
                eye: [8.0, 5.0, 8.0],
                target: [2.5, 2.0, 2.5],
                ..RenderCameraState::default()
            },
        );

        assert!(blocks.blocks().is_empty());
    }

    fn test_world() -> WorldCore {
        WorldCore::new(
            WorldMeta::default(),
            Arc::new(BlockRegistry::load_default().expect("default block registry should load")),
        )
    }
}
