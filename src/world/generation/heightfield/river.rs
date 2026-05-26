use super::{HeightfieldColumn, HeightfieldTerrainKind};

pub(super) fn apply_river_water_descent(
    columns: &mut [HeightfieldColumn],
    _width: usize,
    _height: usize,
    _sea_level_blocks: f32,
) {
    for column in columns {
        column.river_core_strength = 0.0;
        column.river_shoulder_strength = 0.0;
        column.river_valley_strength = 0.0;
        column.river_distance_blocks = 0.0;
        column.river_flow_hint = 0.0;
        column.river_core_depth_blocks = 0.0;
        column.river_bank_roughness_hint = 0.0;
        column.river_gravel_hint = 0.0;
        column.river_cutbank_hint = 0.0;
        column.river_core_water_height_blocks = None;

        if matches!(
            column.terrain_kind,
            HeightfieldTerrainKind::RiverCore | HeightfieldTerrainKind::RiverBed
        ) {
            column.terrain_kind = HeightfieldTerrainKind::Land;
            column.water_level_blocks = None;
            column.water_y = None;
        }
    }
}
