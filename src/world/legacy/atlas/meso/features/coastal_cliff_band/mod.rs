mod resolved;

use crate::world::atlas::{AtlasCoord, MesoGuideCell, MesoGuideMap};

use super::super::{hash01, lerp_f32, normalize_vec2};
use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub(crate) use resolved::{
    build_coastal_cliff_band_window, sample_coastal_cliff_band_surface_from_window,
};

const SOURCE_LENGTH_SALT: u64 = 0xD811_B6D2_2300_0001;
const SOURCE_FACE_WIDTH_SALT: u64 = 0xD811_B6D2_2300_0002;
const SOURCE_PLATEAU_DEPTH_SALT: u64 = 0xD811_B6D2_2300_0003;
const SOURCE_APRON_WIDTH_SALT: u64 = 0xD811_B6D2_2300_0004;
const SOURCE_BENCH_LIFT_SALT: u64 = 0xD811_B6D2_2300_0005;
const SOURCE_KEEP_DISTANCE_SALT: u64 = 0xD811_B6D2_2300_0006;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CoastalCliffBandApplySample {
    pub face_coverage: f32,
    pub plateau_coverage: f32,
    pub bench_coverage: f32,
    pub crest_raise_blocks: f32,
    pub bench_lift_blocks: f32,
    pub signed_distance_blocks: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoastalCliffBandSurfaceSample {
    pub target_surface_y: f32,
    pub blend_weight: f32,
    pub relief_spend: f32,
    pub face_coverage: f32,
    pub plateau_coverage: f32,
    pub bench_coverage: f32,
}

impl CoastalCliffBandSurfaceSample {
    pub fn flat(base_surface_y: f32) -> Self {
        Self {
            target_surface_y: base_surface_y,
            blend_weight: 0.0,
            relief_spend: 0.0,
            face_coverage: 0.0,
            plateau_coverage: 0.0,
            bench_coverage: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg(test)]
pub struct CoastalCliffBandPeakCandidate {
    pub coord: AtlasCoord,
    pub center_x: f32,
    pub center_z: f32,
    pub heading_x: f32,
    pub heading_z: f32,
    pub weight: f32,
    pub keep_distance_blocks: f32,
    pub escarpment_weight: f32,
    pub escarpment_height: f32,
}

#[derive(Debug, Clone, Copy)]
struct CliffSource {
    coord: AtlasCoord,
    cell: MesoGuideCell,
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    normal_x: f32,
    normal_z: f32,
    weight: f32,
    keep_distance_blocks: f32,
}

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "coastal_cliff_band",
    summary: "Deterministic world-space coastal cliff bands resolved from coastal escarpment guides.",
    placement_family: MesoPlacementFamily::CoastalEdge,
    hydrology_coupling: MesoHydrologyCoupling::RequiresCoast,
    terrain_effects: &[
        "Resolves coast-parallel cliff objects from broad escarpment guide bands instead of rebuilding a per-column guess.",
        "Raises a landward cliff-top shelf with an explicit crest, face, and outer bench so coastal relief stays readable across several chunks.",
        "Keeps runtime ownership at the meso-region layer so neighboring chunk requests sample the same cliff object graph.",
    ],
    ecology_notes: &[
        "Later ecology can separate exposed cliff or spray-tolerant cover from inland cover.",
        "The current feature-local runtime reads escarpment guide channels as a temporary coastal-cliff input contract until shared dispatch isolates coastal cliff candidates explicitly.",
    ],
};

#[cfg(test)]
pub fn debug_coastal_cliff_band_peak_candidates(
    guides: &MesoGuideMap,
) -> Vec<CoastalCliffBandPeakCandidate> {
    let meso_span_blocks =
        (crate::world::CHUNK_EDGE_I32 * crate::world::MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
    collect_peak_sources(guides, meso_span_blocks)
        .into_iter()
        .map(|source| CoastalCliffBandPeakCandidate {
            coord: source.coord,
            center_x: source.center_x,
            center_z: source.center_z,
            heading_x: source.heading_x,
            heading_z: source.heading_z,
            weight: source.weight,
            keep_distance_blocks: source.keep_distance_blocks,
            escarpment_weight: source.cell.escarpment_weight,
            escarpment_height: source.cell.escarpment_height,
        })
        .collect()
}

fn guide_source(
    coord: AtlasCoord,
    cell: MesoGuideCell,
    meso_span_blocks: f32,
) -> Option<CliffSource> {
    if cell.escarpment_weight < 0.26 || cell.escarpment_height < 2.2 {
        return None;
    }

    let (heading_x, heading_z) =
        normalize_vec2(cell.escarpment_heading_x, cell.escarpment_heading_z);
    let normal_x = -heading_z;
    let normal_z = heading_x;
    let cell_center_x = coord.x as f32 * meso_span_blocks + meso_span_blocks * 0.5;
    let cell_center_z = coord.z as f32 * meso_span_blocks + meso_span_blocks * 0.5;
    let signed_distance_blocks = cell.escarpment_signed_distance_cells * meso_span_blocks;
    let center_x = cell_center_x - normal_x * signed_distance_blocks;
    let center_z = cell_center_z - normal_z * signed_distance_blocks;
    let weight =
        (0.20 + cell.escarpment_weight * 0.72 + (cell.escarpment_height / 10.0).clamp(0.0, 0.40))
            .clamp(0.20, 1.44);
    let keep_distance_blocks = lerp_f32(
        meso_span_blocks * 0.58,
        meso_span_blocks * 1.02,
        cliff_hash01(coord, cell, SOURCE_KEEP_DISTANCE_SALT),
    ) * (0.82
        + cell.escarpment_weight * 0.26
        + (cell.escarpment_height / 14.0).clamp(0.0, 0.16));

    Some(CliffSource {
        coord,
        cell,
        center_x,
        center_z,
        heading_x,
        heading_z,
        normal_x,
        normal_z,
        weight,
        keep_distance_blocks,
    })
}

fn collect_peak_sources(guides: &MesoGuideMap, meso_span_blocks: f32) -> Vec<CliffSource> {
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

fn is_local_source_peak(guides: &MesoGuideMap, coord: AtlasCoord, cell: MesoGuideCell) -> bool {
    if cell.escarpment_weight >= 0.92 || cell.escarpment_height >= 6.6 {
        return true;
    }

    let heading = normalize_vec2(cell.escarpment_heading_x, cell.escarpment_heading_z);

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
            if neighbor.escarpment_weight < 0.22 || neighbor.escarpment_height < 1.8 {
                continue;
            }

            let neighbor_heading =
                normalize_vec2(neighbor.escarpment_heading_x, neighbor.escarpment_heading_z);
            let aligned = heading_alignment(heading, neighbor_heading) >= 0.72;
            let stronger_weight = neighbor.escarpment_weight >= cell.escarpment_weight + 0.04;
            let stronger_height = neighbor.escarpment_height >= cell.escarpment_height + 0.45;
            let more_centered = neighbor.escarpment_signed_distance_cells.abs()
                <= cell.escarpment_signed_distance_cells.abs() + 0.12;

            if aligned && stronger_weight && (stronger_height || more_centered) {
                return false;
            }
        }
    }

    true
}

fn cliff_hash01(coord: AtlasCoord, cell: MesoGuideCell, salt: u64) -> f32 {
    let seed = salt
        ^ ((cell.escarpment_weight.to_bits() as u64) << 1)
        ^ ((cell.escarpment_height.to_bits() as u64) << 13)
        ^ ((cell.escarpment_signed_distance_cells.to_bits() as u64) << 29)
        ^ (cell.escarpment_heading_x.to_bits() as u64).rotate_left(7)
        ^ (cell.escarpment_heading_z.to_bits() as u64).rotate_left(23);
    hash01(seed, coord.x as i64, coord.z as i64, salt.rotate_left(17))
}

fn heading_alignment(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 * b.0 + a.1 * b.1).abs()
}

fn distance_between_points(a: (f32, f32), b: (f32, f32)) -> f32 {
    let delta_x = a.0 - b.0;
    let delta_z = a.1 - b.1;
    (delta_x * delta_x + delta_z * delta_z).sqrt()
}

fn smoothstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn smoothstep_range(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }
    smoothstep01((value - edge0) / (edge1 - edge0))
}

