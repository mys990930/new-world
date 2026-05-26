use super::types::MacroFieldTileConfig;
use crate::world::generation::graph::WorldPlanePoint;
use crate::world::generation::macro_map::{MacroSite, MacroSurfaceKind};

pub(super) const RIDGE_INFLUENCE_VISIBLE_FLOOR: f32 = 0.12;

pub(super) fn lake_boundary_lowering_factor(
    primary: MacroSite,
    distance_to_curve_blocks: f32,
    blend_radius_blocks: f32,
    roughness_offset_blocks: f32,
) -> f32 {
    let roughened_distance = (distance_to_curve_blocks + roughness_offset_blocks).max(0.0);
    let away_from_boundary =
        smoothstep01(roughened_distance / blend_radius_blocks.max(f32::EPSILON));
    if is_lake_surface(primary.surface_kind) {
        away_from_boundary
    } else {
        0.0
    }
}

pub(super) fn combine_macro_height_without_river(
    macro_elevation: f32,
    ocean_mask: f32,
    lake_lowering_factor: f32,
    ridge_influence: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    let mut height = macro_elevation + ridge_influence * config.ridge_height_scale;
    if ocean_mask > 0.5 {
        height = ocean_bathymetry_macro_height(height);
    } else if lake_lowering_factor > 0.0 {
        height = lake_bed_macro_height(height, lake_lowering_factor, config);
    }
    height.clamp(-2.0, 2.0)
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn combine_macro_height(
    macro_elevation: f32,
    ocean_mask: f32,
    _coast_mask: f32,
    _lake_mask: f32,
    _dry_basin_mask: f32,
    lake_lowering_factor: f32,
    ridge_influence: f32,
    _river_shoulder_strength: f32,
    _river_flow_hint: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    combine_macro_height_without_river(
        macro_elevation,
        ocean_mask,
        lake_lowering_factor,
        ridge_influence,
        config,
    )
}

pub(super) fn ocean_bathymetry_macro_height(source_height: f32) -> f32 {
    if source_height >= 0.0 {
        return source_height;
    }

    let depth = (-source_height).max(0.0).clamp(0.0, 1.0);
    if depth <= 0.08 {
        return source_height;
    }

    let shallow_continuity = 0.08;
    let shelf = smoothstep_range(0.08, 0.18, depth) * 0.04;
    let slope = smoothstep_range(0.18, 0.52, depth) * 0.43;
    let basin = smoothstep_range(0.52, 0.92, depth) * 0.45;

    -(shallow_continuity + shelf + slope + basin).clamp(0.0, 1.0)
}

pub(super) fn lake_bed_macro_height(
    source_height: f32,
    lake_lowering_factor: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    let lake_t = lake_lowering_factor.clamp(0.0, 1.0);
    let u_shape = lake_t * lake_t * (3.0 - 2.0 * lake_t);
    let depth = 0.0025 + u_shape * 0.015;
    let target = source_height - depth;
    let preserve_relief = 1.0 - config.lake_flatten_strength.clamp(0.0, 1.0) * 0.42;

    target + (source_height - target) * preserve_relief
}

pub(super) fn roughened_distance(
    distance_blocks: f32,
    position: WorldPlanePoint,
    roughness_blocks: f32,
    salt: u64,
) -> f32 {
    if !distance_blocks.is_finite() || roughness_blocks <= 0.0 {
        return distance_blocks;
    }
    (distance_blocks + boundary_roughness_offset(position, roughness_blocks, salt)).max(0.0)
}

pub(super) fn boundary_roughness_offset(
    position: WorldPlanePoint,
    roughness_blocks: f32,
    salt: u64,
) -> f32 {
    if roughness_blocks <= 0.0 {
        return 0.0;
    }
    let broad = smooth_value_noise_2d(position, 96.0, salt);
    let medium = smooth_value_noise_2d(
        WorldPlanePoint::new(position.x + 37.0, position.z - 61.0),
        41.0,
        salt ^ 0x9E37_79B9_7F4A_7C15,
    );
    let noise = (broad * 0.58 + medium * 0.42).clamp(-1.0, 1.0);
    let shaped = noise.signum() * noise.abs().powf(0.65);
    shaped * roughness_blocks
}

pub(super) fn smooth_value_noise_2d(
    position: WorldPlanePoint,
    scale_blocks: f32,
    salt: u64,
) -> f32 {
    let scale = scale_blocks.max(1.0);
    let x = position.x / scale;
    let z = position.z / scale;
    let x0 = x.floor() as i32;
    let z0 = z.floor() as i32;
    let tx = smootherstep(x - x0 as f32);
    let tz = smootherstep(z - z0 as f32);
    let a = signed_lattice_noise(x0, z0, salt);
    let b = signed_lattice_noise(x0 + 1, z0, salt);
    let c = signed_lattice_noise(x0, z0 + 1, salt);
    let d = signed_lattice_noise(x0 + 1, z0 + 1, salt);
    let top = a + (b - a) * tx;
    let bottom = c + (d - c) * tx;
    top + (bottom - top) * tz
}

pub(super) fn signed_lattice_noise(x: i32, z: i32, salt: u64) -> f32 {
    let mut value = salt;
    value ^= (x as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= (z as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    let unit = ((value ^ (value >> 31)) as f64 / u64::MAX as f64) as f32;
    unit * 2.0 - 1.0
}

pub(super) fn smootherstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

pub(super) fn is_lake_surface(kind: MacroSurfaceKind) -> bool {
    matches!(
        kind,
        MacroSurfaceKind::LakeCandidate | MacroSurfaceKind::WetlandCandidate
    )
}

pub(super) fn envelope(distance: f32, radius: f32) -> f32 {
    if !distance.is_finite() {
        return 0.0;
    }
    let t = (1.0 - distance / radius.max(f32::EPSILON)).clamp(0.0, 1.0);
    smoothstep01(t)
}

pub(super) fn smoothstep01(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn smoothstep_range(edge0: f32, edge1: f32, value: f32) -> f32 {
    let span = (edge1 - edge0).max(f32::EPSILON);
    smoothstep01((value - edge0) / span)
}

pub(super) fn ridge_envelope(distance: f32, radius: f32) -> f32 {
    let raw = envelope(distance, radius);
    if raw <= RIDGE_INFLUENCE_VISIBLE_FLOOR {
        0.0
    } else {
        let t = ((raw - RIDGE_INFLUENCE_VISIBLE_FLOOR) / (1.0 - RIDGE_INFLUENCE_VISIBLE_FLOOR))
            .clamp(0.0, 1.0);
        smoothstep01(t).powf(0.92)
    }
}
