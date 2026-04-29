mod resolved;

use std::f32::consts::TAU;

#[cfg(test)]
use crate::world::CHUNK_EDGE_I32;
use crate::world::atlas::{AtlasCoord, MesoGuideCell, MesoGuideMap};
#[cfg(test)]
use crate::world::coord::ChunkCoord;

use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};
#[allow(unused_imports)]
pub(crate) use resolved::{
    CraterWindow, build_crater_window, sample_crater_apply_signal_from_window,
    sample_crater_surface_from_window,
};

const HASH_K1: u64 = 0x9E37_79B9_7F4A_7C15;
const HASH_K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const HASH_K3: u64 = 0x1656_67B1_9E37_79F9;
const SOURCE_CENTER_X_SALT: u64 = 0xD811_B6D2_2300_0001;
const SOURCE_CENTER_Z_SALT: u64 = 0xD811_B6D2_2300_0002;
const SOURCE_RADIUS_SALT: u64 = 0xD811_B6D2_2300_0003;
const SOURCE_KEEPOUT_SALT: u64 = 0xD811_B6D2_2300_0004;
const SOURCE_RIM_SALT: u64 = 0xD811_B6D2_2300_0005;
const SOURCE_FALLBACK_AXIS_SALT: u64 = 0xD811_B6D2_2300_0006;
const SOURCE_BOWL_MAJOR_SALT: u64 = 0xD811_B6D2_2300_0007;
const SOURCE_BOWL_MINOR_SALT: u64 = 0xD811_B6D2_2300_0008;
const SOURCE_DEPTH_SALT: u64 = 0xD811_B6D2_2300_0009;
const SOURCE_FLOOR_SALT: u64 = 0xD811_B6D2_2300_000A;
const SOURCE_RIM_OUTER_SALT: u64 = 0xD811_B6D2_2300_000B;
const SOURCE_APRON_SALT: u64 = 0xD811_B6D2_2300_000C;
const SOURCE_CONTOUR_PRIMARY_SALT: u64 = 0xD811_B6D2_2300_000D;
const SOURCE_CONTOUR_SECONDARY_SALT: u64 = 0xD811_B6D2_2300_000E;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CraterApplySample {
    pub bowl_coverage: f32,
    pub rim_coverage: f32,
    pub apron_coverage: f32,
    pub bowl_depth_blocks: f32,
    pub rim_raise_blocks: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraterSurfaceSample {
    pub target_surface_y: f32,
    pub blend_weight: f32,
    pub relief_spend: f32,
    pub bowl_coverage: f32,
    pub rim_coverage: f32,
}

impl CraterSurfaceSample {
    pub fn flat(base_surface_y: f32) -> Self {
        Self {
            target_surface_y: base_surface_y,
            blend_weight: 0.0,
            relief_spend: 0.0,
            bowl_coverage: 0.0,
            rim_coverage: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraterCandidate {
    pub coord: AtlasCoord,
    pub center_x: f32,
    pub center_z: f32,
    pub weight: f32,
    pub estimated_radius_blocks: f32,
    pub basin_weight: f32,
    pub basin_depth: f32,
    pub hilliness: f32,
    pub hill_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct GuideSource {
    coord: AtlasCoord,
    cell: MesoGuideCell,
    center_x: f32,
    center_z: f32,
    weight: f32,
    estimated_radius_blocks: f32,
    keepout_radius_blocks: f32,
    rim_strength_blocks: f32,
}

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "crater",
    summary: "Resolved multi-chunk volcanic crater bowls with raised rims and shallow aprons.",
    placement_family: MesoPlacementFamily::VolcanicField,
    hydrology_coupling: MesoHydrologyCoupling::None,
    terrain_effects: &[
        "Resolves sparse world-space crater bowls from stable meso-region ownership instead of rebuilding them per chunk.",
        "Samples one feature-owned target surface with separate bowl depth, rim uplift, and outer apron support.",
        "Uses existing basin and hill guide channels as a provisional launch-era volcanic proxy until a dedicated crater guide channel lands.",
    ],
    ecology_notes: &[
        "Later ecology can emphasize sparse pioneer cover, ash or scoria rims, and exposed mineral floors.",
        "The runtime helper is ready for later meso-apply wiring, but shared candidate selection and material hooks still belong to the main thread.",
    ],
};

#[cfg(test)]
pub(crate) fn sample_apply_signal(
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
) -> CraterApplySample {
    let window = build_crater_window(guides, chunk_coord_for_world_xz(world_x, world_z));
    sample_crater_apply_signal_from_window(&window, world_x, world_z)
}

#[cfg(test)]
pub(crate) fn sample_surface(
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
    base_surface_y: f32,
    relief_budget: f32,
) -> CraterSurfaceSample {
    let window = build_crater_window(guides, chunk_coord_for_world_xz(world_x, world_z));
    sample_crater_surface_from_window(
        &window,
        guides,
        world_x,
        world_z,
        base_surface_y,
        relief_budget,
    )
}

#[cfg(test)]
fn chunk_coord_for_world_xz(world_x: i32, world_z: i32) -> ChunkCoord {
    ChunkCoord(
        world_x.div_euclid(CHUNK_EDGE_I32),
        0,
        world_z.div_euclid(CHUNK_EDGE_I32),
    )
}

fn crater_source(
    coord: AtlasCoord,
    cell: MesoGuideCell,
    meso_span_blocks: f32,
) -> Option<GuideSource> {
    if cell.basin_weight < 0.34 || cell.basin_depth < 2.1 {
        return None;
    }

    let center_x = coord.x as f32 * meso_span_blocks
        + meso_span_blocks * 0.5
        + lerp_f32(
            -meso_span_blocks * 0.18,
            meso_span_blocks * 0.18,
            crater_hash01(coord, cell, SOURCE_CENTER_X_SALT),
        );
    let center_z = coord.z as f32 * meso_span_blocks
        + meso_span_blocks * 0.5
        + lerp_f32(
            -meso_span_blocks * 0.18,
            meso_span_blocks * 0.18,
            crater_hash01(coord, cell, SOURCE_CENTER_Z_SALT),
        );
    let weight = crater_score(cell);
    let estimated_radius_blocks =
        lerp_f32(
            meso_span_blocks * 0.72,
            meso_span_blocks * 1.44,
            crater_hash01(coord, cell, SOURCE_RADIUS_SALT),
        ) * (0.90 + cell.basin_weight * 0.24 + (cell.basin_depth / 12.0).clamp(0.0, 0.22));
    let keepout_radius_blocks = estimated_radius_blocks
        * lerp_f32(1.00, 1.28, crater_hash01(coord, cell, SOURCE_KEEPOUT_SALT));
    let rim_strength_blocks = (cell.basin_depth * (0.40 + cell.basin_weight * 0.18)
        + cell.hill_height * (0.24 + cell.hilliness * 0.24)
        + cell.hilliness * 1.6)
        * lerp_f32(0.86, 1.22, crater_hash01(coord, cell, SOURCE_RIM_SALT));

    Some(GuideSource {
        coord,
        cell,
        center_x,
        center_z,
        weight,
        estimated_radius_blocks,
        keepout_radius_blocks,
        rim_strength_blocks,
    })
}

fn crater_score(cell: MesoGuideCell) -> f32 {
    (cell.basin_weight * 0.66
        + (cell.basin_depth / 10.0).clamp(0.0, 1.0) * 0.28
        + cell.hilliness * 0.12
        + (cell.hill_height / 14.0).clamp(0.0, 1.0) * 0.08)
        .clamp(0.0, 1.6)
}

fn is_local_crater_peak(guides: &MesoGuideMap, coord: AtlasCoord, cell: MesoGuideCell) -> bool {
    let score = crater_score(cell);
    if score >= 1.08 || cell.basin_depth >= 6.8 {
        return true;
    }

    for neighbor_z in (coord.z - 1)..=(coord.z + 1) {
        for neighbor_x in (coord.x - 1)..=(coord.x + 1) {
            if neighbor_x == coord.x && neighbor_z == coord.z {
                continue;
            }

            let Some(neighbor) = guides
                .cells()
                .get(AtlasCoord::new(neighbor_x, neighbor_z))
                .copied()
            else {
                continue;
            };
            if neighbor.basin_weight < 0.28 || neighbor.basin_depth < 1.8 {
                continue;
            }

            let neighbor_score = crater_score(neighbor);
            let stronger_bowl = neighbor.basin_depth >= cell.basin_depth + 0.35;
            let comparable_weight = neighbor_score >= score - 0.03;
            if stronger_bowl && comparable_weight {
                return false;
            }
        }
    }

    true
}

fn collect_crater_sources(guides: &MesoGuideMap, meso_span_blocks: f32) -> Vec<GuideSource> {
    let mut candidates = Vec::new();

    for coord in guides.area().coords() {
        let Some(cell) = guides.cells().get(coord).copied() else {
            continue;
        };
        if !is_local_crater_peak(guides, coord, cell) {
            continue;
        }
        let Some(source) = crater_source(coord, cell, meso_span_blocks) else {
            continue;
        };
        candidates.push(source);
    }

    candidates.sort_by(|a, b| b.weight.total_cmp(&a.weight));
    candidates
}

pub fn debug_crater_candidates(guides: &MesoGuideMap) -> Vec<CraterCandidate> {
    let meso_span_blocks =
        (crate::world::CHUNK_EDGE_I32 * crate::world::MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;

    collect_crater_sources(guides, meso_span_blocks)
        .into_iter()
        .map(|source| CraterCandidate {
            coord: source.coord,
            center_x: source.center_x,
            center_z: source.center_z,
            weight: source.weight,
            estimated_radius_blocks: source.estimated_radius_blocks,
            basin_weight: source.cell.basin_weight,
            basin_depth: source.cell.basin_depth,
            hilliness: source.cell.hilliness,
            hill_height: source.cell.hill_height,
        })
        .collect()
}

fn crater_hash01(coord: AtlasCoord, cell: MesoGuideCell, salt: u64) -> f32 {
    let mut state = salt;
    state ^= mix_hash(coord.x as i64 as u64 ^ HASH_K1);
    state ^= mix_hash(coord.z as i64 as u64 ^ HASH_K2);
    state ^= mix_hash(cell.basin_weight.to_bits() as u64 ^ HASH_K3);
    state ^= mix_hash(cell.basin_depth.to_bits() as u64 ^ HASH_K1.rotate_left(7));
    state ^= mix_hash(cell.hilliness.to_bits() as u64 ^ HASH_K2.rotate_left(11));
    state ^= mix_hash(cell.hill_height.to_bits() as u64 ^ HASH_K3.rotate_left(19));
    let mixed = mix_hash(state);
    let mantissa = (mixed >> 40) as u32;
    mantissa as f32 / ((1_u32 << 24) - 1) as f32
}

fn mix_hash(mut state: u64) -> u64 {
    state ^= state >> 33;
    state = state.wrapping_mul(0xff51_afd7_ed55_8ccd);
    state ^= state >> 33;
    state = state.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    state ^= state >> 33;
    state
}

fn indexed_crater_hash01(coord: AtlasCoord, cell: MesoGuideCell, index: usize, salt: u64) -> f32 {
    let basin_weight_bits =
        cell.basin_weight.to_bits() ^ (index as u32 + 1).wrapping_mul(0x045d_9f3b);
    let basin_depth_bits =
        cell.basin_depth.to_bits() ^ (index as u32 + 3).wrapping_mul(0x27d4_eb2d);
    let hilliness_bits = cell.hilliness.to_bits() ^ (index as u32 + 5).wrapping_mul(0x1656_67b1);
    let hill_height_bits =
        cell.hill_height.to_bits() ^ (index as u32 + 7).wrapping_mul(0x9e37_79b9);

    crater_hash01(
        coord,
        MesoGuideCell {
            basin_weight: f32::from_bits(basin_weight_bits),
            basin_depth: f32::from_bits(basin_depth_bits),
            hilliness: f32::from_bits(hilliness_bits),
            hill_height: f32::from_bits(hill_height_bits),
            ..cell
        },
        salt,
    )
}

fn fallback_axis_from_source(source: GuideSource) -> (f32, f32) {
    let angle = crater_hash01(source.coord, source.cell, SOURCE_FALLBACK_AXIS_SALT) * TAU;
    (angle.cos(), angle.sin())
}

fn distance_between_points(a: (f32, f32), b: (f32, f32)) -> f32 {
    let delta_x = a.0 - b.0;
    let delta_z = a.1 - b.1;
    (delta_x * delta_x + delta_z * delta_z).sqrt()
}

fn insert_top2(value: f32, strongest: &mut f32, second: &mut f32) {
    if value >= *strongest {
        *second = *strongest;
        *strongest = value;
    } else if value > *second {
        *second = value;
    }
}

fn coverage_union(a: f32, b: f32) -> f32 {
    (a + b - a * b).clamp(0.0, 1.0)
}

fn smoothstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn smoothstep_range(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }

    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn soft_cap_signed(value: f32, limit: f32) -> f32 {
    if limit <= f32::EPSILON {
        return 0.0;
    }

    let sign = value.signum();
    let capped = limit * (1.0 - (-value.abs() / limit).exp());
    sign * capped
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{
        build_crater_window, debug_crater_candidates, sample_crater_surface_from_window,
        sample_surface,
    };
    use crate::world::atlas::{AtlasArea, AtlasCoord, AtlasGrid, MesoGuideCell, MesoGuideMap};
    use crate::world::{CHUNK_EDGE_I32, MESO_GUIDE_CELL_SIZE_IN_CHUNKS};

    #[test]
    fn debug_crater_candidates_only_emit_local_bowl_peaks() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 5, 4).unwrap();
        let mut cells = AtlasGrid::defaulted(area);
        *cells.get_mut(AtlasCoord::new(1, 1)).unwrap() = MesoGuideCell {
            basin_weight: 0.94,
            basin_depth: 6.8,
            hilliness: 0.42,
            hill_height: 4.8,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(2, 1)).unwrap() = MesoGuideCell {
            basin_weight: 0.82,
            basin_depth: 5.7,
            hilliness: 0.31,
            hill_height: 3.9,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(4, 3)).unwrap() = MesoGuideCell {
            basin_weight: 0.88,
            basin_depth: 5.9,
            hilliness: 0.36,
            hill_height: 4.2,
            ..MesoGuideCell::default()
        };
        let guides = MesoGuideMap { area, cells };

        let candidates = debug_crater_candidates(&guides);

        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.coord == AtlasCoord::new(1, 1)),
            "expected strongest basin cell to survive as crater candidate, got {candidates:?}"
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.coord == AtlasCoord::new(4, 3)),
            "expected separated crater cell to survive as crater candidate, got {candidates:?}"
        );
        assert!(
            !candidates
                .iter()
                .any(|candidate| candidate.coord == AtlasCoord::new(2, 1)),
            "expected weaker adjacent basin cell to be filtered out, got {candidates:?}"
        );
    }

    #[test]
    fn sample_surface_returns_flat_when_guides_are_absent() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 4, 4).unwrap();
        let guides = MesoGuideMap {
            area,
            cells: AtlasGrid::defaulted(area),
        };

        let sample = sample_surface(&guides, 96, 96, 100.0, 16.0);

        assert_eq!(sample, super::CraterSurfaceSample::flat(100.0));
    }

    #[test]
    fn sample_surface_keeps_center_lower_than_rim_for_strong_crater_guides() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 8, 8).unwrap();
        let mut cells = AtlasGrid::defaulted(area);
        *cells.get_mut(AtlasCoord::new(2, 2)).unwrap() = MesoGuideCell {
            basin_weight: 0.96,
            basin_depth: 7.2,
            hilliness: 0.54,
            hill_height: 6.1,
            ..MesoGuideCell::default()
        };
        let guides = MesoGuideMap { area, cells };
        let meso_span_blocks = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32;
        let center_x = meso_span_blocks * 2 + meso_span_blocks / 2;
        let center_z = meso_span_blocks * 2 + meso_span_blocks / 2;
        let window = build_crater_window(
            &guides,
            crate::world::coord::ChunkCoord(
                center_x.div_euclid(CHUNK_EDGE_I32),
                0,
                center_z.div_euclid(CHUNK_EDGE_I32),
            ),
        );
        let center =
            sample_crater_surface_from_window(&window, &guides, center_x, center_z, 100.0, 18.0);
        let mut strongest_rim = super::CraterSurfaceSample::flat(100.0);

        for offset_z in (-80..=80).step_by(4) {
            for offset_x in (-80..=80).step_by(4) {
                let sample = sample_crater_surface_from_window(
                    &window,
                    &guides,
                    center_x + offset_x,
                    center_z + offset_z,
                    100.0,
                    18.0,
                );
                if sample.target_surface_y > strongest_rim.target_surface_y {
                    strongest_rim = sample;
                }
            }
        }

        assert!(
            center.target_surface_y <= 96.5,
            "expected crater center to carve below the prototype, got {center:?}"
        );
        assert!(
            strongest_rim.target_surface_y >= 100.1,
            "expected crater rim to keep visible uplift, got {strongest_rim:?}"
        );
        assert!(
            strongest_rim.target_surface_y > center.target_surface_y,
            "expected crater rim to stay above bowl center, center={center:?}, rim={strongest_rim:?}"
        );
    }
}
