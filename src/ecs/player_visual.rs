use bevy_ecs::prelude::{Res, ResMut, Resource, World};

use super::{
    input::EcsInputSnapshot,
    player::{
        FrameDeltaSeconds, LocalPlayerEntity, PlayerMovementConfig, PlayerPhysicsState, Transform,
        Velocity,
    },
};

pub const VOXEL_PLAYER_PART_COUNT: usize = 6;
pub const VOXEL_PLAYER_IDLE_SPEED_EPSILON: f32 = 0.01;
pub const VOXEL_PLAYER_SKIN_COLOR: [f32; 4] = [0.94, 0.74, 0.52, 1.0];
pub const VOXEL_PLAYER_SHIRT_COLOR: [f32; 4] = [0.18, 0.42, 0.82, 1.0];
pub const VOXEL_PLAYER_PANTS_COLOR: [f32; 4] = [0.12, 0.15, 0.22, 1.0];
pub const VOXEL_PLAYER_TOOL_SWING_SECONDS: f32 = 0.5;
pub const VOXEL_PLAYER_VISUAL_FOOT_OFFSET_Y: f32 = -1.5;

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
    pub const EAST: Self = Self(2);
    pub const SOUTH: Self = Self(4);
    pub const WEST: Self = Self(6);
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct VoxelPlayerAnimationClock {
    pub seconds: f32,
}

