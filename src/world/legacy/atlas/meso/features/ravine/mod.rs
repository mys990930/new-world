mod resolved;

use std::f32::consts::TAU;

#[cfg(test)]
use crate::world::CHUNK_EDGE_I32;
use crate::world::atlas::{AtlasCoord, MesoGuideCell, MesoGuideMap};
#[cfg(test)]
use crate::world::coord::ChunkCoord;

use super::super::{hash01, lerp_f32};
use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};
pub(crate) type RavineWindow = resolved::RavineWindow;

const SOURCE_CENTER_X_SALT: u64 = 0xD811_B6D2_2600_0001;
const SOURCE_CENTER_Z_SALT: u64 = 0xD811_B6D2_2600_0002;
const SOURCE_FALLBACK_HEADING_SALT: u64 = 0xD811_B6D2_2600_0003;
const SOURCE_KEEP_DISTANCE_SALT: u64 = 0xD811_B6D2_2600_0004;
const SOURCE_DEPTH_HINT_SALT: u64 = 0xD811_B6D2_2600_0005;
const SOURCE_WEIGHT_SALT: u64 = 0xD811_B6D2_2600_0006;
const SOURCE_HEADING_BLEND_SALT: u64 = 0xD811_B6D2_2600_0007;

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "ravine",
    summary: "Feature-owned runtime ravine resolver for medium-scale drainage valleys.",
    placement_family: MesoPlacementFamily::ValleyFloor,
    hydrology_coupling: MesoHydrologyCoupling::AvoidPrimaryCorridor,
    terrain_effects: &[
        "Resolves stable world-space ravine valleys from a meso-region ownership layer instead of re-rolling trench guesses per chunk.",
        "Keeps narrow trench depth and broader shoulder lowering as separate signals so a wide lowland blend cannot fake the deepest cut.",
        "Uses downstream width/depth progression plus angular centerline kinks so the result reads as a rough eroded reach rather than a repeated smooth capsule.",
        "Should avoid displacing major river corridors and instead sit beside or above them.",
    ],
    ecology_notes: &[
        "Later ecology can use these cuts to channel denser vegetation, shade, or runoff-biased cover.",
        "Current runtime resolve derives bounded ravine sources from existing basin/escarpment/terrace guide context until dedicated ravine guide channels are added.",
    ],
};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RavineApplySample {
    pub trench_coverage: f32,
    pub shoulder_coverage: f32,
    pub trench_depth_blocks: f32,
    pub shoulder_depth_blocks: f32,
    pub cut_cap_blocks: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RavineSurfaceSample {
    pub target_surface_y: f32,
    pub blend_weight: f32,
    pub relief_spend: f32,
    pub trench_coverage: f32,
    pub shoulder_coverage: f32,
}

impl RavineSurfaceSample {
    pub fn flat(base_surface_y: f32) -> Self {
        Self {
            target_surface_y: base_surface_y,
            blend_weight: 0.0,
            relief_spend: 0.0,
            trench_coverage: 0.0,
            shoulder_coverage: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RavineCandidate {
    pub coord: AtlasCoord,
    pub center_x: f32,
    pub center_z: f32,
    pub heading_x: f32,
    pub heading_z: f32,
    pub weight: f32,
    pub depth_hint_blocks: f32,
    pub keep_distance_blocks: f32,
}

#[derive(Debug, Clone, Copy)]
struct GuideSource {
    coord: AtlasCoord,
    cell: MesoGuideCell,
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    weight: f32,
    depth_hint_blocks: f32,
    keep_distance_blocks: f32,
}

pub(crate) fn build_ravine_window(
    guides: &MesoGuideMap,
    chunk: crate::world::coord::ChunkCoord,
) -> RavineWindow {
    resolved::build_ravine_window(guides, chunk)
}

pub(crate) fn sample_ravine_apply_signal_from_window(
    window: &RavineWindow,
    world_x: i32,
    world_z: i32,
) -> RavineApplySample {
    resolved::sample_ravine_apply_signal_from_window(window, world_x, world_z)
}

pub(crate) fn sample_ravine_surface_from_window(
    window: &RavineWindow,
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
    base_surface_y: f32,
    relief_budget: f32,
) -> RavineSurfaceSample {
    resolved::sample_ravine_surface_from_window(
        window,
        guides,
        world_x,
        world_z,
        base_surface_y,
        relief_budget,
    )
}

pub fn debug_ravine_candidates(guides: &MesoGuideMap) -> Vec<RavineCandidate> {
    let meso_span_blocks =
        (crate::world::CHUNK_EDGE_I32 * crate::world::MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
    collect_ravine_sources(guides, meso_span_blocks)
        .into_iter()
        .map(|source| RavineCandidate {
            coord: source.coord,
            center_x: source.center_x,
            center_z: source.center_z,
            heading_x: source.heading_x,
            heading_z: source.heading_z,
            weight: source.weight,
            depth_hint_blocks: source.depth_hint_blocks,
            keep_distance_blocks: source.keep_distance_blocks,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn sample_ravine_apply_signal(
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
) -> RavineApplySample {
    let window = build_ravine_window(guides, chunk_coord_for_world_xz(world_x, world_z));
    sample_ravine_apply_signal_from_window(&window, world_x, world_z)
}

#[cfg(test)]
pub(crate) fn sample_ravine_surface(
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
    base_surface_y: f32,
    relief_budget: f32,
) -> RavineSurfaceSample {
    let window = build_ravine_window(guides, chunk_coord_for_world_xz(world_x, world_z));
    sample_ravine_surface_from_window(
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

fn collect_ravine_sources(guides: &MesoGuideMap, meso_span_blocks: f32) -> Vec<GuideSource> {
    let mut candidates = Vec::new();

    for coord in guides.area().coords() {
        let Some(cell) = guides.cells().get(coord).copied() else {
            continue;
        };
        if !is_local_source_peak(guides, coord, cell) {
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

fn guide_source(
    coord: AtlasCoord,
    cell: MesoGuideCell,
    meso_span_blocks: f32,
) -> Option<GuideSource> {
    let basin_signal =
        (cell.basin_weight * 0.72 + (cell.basin_depth / 7.0).clamp(0.0, 0.72)).clamp(0.0, 1.40);
    let escarpment_signal = (cell.escarpment_weight * 0.64
        + (cell.escarpment_height / 9.0).clamp(0.0, 0.64))
    .clamp(0.0, 1.28);
    let terrace_signal = (cell.terrace_weight * 0.32
        + (cell.terrace_step_height / 5.0).clamp(0.0, 0.28))
    .clamp(0.0, 0.72);
    let hill_signal =
        (cell.hilliness * 0.12 + (cell.hill_height / 18.0).clamp(0.0, 0.18)).clamp(0.0, 0.30);
    let incision_context = basin_signal.max(escarpment_signal).max(terrace_signal);
    let weight = (basin_signal * 0.44
        + escarpment_signal * 0.42
        + terrace_signal * 0.16
        + hill_signal * 0.04
        + lerp_f32(
            0.0,
            0.08,
            hash01(0, coord.x as i64, coord.z as i64, SOURCE_WEIGHT_SALT),
        ))
    .clamp(0.0, 1.52);
    if incision_context < 0.42 || weight < 0.36 {
        return None;
    }

    let center_x = coord.x as f32 * meso_span_blocks
        + meso_span_blocks * 0.5
        + lerp_f32(
            -meso_span_blocks * 0.24,
            meso_span_blocks * 0.24,
            hash01(0, coord.x as i64, coord.z as i64, SOURCE_CENTER_X_SALT),
        );
    let center_z = coord.z as f32 * meso_span_blocks
        + meso_span_blocks * 0.5
        + lerp_f32(
            -meso_span_blocks * 0.24,
            meso_span_blocks * 0.24,
            hash01(0, coord.x as i64, coord.z as i64, SOURCE_CENTER_Z_SALT),
        );
    let fallback_angle = hash01(
        0,
        coord.x as i64,
        coord.z as i64,
        SOURCE_FALLBACK_HEADING_SALT,
    ) * TAU;
    let fallback = (fallback_angle.cos(), fallback_angle.sin());
    let escarpment_heading = normalize_vec2(cell.escarpment_heading_x, cell.escarpment_heading_z);
    let terrace_heading = normalize_vec2(cell.terrace_heading_x, cell.terrace_heading_z);
    let heading = normalize_vec2(
        escarpment_heading.0 * escarpment_signal
            + terrace_heading.0 * terrace_signal * 0.82
            + fallback.0
                * lerp_f32(
                    0.18,
                    0.54,
                    hash01(0, coord.x as i64, coord.z as i64, SOURCE_HEADING_BLEND_SALT),
                ),
        escarpment_heading.1 * escarpment_signal
            + terrace_heading.1 * terrace_signal * 0.82
            + fallback.1
                * lerp_f32(
                    0.18,
                    0.54,
                    hash01(
                        0,
                        coord.x as i64,
                        coord.z as i64,
                        SOURCE_HEADING_BLEND_SALT ^ 0x55AA,
                    ),
                ),
    );
    let depth_hint_blocks =
        lerp_f32(
            7.2,
            18.5,
            hash01(0, coord.x as i64, coord.z as i64, SOURCE_DEPTH_HINT_SALT),
        ) * (0.86 + basin_signal * 0.28 + escarpment_signal * 0.24 + terrace_signal * 0.10);
    let keep_distance_blocks = meso_span_blocks
        * lerp_f32(
            1.75,
            2.72,
            hash01(0, coord.x as i64, coord.z as i64, SOURCE_KEEP_DISTANCE_SALT),
        )
        * (1.02 + weight * 0.24);

    Some(GuideSource {
        coord,
        cell,
        center_x,
        center_z,
        heading_x: heading.0,
        heading_z: heading.1,
        weight,
        depth_hint_blocks,
        keep_distance_blocks,
    })
}

fn is_local_source_peak(guides: &MesoGuideMap, coord: AtlasCoord, cell: MesoGuideCell) -> bool {
    let self_weight = local_peak_weight(cell);

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
            let neighbor_weight = local_peak_weight(neighbor);
            if neighbor_weight < 0.34 {
                continue;
            }

            if neighbor_weight >= self_weight + 0.08
                && neighbor.basin_depth >= cell.basin_depth - 0.4
                && neighbor.escarpment_height >= cell.escarpment_height - 0.4
            {
                return false;
            }
        }
    }

    true
}

fn local_peak_weight(cell: MesoGuideCell) -> f32 {
    cell.basin_weight * 0.56
        + (cell.basin_depth / 7.0).clamp(0.0, 0.74)
        + cell.escarpment_weight * 0.34
        + (cell.escarpment_height / 9.0).clamp(0.0, 0.52)
        + cell.terrace_weight * 0.18
}

fn normalize_vec2(x: f32, z: f32) -> (f32, f32) {
    let length_sq = x * x + z * z;
    if length_sq <= f32::EPSILON {
        (1.0, 0.0)
    } else {
        let inv_length = length_sq.sqrt().recip();
        (x * inv_length, z * inv_length)
    }
}

fn distance_between_points(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dz = a.1 - b.1;
    (dx * dx + dz * dz).sqrt()
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

fn soft_cap_positive(value: f32, cap: f32) -> f32 {
    if value <= f32::EPSILON || cap <= f32::EPSILON {
        return 0.0;
    }

    cap * (1.0 - (-(value / cap)).exp())
}

#[cfg(test)]
mod tests {
    use super::{
        RavineCandidate, debug_ravine_candidates, guide_source, sample_ravine_apply_signal,
        sample_ravine_surface,
    };
    use crate::world::atlas::{AtlasArea, AtlasCoord, AtlasGrid, MesoGuideCell, MesoGuideMap};
    use crate::world::{CHUNK_EDGE_I32, MESO_GUIDE_CELL_SIZE_IN_CHUNKS};

    #[test]
    fn debug_candidates_only_emit_local_ravine_peaks() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 4, 4).unwrap();
        let mut cells = AtlasGrid::defaulted(area);
        *cells.get_mut(AtlasCoord::new(1, 1)).unwrap() = MesoGuideCell {
            basin_weight: 0.82,
            basin_depth: 5.2,
            escarpment_weight: 0.64,
            escarpment_height: 5.8,
            escarpment_heading_x: 0.0,
            escarpment_heading_z: 1.0,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(2, 1)).unwrap() = MesoGuideCell {
            basin_weight: 0.66,
            basin_depth: 4.1,
            escarpment_weight: 0.48,
            escarpment_height: 4.4,
            escarpment_heading_x: 0.0,
            escarpment_heading_z: 1.0,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(3, 3)).unwrap() = MesoGuideCell {
            basin_weight: 0.78,
            basin_depth: 4.8,
            escarpment_weight: 0.58,
            escarpment_height: 5.2,
            escarpment_heading_x: 1.0,
            escarpment_heading_z: 0.0,
            ..MesoGuideCell::default()
        };
        let guides = MesoGuideMap { area, cells };

        let candidates = debug_ravine_candidates(&guides);

        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.coord == AtlasCoord::new(1, 1)),
            "expected strongest ravine source to appear in debug candidates, got {candidates:?}"
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.coord == AtlasCoord::new(3, 3)),
            "expected separated ravine source to appear in debug candidates, got {candidates:?}"
        );
        assert!(
            !candidates
                .iter()
                .any(|candidate| candidate.coord == AtlasCoord::new(2, 1)),
            "expected weaker adjacent ravine cell to be filtered out as a non-peak, got {candidates:?}"
        );
    }

    #[test]
    fn guide_sources_keep_nonzero_depth_hint_and_heading() {
        let meso_span_blocks = (CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
        let source = guide_source(
            AtlasCoord::new(2, 3),
            MesoGuideCell {
                basin_weight: 0.72,
                basin_depth: 4.6,
                escarpment_weight: 0.54,
                escarpment_height: 5.0,
                escarpment_heading_x: 0.0,
                escarpment_heading_z: 1.0,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .expect("strong ravine guide should resolve to a source");

        assert!(
            source.depth_hint_blocks >= 4.0,
            "expected guide source to keep visible cut depth, got {source:?}"
        );
        assert!(
            source.heading_x.abs() + source.heading_z.abs() >= 0.9,
            "expected guide source heading to stay normalized, got {source:?}"
        );
        assert!(
            source.keep_distance_blocks >= meso_span_blocks * 0.8,
            "expected ravine source to keep multi-cell spacing, got {source:?}"
        );
    }

    #[test]
    fn sample_surface_returns_flat_when_guides_are_absent() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 4, 4).unwrap();
        let cells = AtlasGrid::defaulted(area);
        let guides = MesoGuideMap { area, cells };
        let sample = sample_ravine_surface(&guides, 96, 96, 100.0, 12.0);

        assert_eq!(sample, super::RavineSurfaceSample::flat(100.0));
    }

    #[test]
    fn ravine_debug_candidates_are_deterministic() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 5, 5).unwrap();
        let mut cells = AtlasGrid::defaulted(area);
        *cells.get_mut(AtlasCoord::new(2, 2)).unwrap() = MesoGuideCell {
            basin_weight: 0.84,
            basin_depth: 5.4,
            escarpment_weight: 0.62,
            escarpment_height: 6.2,
            escarpment_heading_x: 1.0,
            escarpment_heading_z: 0.0,
            ..MesoGuideCell::default()
        };
        let guides = MesoGuideMap { area, cells };

        let first: Vec<RavineCandidate> = debug_ravine_candidates(&guides);
        let second: Vec<RavineCandidate> = debug_ravine_candidates(&guides);

        assert_eq!(first, second);
    }

    #[test]
    fn ravine_surface_keeps_floor_deeper_than_shoulders() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 4, 4).unwrap();
        let mut cells = AtlasGrid::defaulted(area);
        *cells.get_mut(AtlasCoord::new(1, 1)).unwrap() = MesoGuideCell {
            basin_weight: 0.88,
            basin_depth: 5.6,
            escarpment_weight: 0.72,
            escarpment_height: 6.8,
            escarpment_heading_x: 1.0,
            escarpment_heading_z: 0.0,
            ..MesoGuideCell::default()
        };
        let guides = MesoGuideMap { area, cells };
        let meso_span_blocks = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32;
        let center_world_x = meso_span_blocks + meso_span_blocks / 2;
        let center_world_z = meso_span_blocks + meso_span_blocks / 2;
        let center = sample_ravine_apply_signal(&guides, center_world_x, center_world_z);
        let shoulder = sample_ravine_apply_signal(&guides, center_world_x, center_world_z + 18);

        assert!(
            center.trench_depth_blocks >= shoulder.trench_depth_blocks,
            "expected ravine centerline to keep the deepest cut, center={center:?}, shoulder={shoulder:?}"
        );
        assert!(
            center.shoulder_coverage >= center.trench_coverage,
            "expected ravine shoulders to stay broader than trench core, got {center:?}"
        );
    }
}
