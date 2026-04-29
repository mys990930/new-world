use bevy_ecs::prelude::Resource;

use crate::simulation::{
    LocalClimateDisplay, LocalClimateState, display_local_climate, evaluate_local_climate,
};
use crate::world::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasCoord, CHUNK_EDGE_I32, LocalWeatherState, RegionClassSample,
    WorldCalendar, WorldCore,
};

#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct LocalEnvironmentStatus {
    current: Option<LocalEnvironmentSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalEnvironmentSnapshot {
    pub atlas_coord: AtlasCoord,
    pub region: RegionClassSample,
    pub calendar: WorldCalendar,
    pub weather: LocalWeatherState,
    pub climate: LocalClimateState,
    pub display: LocalClimateDisplay,
}

impl LocalEnvironmentStatus {
    pub fn current(&self) -> Option<LocalEnvironmentSnapshot> {
        self.current
    }

    pub fn refresh_from_world(&mut self, player_translation: Option<[f32; 3]>, world: &WorldCore) {
        self.current = player_translation.map(|translation| {
            let atlas_coord = atlas_coord_for_translation(translation);
            let region = world
                .sample_cached_region_class_atlas(atlas_coord)
                .unwrap_or_else(RegionClassSample::default);
            let calendar = *world.calendar();
            let weather = world.local_weather(atlas_coord).unwrap_or_else(|| {
                LocalWeatherState::clear(
                    atlas_coord,
                    region.climate_regime,
                    calendar.absolute_tick,
                    calendar.absolute_tick,
                )
            });
            let climate = evaluate_local_climate(region, world.climate_state(atlas_coord));
            let display = display_local_climate(climate, weather);

            LocalEnvironmentSnapshot {
                atlas_coord,
                region,
                calendar,
                weather,
                climate,
                display,
            }
        });
    }
}

fn atlas_coord_for_translation(translation: [f32; 3]) -> AtlasCoord {
    let atlas_span_blocks = (ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32).max(1);
    AtlasCoord::new(
        (translation[0].floor() as i32).div_euclid(atlas_span_blocks),
        (translation[2].floor() as i32).div_euclid(atlas_span_blocks),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::world::{BlockRegistry, WorldMeta};

    fn test_world() -> WorldCore {
        let registry =
            Arc::new(BlockRegistry::load_default().expect("default registry should load"));
        WorldCore::new(WorldMeta::new(42), registry)
    }

    #[test]
    fn refresh_samples_local_environment_from_player_translation() {
        let world = test_world();
        let mut status = LocalEnvironmentStatus::default();

        status.refresh_from_world(Some([520.0, 0.0, -12.0]), &world);

        let current = status.current().expect("environment snapshot should exist");
        assert_eq!(current.atlas_coord, AtlasCoord::new(2, -1));
        assert_eq!(current.calendar, *world.calendar());
    }

    #[test]
    fn refresh_clears_snapshot_without_player_translation() {
        let world = test_world();
        let mut status = LocalEnvironmentStatus::default();
        status.refresh_from_world(Some([0.0, 0.0, 0.0]), &world);

        status.refresh_from_world(None, &world);

        assert!(status.current().is_none());
    }
}
