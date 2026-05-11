use super::super::atlas::{ATLAS_CELL_SIZE_IN_CHUNKS, AtlasCoord, BiomeFamily};
use super::super::coord::ChunkCoord;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceCondition {
    pub kind: SurfaceConditionKind,
    pub wetness: f32,
    pub snow_depth: f32,
    pub thaw: f32,
}

impl SurfaceCondition {
    pub fn new(kind: SurfaceConditionKind, wetness: f32, snow_depth: f32, thaw: f32) -> Self {
        Self {
            kind,
            wetness: clamp_observation_unit(wetness),
            snow_depth: clamp_observation_unit(snow_depth),
            thaw: clamp_observation_unit(thaw),
        }
    }

    pub const fn dry() -> Self {
        Self {
            kind: SurfaceConditionKind::Dry,
            wetness: 0.0,
            snow_depth: 0.0,
            thaw: 0.0,
        }
    }

    pub fn wet(wetness: f32) -> Self {
        Self::new(SurfaceConditionKind::Wet, wetness, 0.0, 0.0)
    }

    pub fn snow_covered(snow_depth: f32) -> Self {
        Self::new(SurfaceConditionKind::SnowCovered, 0.0, snow_depth, 0.0)
    }

    pub fn half_thawed_snow(snow_depth: f32, thaw: f32) -> Self {
        Self::new(SurfaceConditionKind::HalfThawedSnow, 0.0, snow_depth, thaw)
    }

    pub fn frozen(wetness: f32) -> Self {
        Self::new(SurfaceConditionKind::Frozen, wetness, 0.0, 0.0)
    }
}

impl Default for SurfaceCondition {
    fn default() -> Self {
        Self::dry()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceConditionKind {
    Dry,
    Wet,
    SnowCovered,
    HalfThawedSnow,
    Frozen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceConditionScope {
    AtlasCell(AtlasCoord),
    Chunk(ChunkCoord),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceConditionObservation {
    pub scope: SurfaceConditionScope,
    pub cell_biome: BiomeFamily,
    pub condition: SurfaceCondition,
}

pub fn atlas_coord_for_chunk(coord: ChunkCoord) -> AtlasCoord {
    let cell_span = ATLAS_CELL_SIZE_IN_CHUNKS as i32;
    AtlasCoord::new(coord.0.div_euclid(cell_span), coord.2.div_euclid(cell_span))
}

fn clamp_observation_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_condition_numeric_fields_are_clamped_for_observers() {
        let condition =
            SurfaceCondition::new(SurfaceConditionKind::HalfThawedSnow, -1.0, 1.5, f32::NAN);

        assert_eq!(condition.wetness, 0.0);
        assert_eq!(condition.snow_depth, 1.0);
        assert_eq!(condition.thaw, 0.0);
    }

    #[test]
    fn chunk_to_atlas_mapping_uses_world_owned_atlas_scale() {
        assert_eq!(
            atlas_coord_for_chunk(ChunkCoord(0, 0, 0)),
            AtlasCoord::new(0, 0)
        );
        assert_eq!(
            atlas_coord_for_chunk(ChunkCoord(7, 0, 7)),
            AtlasCoord::new(0, 0)
        );
        assert_eq!(
            atlas_coord_for_chunk(ChunkCoord(8, 0, -1)),
            AtlasCoord::new(1, -1)
        );
        assert_eq!(
            atlas_coord_for_chunk(ChunkCoord(-1, 0, -8)),
            AtlasCoord::new(-1, -1)
        );
    }
}