fn soft_cap_positive(value: f32, cap: f32) -> f32 {
    if value <= 0.0 || cap <= f32::EPSILON {
        return 0.0;
    }
    cap * (1.0 - (-value / cap).exp())
}

fn coverage_union(a: f32, b: f32) -> f32 {
    (a + b - a * b).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{
        SOURCE_APRON_WIDTH_SALT, SOURCE_BENCH_LIFT_SALT, SOURCE_FACE_WIDTH_SALT,
        SOURCE_LENGTH_SALT, SOURCE_PLATEAU_DEPTH_SALT, cliff_hash01,
        debug_coastal_cliff_band_peak_candidates, guide_source,
    };
    use crate::world::atlas::{AtlasArea, AtlasCoord, AtlasGrid, MesoGuideCell, MesoGuideMap};
    use crate::world::{CHUNK_EDGE_I32, MESO_GUIDE_CELL_SIZE_IN_CHUNKS};

    #[test]
    fn guide_source_reconstructs_centerline_from_signed_distance() {
        let meso_span_blocks = (CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
        let cell = MesoGuideCell {
            escarpment_weight: 0.86,
            escarpment_height: 5.2,
            escarpment_heading_x: 1.0,
            escarpment_heading_z: 0.0,
            escarpment_signed_distance_cells: 0.50,
            ..MesoGuideCell::default()
        };
        let source = guide_source(AtlasCoord::new(2, 3), cell, meso_span_blocks).unwrap();
        let expected_center_z =
            (3.0 * meso_span_blocks + meso_span_blocks * 0.5) - meso_span_blocks * 0.50;

        assert!(
            (source.center_z - expected_center_z).abs() <= 0.001,
            "expected source centerline reconstruction to apply signed-distance offset, got {source:?}"
        );
    }

    #[test]
    fn debug_peak_candidates_only_emit_local_cliff_peaks() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 5, 4).unwrap();
        let mut cells = AtlasGrid::defaulted(area);
        *cells.get_mut(AtlasCoord::new(1, 1)).unwrap() = MesoGuideCell {
            escarpment_weight: 0.94,
            escarpment_height: 6.1,
            escarpment_heading_x: 1.0,
            escarpment_heading_z: 0.0,
            escarpment_signed_distance_cells: 0.06,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(2, 1)).unwrap() = MesoGuideCell {
            escarpment_weight: 0.82,
            escarpment_height: 5.3,
            escarpment_heading_x: 1.0,
            escarpment_heading_z: 0.0,
            escarpment_signed_distance_cells: 0.08,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(4, 2)).unwrap() = MesoGuideCell {
            escarpment_weight: 0.91,
            escarpment_height: 5.8,
            escarpment_heading_x: 0.0,
            escarpment_heading_z: 1.0,
            escarpment_signed_distance_cells: -0.10,
            ..MesoGuideCell::default()
        };
        let guides = MesoGuideMap { area, cells };

        let candidates = debug_coastal_cliff_band_peak_candidates(&guides);

        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.coord == AtlasCoord::new(1, 1)),
            "expected strongest local cliff source to appear, got {candidates:?}"
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.coord == AtlasCoord::new(4, 2)),
            "expected separated cliff source to appear, got {candidates:?}"
        );
        assert!(
            !candidates
                .iter()
                .any(|candidate| candidate.coord == AtlasCoord::new(2, 1)),
            "expected weaker adjacent cliff source to be filtered out, got {candidates:?}"
        );
    }

    #[test]
    fn source_shape_hashes_stay_deterministic_for_same_cell() {
        let coord = AtlasCoord::new(2, 2);
        let cell = MesoGuideCell {
            escarpment_weight: 0.88,
            escarpment_height: 5.6,
            escarpment_heading_x: 0.6,
            escarpment_heading_z: 0.8,
            escarpment_signed_distance_cells: -0.14,
            ..MesoGuideCell::default()
        };
        let first = [
            cliff_hash01(coord, cell, SOURCE_LENGTH_SALT),
            cliff_hash01(coord, cell, SOURCE_FACE_WIDTH_SALT),
            cliff_hash01(coord, cell, SOURCE_PLATEAU_DEPTH_SALT),
            cliff_hash01(coord, cell, SOURCE_APRON_WIDTH_SALT),
            cliff_hash01(coord, cell, SOURCE_BENCH_LIFT_SALT),
        ];
        let second = [
            cliff_hash01(coord, cell, SOURCE_LENGTH_SALT),
            cliff_hash01(coord, cell, SOURCE_FACE_WIDTH_SALT),
            cliff_hash01(coord, cell, SOURCE_PLATEAU_DEPTH_SALT),
            cliff_hash01(coord, cell, SOURCE_APRON_WIDTH_SALT),
            cliff_hash01(coord, cell, SOURCE_BENCH_LIFT_SALT),
        ];

        assert_eq!(first, second);
    }
}
