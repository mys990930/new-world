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
    DuneFieldWindow, build_dune_field_window, sample_dune_field_apply_signal_from_window,
    sample_dune_field_surface_from_window,
};

const SOURCE_CENTER_X_SALT: u64 = 0xD811_B6D2_2800_0001;
const SOURCE_CENTER_Z_SALT: u64 = 0xD811_B6D2_2800_0002;
const SOURCE_HEADING_SALT: u64 = 0xD811_B6D2_2800_0003;
const SOURCE_SPACING_SALT: u64 = 0xD811_B6D2_2800_0004;
const SOURCE_LENGTH_SALT: u64 = 0xD811_B6D2_2800_0005;
const SOURCE_HEIGHT_SALT: u64 = 0xD811_B6D2_2800_0006;
const SOURCE_KEEP_OUT_SALT: u64 = 0xD811_B6D2_2800_0007;
const SOURCE_COUNT_SALT: u64 = 0xD811_B6D2_2800_0008;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DuneFieldApplySample {
    pub crest_coverage: f32,
    pub field_coverage: f32,
    pub crest_height_blocks: f32,
    pub support_height_blocks: f32,
    pub raise_cap_blocks: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuneFieldSurfaceSample {
    pub target_surface_y: f32,
    pub blend_weight: f32,
    pub relief_spend: f32,
    pub crest_coverage: f32,
    pub field_coverage: f32,
}

impl DuneFieldSurfaceSample {
    pub fn flat(base_surface_y: f32) -> Self {
        Self {
            target_surface_y: base_surface_y,
            blend_weight: 0.0,
            relief_spend: 0.0,
            crest_coverage: 0.0,
            field_coverage: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct GuideSource {
    coord: AtlasCoord,
    cell: MesoGuideCell,
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    weight: f32,
    crest_spacing_blocks: f32,
    field_length_blocks: f32,
    crest_height_blocks: f32,
    ridge_count: usize,
    keepout_radius_blocks: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuneFieldRidgeCandidate {
    pub coord: AtlasCoord,
    pub center_x: f32,
    pub center_z: f32,
    pub heading_x: f32,
    pub heading_z: f32,
    pub weight: f32,
    pub crest_spacing_blocks: f32,
    pub field_length_blocks: f32,
    pub crest_height_blocks: f32,
    pub ridge_count: usize,
    pub keepout_radius_blocks: f32,
}

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "dune_field",
    summary: "Feature-owned resolved dune fields with deterministic multi-ridge arid surface sampling.",
    placement_family: MesoPlacementFamily::AridExposure,
    hydrology_coupling: MesoHydrologyCoupling::PrefersAridRunoff,
    terrain_effects: &[
        "Resolves sparse multi-ridge dune groups from stable meso-region ownership instead of rebuilding a different crest guess per sampled column.",
        "Keeps broad field support separate from sharper dune-crest height so arid transition fill cannot fake a crest where no resolved ridge exists.",
        "Treats dry spacing and heading hints as world-space dune objects whose footprint can cross chunk seams without rerolling orientation.",
    ],
    ecology_notes: &[
        "Later ecology can bias sparse scrub, exposed sediment, or dune-tolerant cover from the same resolved field footprint and lee-side shelter hints.",
        "Until shared dune guide channels land, preview/debug wiring may route dune staging input through terrace-like spacing and heading fields before the main thread hooks authoritative atlas emission.",
    ],
};

fn guide_source(
    coord: AtlasCoord,
    cell: MesoGuideCell,
    meso_span_blocks: f32,
) -> Option<GuideSource> {
    if cell.terrace_weight < 0.24 || cell.terrace_step_height < 0.45 {
        return None;
    }

    let center_x = coord.x as f32 * meso_span_blocks
        + meso_span_blocks * 0.5
        + lerp_f32(
            -meso_span_blocks * 0.18,
            meso_span_blocks * 0.18,
            dune_hash01(coord, cell, SOURCE_CENTER_X_SALT),
        );
    let center_z = coord.z as f32 * meso_span_blocks
        + meso_span_blocks * 0.5
        + lerp_f32(
            -meso_span_blocks * 0.18,
            meso_span_blocks * 0.18,
            dune_hash01(coord, cell, SOURCE_CENTER_Z_SALT),
        );
    let fallback_angle = dune_hash01(coord, cell, SOURCE_HEADING_SALT) * TAU;
    let heading = normalize_vec2_or_fallback(
        cell.terrace_heading_x,
        cell.terrace_heading_z,
        (fallback_angle.cos(), fallback_angle.sin()),
    );
    let weight =
        (0.22 + cell.terrace_weight * 0.72 + (cell.terrace_step_height / 5.0).clamp(0.0, 0.34))
            .clamp(0.22, 1.46);
    let crest_spacing_blocks = (meso_span_blocks
        * cell.terrace_spacing_cells.clamp(0.55, 1.45)
        * lerp_f32(0.34, 0.58, dune_hash01(coord, cell, SOURCE_SPACING_SALT)))
    .clamp(18.0, 54.0);
    let field_length_blocks = lerp_f32(78.0, 168.0, dune_hash01(coord, cell, SOURCE_LENGTH_SALT))
        * (0.92 + cell.terrace_weight * 0.24);
    let crest_height_blocks = (cell.terrace_step_height
        * lerp_f32(1.18, 2.05, dune_hash01(coord, cell, SOURCE_HEIGHT_SALT))
        * (0.92 + cell.terrace_weight * 0.16))
        .clamp(0.8, 8.6);
    let ridge_count = if weight >= 1.06 {
        5
    } else if dune_hash01(coord, cell, SOURCE_COUNT_SALT) >= 0.44 {
        4
    } else {
        3
    };
    let keepout_radius_blocks = field_length_blocks
        .max(crest_spacing_blocks * ridge_count as f32 * 0.96)
        * lerp_f32(0.42, 0.62, dune_hash01(coord, cell, SOURCE_KEEP_OUT_SALT));

    Some(GuideSource {
        coord,
        cell,
        center_x,
        center_z,
        heading_x: heading.0,
        heading_z: heading.1,
        weight,
        crest_spacing_blocks,
        field_length_blocks,
        crest_height_blocks,
        ridge_count,
        keepout_radius_blocks,
    })
}

fn collect_peak_sources(guides: &MesoGuideMap, meso_span_blocks: f32) -> Vec<GuideSource> {
    let mut candidates = Vec::new();

    for coord in guides.area().coords() {
        let Some(cell) = guides.cells().get(coord).copied() else {
            continue;
        };
        if !is_local_dune_peak(guides, coord, cell) {
            continue;
        }
        let Some(source) = guide_source(coord, cell, meso_span_blocks) else {
            continue;
        };
        candidates.push(source);
    }

    candidates.sort_by(|a, b| b.weight.total_cmp(&a.weight));
    candidates
}

pub fn debug_dune_field_ridge_candidates(guides: &MesoGuideMap) -> Vec<DuneFieldRidgeCandidate> {
    let meso_span_blocks =
        (crate::world::CHUNK_EDGE_I32 * crate::world::MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
    collect_peak_sources(guides, meso_span_blocks)
        .into_iter()
        .map(|source| DuneFieldRidgeCandidate {
            coord: source.coord,
            center_x: source.center_x,
            center_z: source.center_z,
            heading_x: source.heading_x,
            heading_z: source.heading_z,
            weight: source.weight,
            crest_spacing_blocks: source.crest_spacing_blocks,
            field_length_blocks: source.field_length_blocks,
            crest_height_blocks: source.crest_height_blocks,
            ridge_count: source.ridge_count,
            keepout_radius_blocks: source.keepout_radius_blocks,
        })
        .collect()
}

fn is_local_dune_peak(guides: &MesoGuideMap, coord: AtlasCoord, cell: MesoGuideCell) -> bool {
    if cell.terrace_weight >= 0.92 || cell.terrace_step_height >= 2.8 {
        return true;
    }

    let self_score = cell.terrace_weight * 0.78 + (cell.terrace_step_height / 5.0).clamp(0.0, 0.22);
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
            if neighbor.terrace_weight < 0.18 || neighbor.terrace_step_height < 0.35 {
                continue;
            }

            let neighbor_score = neighbor.terrace_weight * 0.78
                + (neighbor.terrace_step_height / 5.0).clamp(0.0, 0.22);
            if neighbor_score >= self_score + 0.06
                && neighbor.terrace_weight >= cell.terrace_weight - 0.04
            {
                return false;
            }
        }
    }

    true
}

fn normalize_vec2_or_fallback(x: f32, z: f32, fallback: (f32, f32)) -> (f32, f32) {
    let length_sq = x * x + z * z;
    if length_sq <= f32::EPSILON || !length_sq.is_finite() {
        fallback
    } else {
        let inv_length = length_sq.sqrt().recip();
        (x * inv_length, z * inv_length)
    }
}

fn quantize_for_hash(value: f32) -> u64 {
    if !value.is_finite() {
        0
    } else {
        (value * 1024.0).round() as i64 as u64
    }
}

fn dune_hash01(coord: AtlasCoord, cell: MesoGuideCell, salt: u64) -> f32 {
    let mut state = salt
        ^ (coord.x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (coord.z as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ quantize_for_hash(cell.terrace_weight).wrapping_mul(0x1656_67B1_9E37_79F9)
        ^ quantize_for_hash(cell.terrace_step_height).wrapping_mul(0x94D0_49BB_1331_11EB)
        ^ quantize_for_hash(cell.terrace_spacing_cells).wrapping_mul(0xD6E8_FEB8_6659_FD93)
        ^ quantize_for_hash(cell.terrace_heading_x).wrapping_mul(0xA24B_AED4_963E_E407)
        ^ quantize_for_hash(cell.terrace_heading_z).wrapping_mul(0x9FB2_1C65_1E98_DF25);
    state ^= state >> 30;
    state = state.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state ^= state >> 27;
    state = state.wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^= state >> 31;
    ((state >> 40) as u32) as f32 / ((1u32 << 24) - 1) as f32
}

fn indexed_dune_hash01(coord: AtlasCoord, cell: MesoGuideCell, index: usize, salt: u64) -> f32 {
    dune_hash01(
        coord,
        cell,
        salt ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
    )
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[cfg(test)]
pub(crate) fn sample_dune_field_apply_signal(
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
) -> DuneFieldApplySample {
    let window = build_dune_field_window(guides, chunk_coord_for_world_xz(world_x, world_z));
    sample_dune_field_apply_signal_from_window(&window, world_x, world_z)
}

#[cfg(test)]
pub(crate) fn sample_dune_field_surface(
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
    base_surface_y: f32,
    relief_budget: f32,
) -> DuneFieldSurfaceSample {
    let window = build_dune_field_window(guides, chunk_coord_for_world_xz(world_x, world_z));
    sample_dune_field_surface_from_window(
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

#[cfg(test)]
mod tests {
    use super::{
        DuneFieldSurfaceSample, debug_dune_field_ridge_candidates, guide_source,
        indexed_dune_hash01, sample_dune_field_apply_signal, sample_dune_field_surface,
    };
    use crate::world::atlas::{AtlasArea, AtlasCoord, AtlasGrid, MesoGuideCell, MesoGuideMap};
    use crate::world::{CHUNK_EDGE_I32, MESO_GUIDE_CELL_SIZE_IN_CHUNKS};

    fn dune_guides(area: AtlasArea, specs: &[(AtlasCoord, MesoGuideCell)]) -> MesoGuideMap {
        let mut cells = AtlasGrid::defaulted(area);
        for (coord, cell) in specs {
            *cells.get_mut(*coord).unwrap() = *cell;
        }
        MesoGuideMap { area, cells }
    }

    fn strong_dune_cell() -> MesoGuideCell {
        MesoGuideCell {
            terrace_weight: 0.92,
            terrace_step_height: 2.6,
            terrace_spacing_cells: 0.94,
            terrace_heading_x: 1.0,
            terrace_heading_z: 0.0,
            ..MesoGuideCell::default()
        }
    }

    #[test]
    fn debug_candidates_only_emit_local_dune_peaks() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 8, 8).unwrap();
        let guides = dune_guides(
            area,
            &[
                (AtlasCoord::new(2, 2), strong_dune_cell()),
                (
                    AtlasCoord::new(3, 2),
                    MesoGuideCell {
                        terrace_weight: 0.66,
                        terrace_step_height: 1.5,
                        terrace_spacing_cells: 0.88,
                        terrace_heading_x: 1.0,
                        terrace_heading_z: 0.0,
                        ..MesoGuideCell::default()
                    },
                ),
                (
                    AtlasCoord::new(5, 4),
                    MesoGuideCell {
                        terrace_weight: 0.90,
                        terrace_step_height: 2.4,
                        terrace_spacing_cells: 1.08,
                        terrace_heading_x: 0.0,
                        terrace_heading_z: 1.0,
                        ..MesoGuideCell::default()
                    },
                ),
            ],
        );

        let candidates = debug_dune_field_ridge_candidates(&guides);

        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.coord == AtlasCoord::new(2, 2)),
            "expected strongest dune source to survive as a debug candidate, got {candidates:?}"
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.coord == AtlasCoord::new(5, 4)),
            "expected separated dune source to survive as a debug candidate, got {candidates:?}"
        );
        assert!(
            !candidates
                .iter()
                .any(|candidate| candidate.coord == AtlasCoord::new(3, 2)),
            "expected weaker adjacent dune source to be filtered out, got {candidates:?}"
        );
    }

    #[test]
    fn staged_terrace_inputs_build_meaningful_dune_sources() {
        let meso_span_blocks = (CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
        let cell = strong_dune_cell();
        let source = guide_source(AtlasCoord::new(2, 2), cell, meso_span_blocks)
            .expect("strong terrace-like dune staging input should resolve into a dune source");

        assert!(
            source.crest_spacing_blocks >= 18.0 && source.crest_spacing_blocks <= 54.0,
            "expected dune crest spacing to stay in a readable band, got {source:?}"
        );
        assert!(
            source.ridge_count >= 3,
            "expected dune fields to resolve into multiple ridges, got {source:?}"
        );
    }

    #[test]
    fn indexed_hash_stays_stable_for_same_source() {
        let cell = strong_dune_cell();
        let first = indexed_dune_hash01(AtlasCoord::new(3, 4), cell, 2, 0xCAFE);
        let second = indexed_dune_hash01(AtlasCoord::new(3, 4), cell, 2, 0xCAFE);

        assert_eq!(first, second);
    }

    #[test]
    fn sample_surface_returns_flat_when_guides_are_absent() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 8, 8).unwrap();
        let guides = dune_guides(area, &[]);
        let sample = sample_dune_field_surface(&guides, 128, 128, 100.0, 8.0);

        assert_eq!(sample, DuneFieldSurfaceSample::flat(100.0));
    }

    #[test]
    fn sample_apply_signal_keeps_field_coverage_broader_than_crest_coverage() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 8, 8).unwrap();
        let guides = dune_guides(area, &[(AtlasCoord::new(3, 3), strong_dune_cell())]);
        let sample = sample_dune_field_apply_signal(&guides, 224, 224);

        assert!(
            sample.crest_height_blocks > 0.0,
            "expected resolved dune ridges to raise a crest, got {sample:?}"
        );
        assert!(
            sample.field_coverage >= sample.crest_coverage,
            "expected dune field coverage to stay broader than the crest core, got {sample:?}"
        );
    }
}
