use bevy_ecs::prelude::World;

use super::player::{LocalPlayerEntity, PlayerPhysicsState, Transform, Velocity};

pub const VOXEL_PLAYER_PART_COUNT: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelPlayerAnimationState {
    Idle,
    Walk,
    Sprint,
    Airborne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelPlayerFacingOctant(pub u8);

impl VoxelPlayerFacingOctant {
    pub const NORTH: Self = Self(0);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelPlayerVisualState {
    pub root_translation: [f32; 3],
    pub facing: VoxelPlayerFacingOctant,
    pub animation_state: VoxelPlayerAnimationState,
    pub horizontal_speed: f32,
    pub grounded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelPlayerPart {
    Head,
    Torso,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelPlayerPartPose {
    pub part: VoxelPlayerPart,
    pub local_center: [f32; 3],
    pub half_extents: [f32; 3],
    pub color: [f32; 4],
}

pub fn local_player_visual_state(world: &World) -> Option<VoxelPlayerVisualState> {
    let entity = world.resource::<LocalPlayerEntity>().0?;
    let transform = world.get::<Transform>(entity)?;
    let velocity = world.get::<Velocity>(entity)?;
    let physics = world.get::<PlayerPhysicsState>(entity)?;
    let horizontal_speed =
        (velocity.linear[0] * velocity.linear[0] + velocity.linear[2] * velocity.linear[2]).sqrt();

    Some(VoxelPlayerVisualState {
        root_translation: transform.translation,
        facing: facing_octant_from_velocity(velocity.linear),
        animation_state: animation_state_from_motion(horizontal_speed, physics.grounded),
        horizontal_speed,
        grounded: physics.grounded,
    })
}

pub fn default_voxel_player_part_poses() -> [VoxelPlayerPartPose; VOXEL_PLAYER_PART_COUNT] {
    [
        VoxelPlayerPartPose {
            part: VoxelPlayerPart::Head,
            local_center: [0.0, 1.25, 0.0],
            half_extents: [0.42, 0.42, 0.42],
            color: [0.94, 0.74, 0.52, 1.0],
        },
        VoxelPlayerPartPose {
            part: VoxelPlayerPart::Torso,
            local_center: [0.0, 0.35, 0.0],
            half_extents: [0.46, 0.55, 0.28],
            color: [0.18, 0.42, 0.82, 1.0],
        },
        VoxelPlayerPartPose {
            part: VoxelPlayerPart::LeftArm,
            local_center: [-0.62, 0.30, 0.0],
            half_extents: [0.16, 0.52, 0.18],
            color: [0.94, 0.74, 0.52, 1.0],
        },
        VoxelPlayerPartPose {
            part: VoxelPlayerPart::RightArm,
            local_center: [0.62, 0.30, 0.0],
            half_extents: [0.16, 0.52, 0.18],
            color: [0.94, 0.74, 0.52, 1.0],
        },
        VoxelPlayerPartPose {
            part: VoxelPlayerPart::LeftLeg,
            local_center: [-0.22, -0.70, 0.0],
            half_extents: [0.18, 0.55, 0.20],
            color: [0.12, 0.15, 0.22, 1.0],
        },
        VoxelPlayerPartPose {
            part: VoxelPlayerPart::RightLeg,
            local_center: [0.22, -0.70, 0.0],
            half_extents: [0.18, 0.55, 0.20],
            color: [0.12, 0.15, 0.22, 1.0],
        },
    ]
}

fn animation_state_from_motion(horizontal_speed: f32, grounded: bool) -> VoxelPlayerAnimationState {
    if !grounded {
        return VoxelPlayerAnimationState::Airborne;
    }
    if horizontal_speed <= 0.01 {
        return VoxelPlayerAnimationState::Idle;
    }
    if horizontal_speed >= 0.95 {
        return VoxelPlayerAnimationState::Sprint;
    }
    VoxelPlayerAnimationState::Walk
}

fn facing_octant_from_velocity(velocity: [f32; 3]) -> VoxelPlayerFacingOctant {
    let x = velocity[0];
    let z = velocity[2];
    if x * x + z * z <= 0.0001 {
        return VoxelPlayerFacingOctant::NORTH;
    }

    let angle = z.atan2(x);
    let octant = ((angle / std::f32::consts::FRAC_PI_4).round() as i32).rem_euclid(8);
    VoxelPlayerFacingOctant(octant as u8)
}

#[cfg(test)]
mod tests {
    use bevy_ecs::prelude::World;

    use super::*;
    use crate::ecs::Player;

    #[test]
    fn default_rig_has_expected_part_count() {
        assert_eq!(
            default_voxel_player_part_poses().len(),
            VOXEL_PLAYER_PART_COUNT
        );
    }

    #[test]
    fn visual_state_classifies_airborne_before_speed() {
        assert_eq!(
            animation_state_from_motion(1.0, false),
            VoxelPlayerAnimationState::Airborne
        );
    }

    #[test]
    fn local_player_visual_state_reads_current_player_snapshot() {
        let mut world = World::new();
        world.insert_resource(LocalPlayerEntity::default());
        let entity = world
            .spawn((
                Player,
                Transform {
                    translation: [2.0, 3.0, 4.0],
                },
                Velocity {
                    linear: [1.0, 0.0, 0.0],
                },
                PlayerPhysicsState { grounded: true },
            ))
            .id();
        world.resource_mut::<LocalPlayerEntity>().0 = Some(entity);

        let visual = local_player_visual_state(&world).expect("local player visual state");

        assert_eq!(visual.root_translation, [2.0, 3.0, 4.0]);
        assert_eq!(visual.animation_state, VoxelPlayerAnimationState::Sprint);
        assert_eq!(visual.horizontal_speed, 1.0);
    }
}
