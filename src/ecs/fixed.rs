use bevy_ecs::prelude::{Query, Res, ResMut, Resource, With};

use crate::simulation::SimulationResult;
use crate::world::{ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasCoord, CHUNK_EDGE_I32};

use super::player::{LocalPlayerEntity, Player, Transform};

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SimClock {
    pub tick_index: u64,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveSimRegion {
    pub center_atlas: AtlasCoord,
    pub area: AtlasArea,
}

impl Default for ActiveSimRegion {
    fn default() -> Self {
        Self {
            center_atlas: AtlasCoord::new(0, 0),
            area: AtlasArea::new(AtlasCoord::new(0, 0), 1, 1)
                .expect("default active simulation area must be valid"),
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimulationControlState {
    pub time_enabled: bool,
    pub eager_atlas_radius: u32,
}

impl Default for SimulationControlState {
    fn default() -> Self {
        Self {
            time_enabled: true,
            eager_atlas_radius: 0,
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct PendingSimulationResults(pub Vec<SimulationResult>);

pub(crate) fn advance_sim_clock_system(mut sim_clock: ResMut<SimClock>) {
    sim_clock.tick_index = sim_clock.tick_index.saturating_add(1);
}

pub(crate) fn update_active_sim_region_system(
    local_player: Res<LocalPlayerEntity>,
    transforms: Query<&Transform, With<Player>>,
    control: Res<SimulationControlState>,
    mut active_region: ResMut<ActiveSimRegion>,
) {
    let Some(entity) = local_player.0 else {
        return;
    };

    let Ok(transform) = transforms.get(entity) else {
        return;
    };

    *active_region =
        active_sim_region_for_translation(transform.translation, control.eager_atlas_radius);
}

fn atlas_coord_for_translation(translation: [f32; 3]) -> AtlasCoord {
    let atlas_span_blocks = (ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32).max(1);
    AtlasCoord::new(
        (translation[0].floor() as i32).div_euclid(atlas_span_blocks),
        (translation[2].floor() as i32).div_euclid(atlas_span_blocks),
    )
}

fn active_sim_region_for_translation(
    translation: [f32; 3],
    eager_atlas_radius: u32,
) -> ActiveSimRegion {
    let center_atlas = atlas_coord_for_translation(translation);
    let radius = eager_atlas_radius as i32;
    let origin = AtlasCoord::new(center_atlas.x - radius, center_atlas.z - radius);
    let edge = (radius * 2 + 1).max(1) as u32;
    ActiveSimRegion {
        center_atlas,
        area: AtlasArea::new(origin, edge, edge).expect("active simulation area must stay valid"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_region_tracks_player_atlas_cell() {
        let active = active_sim_region_for_translation([600.0, 0.0, -12.0], 0);
        assert_eq!(active.center_atlas, AtlasCoord::new(1, -1));
        assert_eq!(active.area.origin(), AtlasCoord::new(1, -1));
    }
}