impl Default for VoxelPlayerAnimationClock {
    fn default() -> Self {
        Self { seconds: 0.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelPlayerVisualState {
    pub root_translation: [f32; 3],
    pub facing: VoxelPlayerFacingOctant,
    pub animation_state: VoxelPlayerAnimationState,
    pub horizontal_speed: f32,
    pub grounded: bool,
    pub animation_seconds: f32,
    pub tool_swing_fraction: f32,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct VoxelPlayerToolSwingState {
    pub elapsed_seconds: f32,
    pub duration_seconds: f32,
    pub active: bool,
}

impl Default for VoxelPlayerToolSwingState {
    fn default() -> Self {
        Self {
            elapsed_seconds: 0.0,
            duration_seconds: VOXEL_PLAYER_TOOL_SWING_SECONDS,
            active: false,
        }
    }
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
    let input = world
        .get_resource::<EcsInputSnapshot>()
        .cloned()
        .unwrap_or_default();
    let movement = world
        .get_resource::<PlayerMovementConfig>()
        .copied()
        .unwrap_or_default();
    let animation_clock = world
        .get_resource::<VoxelPlayerAnimationClock>()
        .copied()
        .unwrap_or_default();
    let tool_swing = world
        .get_resource::<VoxelPlayerToolSwingState>()
        .copied()
        .unwrap_or_default();
    let intent_strength =
        (velocity.linear[0] * velocity.linear[0] + velocity.linear[2] * velocity.linear[2]).sqrt();
    let horizontal_speed = if intent_strength <= VOXEL_PLAYER_IDLE_SPEED_EPSILON {
        0.0
    } else if input.sprint_down {
        movement.sprint_units_per_second
    } else {
        movement.walk_units_per_second
    };

    Some(VoxelPlayerVisualState {
        root_translation: transform.translation,
        facing: facing_octant_from_velocity(velocity.linear),
        animation_state: animation_state_from_motion(
            intent_strength,
            input.sprint_down,
            physics.grounded,
        ),
        horizontal_speed,
        grounded: physics.grounded,
        animation_seconds: animation_clock.seconds,
        tool_swing_fraction: tool_swing.fraction(),
    })
}

pub fn default_voxel_player_part_poses() -> [VoxelPlayerPartPose; VOXEL_PLAYER_PART_COUNT] {
    [
        VoxelPlayerPartPose {
            part: VoxelPlayerPart::Head,
            local_center: [0.0, 1.0, 0.0],
            half_extents: [0.42, 0.42, 0.42],
            color: VOXEL_PLAYER_SKIN_COLOR,
        },
        VoxelPlayerPartPose {
            part: VoxelPlayerPart::Torso,
            local_center: [0.0, 0.10, 0.0],
            half_extents: [0.46, 0.55, 0.28],
            color: VOXEL_PLAYER_SHIRT_COLOR,
        },
        VoxelPlayerPartPose {
            part: VoxelPlayerPart::LeftArm,
            local_center: [-0.62, 0.05, 0.0],
            half_extents: [0.16, 0.52, 0.18],
            color: VOXEL_PLAYER_SKIN_COLOR,
        },
        VoxelPlayerPartPose {
            part: VoxelPlayerPart::RightArm,
            local_center: [0.62, 0.05, 0.0],
            half_extents: [0.16, 0.52, 0.18],
            color: VOXEL_PLAYER_SKIN_COLOR,
        },
        VoxelPlayerPartPose {
            part: VoxelPlayerPart::LeftLeg,
            local_center: [-0.22, VOXEL_PLAYER_VISUAL_FOOT_OFFSET_Y + 0.55, 0.0],
            half_extents: [0.18, 0.55, 0.20],
            color: VOXEL_PLAYER_PANTS_COLOR,
        },
        VoxelPlayerPartPose {
            part: VoxelPlayerPart::RightLeg,
            local_center: [0.22, VOXEL_PLAYER_VISUAL_FOOT_OFFSET_Y + 0.55, 0.0],
            half_extents: [0.18, 0.55, 0.20],
            color: VOXEL_PLAYER_PANTS_COLOR,
        },
    ]
}

pub(crate) fn advance_voxel_player_animation_system(
    frame_delta: Res<FrameDeltaSeconds>,
    mut clock: ResMut<VoxelPlayerAnimationClock>,
) {
    clock.seconds = (clock.seconds + frame_delta.0.max(0.0)).rem_euclid(3600.0);
}

pub(crate) fn advance_voxel_player_tool_swing_system(
    frame_delta: Res<FrameDeltaSeconds>,
    mut swing: ResMut<VoxelPlayerToolSwingState>,
) {
    if !swing.active {
        return;
    }
    swing.elapsed_seconds += frame_delta.0.max(0.0);
    if swing.elapsed_seconds >= swing.duration_seconds {
        swing.active = false;
        swing.elapsed_seconds = swing.duration_seconds;
    }
}

pub(crate) fn trigger_voxel_player_tool_swing(world: &mut World) {
    if world.get_resource::<VoxelPlayerToolSwingState>().is_none() {
        world.insert_resource(VoxelPlayerToolSwingState::default());
    }
    let mut swing = world.resource_mut::<VoxelPlayerToolSwingState>();
    swing.active = true;
    swing.elapsed_seconds = 0.05;
    swing.duration_seconds = VOXEL_PLAYER_TOOL_SWING_SECONDS;
}

impl VoxelPlayerToolSwingState {
    pub fn fraction(self) -> f32 {
        if !self.active || self.duration_seconds <= f32::EPSILON {
            return 0.0;
        }
        (self.elapsed_seconds / self.duration_seconds).clamp(0.0, 1.0)
    }
}

fn animation_state_from_motion(
    intent_strength: f32,
    sprint_down: bool,
    grounded: bool,
) -> VoxelPlayerAnimationState {
    if !grounded {
        return VoxelPlayerAnimationState::Airborne;
    }
    if intent_strength <= VOXEL_PLAYER_IDLE_SPEED_EPSILON {
        return VoxelPlayerAnimationState::Idle;
    }
    if sprint_down {
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

    let angle = x.atan2(z);
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
    fn default_rig_feet_align_to_collision_foot_offset() {
        let min_y = default_voxel_player_part_poses()
            .into_iter()
            .map(|part| part.local_center[1] - part.half_extents[1])
            .fold(f32::INFINITY, f32::min);

        assert!((min_y - VOXEL_PLAYER_VISUAL_FOOT_OFFSET_Y).abs() < 1e-5);
    }

    #[test]
    fn visual_state_classifies_airborne_before_speed() {
        assert_eq!(
            animation_state_from_motion(1.0, true, false),
            VoxelPlayerAnimationState::Airborne
        );
    }

    #[test]
    fn facing_octants_use_north_as_positive_z() {
        assert_eq!(
            facing_octant_from_velocity([0.0, 0.0, 1.0]),
            VoxelPlayerFacingOctant::NORTH
        );
        assert_eq!(
            facing_octant_from_velocity([1.0, 0.0, 0.0]),
            VoxelPlayerFacingOctant::EAST
        );
        assert_eq!(
            facing_octant_from_velocity([0.0, 0.0, -1.0]),
            VoxelPlayerFacingOctant::SOUTH
        );
        assert_eq!(
            facing_octant_from_velocity([-1.0, 0.0, 0.0]),
            VoxelPlayerFacingOctant::WEST
        );
    }

    #[test]
    fn local_player_visual_state_reads_current_player_snapshot() {
        let mut world = World::new();
        world.insert_resource(LocalPlayerEntity::default());
        world.insert_resource(EcsInputSnapshot {
            sprint_down: true,
            ..EcsInputSnapshot::default()
        });
        world.insert_resource(PlayerMovementConfig::default());
        world.insert_resource(VoxelPlayerAnimationClock { seconds: 1.25 });
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
        assert_eq!(visual.horizontal_speed, 11.0);
        assert_eq!(visual.animation_seconds, 1.25);
    }
}
