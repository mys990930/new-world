pub mod catalog;
pub mod features;

pub use catalog::{MesoCatalogEntry, MesoCatalogStatus, meso_catalog_entries};
pub use features::{MesoFeatureDef, meso_feature_def, meso_feature_defs};

use std::f32::consts::TAU;

use crate::world::WorldMeta;

use super::atlas_fields::AtlasCell;
use super::scale::{ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasCoord, AtlasGrid};
use super::seed::{domain_warp, fbm};
use super::structure::{AtlasStructureMap, DrainageNodeKind};
use super::tuning::AtlasTuning;

pub const MESO_GUIDE_CELL_SIZE_IN_CHUNKS: u32 = 2;
pub const MESO_GUIDE_CELLS_PER_ATLAS_CELL: u32 =
    ATLAS_CELL_SIZE_IN_CHUNKS / MESO_GUIDE_CELL_SIZE_IN_CHUNKS;
pub const MESO_REGION_EDGE_CELLS: u32 = MESO_GUIDE_CELLS_PER_ATLAS_CELL;

const MESO_FEATURE_SPAWN_SALT: u64 = 0xD811_B6D2_2000_0001;
const MESO_FEATURE_PICK_SALT: u64 = 0xD811_B6D2_2000_0002;
const MESO_FEATURE_HEADING_SALT: u64 = 0xD811_B6D2_2000_0003;
const MESO_FEATURE_RADIUS_X_SALT: u64 = 0xD811_B6D2_2000_0004;
const MESO_FEATURE_RADIUS_Z_SALT: u64 = 0xD811_B6D2_2000_0005;
const MESO_FEATURE_STRENGTH_SALT: u64 = 0xD811_B6D2_2000_0006;
const MESO_FEATURE_SPACING_SALT: u64 = 0xD811_B6D2_2000_0007;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MesoRegionCoord {
    pub x: i32,
    pub z: i32,
}

impl MesoRegionCoord {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MesoRegion {
    coord: MesoRegionCoord,
}

impl MesoRegion {
    pub fn new(coord: MesoRegionCoord) -> Self {
        Self { coord }
    }

    pub fn coord(self) -> MesoRegionCoord {
        self.coord
    }

    pub fn atlas_area(self) -> AtlasArea {
        AtlasArea::new(AtlasCoord::new(self.coord.x, self.coord.z), 1, 1)
            .expect("meso region atlas area must be valid")
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MesoGuideCell {
    pub hilliness: f32,
    pub hill_height: f32,
    pub basin_weight: f32,
    pub basin_depth: f32,
    pub escarpment_weight: f32,
    pub escarpment_height: f32,
    pub escarpment_heading_x: f32,
    pub escarpment_heading_z: f32,
    pub escarpment_signed_distance_cells: f32,
    pub terrace_weight: f32,
    pub terrace_step_height: f32,
    pub terrace_spacing_cells: f32,
    pub terrace_heading_x: f32,
    pub terrace_heading_z: f32,
    pub terrace_signed_distance_cells: f32,
}

impl Default for MesoGuideCell {
    fn default() -> Self {
        Self {
            hilliness: 0.0,
            hill_height: 0.0,
            basin_weight: 0.0,
            basin_depth: 0.0,
            escarpment_weight: 0.0,
            escarpment_height: 0.0,
            escarpment_heading_x: 1.0,
            escarpment_heading_z: 0.0,
            escarpment_signed_distance_cells: 0.0,
            terrace_weight: 0.0,
            terrace_step_height: 0.0,
            terrace_spacing_cells: 1.0,
            terrace_heading_x: 1.0,
            terrace_heading_z: 0.0,
            terrace_signed_distance_cells: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MesoGuideSample {
    pub hilliness: f32,
    pub hill_height: f32,
    pub basin_weight: f32,
    pub basin_depth: f32,
    pub escarpment_weight: f32,
    pub escarpment_height: f32,
    pub escarpment_heading_x: f32,
    pub escarpment_heading_z: f32,
    pub escarpment_signed_distance_cells: f32,
    pub terrace_weight: f32,
    pub terrace_step_height: f32,
    pub terrace_spacing_cells: f32,
    pub terrace_heading_x: f32,
    pub terrace_heading_z: f32,
    pub terrace_signed_distance_cells: f32,
}

impl Default for MesoGuideSample {
    fn default() -> Self {
        Self {
            hilliness: 0.0,
            hill_height: 0.0,
            basin_weight: 0.0,
            basin_depth: 0.0,
            escarpment_weight: 0.0,
            escarpment_height: 0.0,
            escarpment_heading_x: 1.0,
            escarpment_heading_z: 0.0,
            escarpment_signed_distance_cells: 0.0,
            terrace_weight: 0.0,
            terrace_step_height: 0.0,
            terrace_spacing_cells: 1.0,
            terrace_heading_x: 1.0,
            terrace_heading_z: 0.0,
            terrace_signed_distance_cells: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MesoGuideMap {
    area: AtlasArea,
    cells: AtlasGrid<MesoGuideCell>,
}

impl MesoGuideMap {
    pub fn area(&self) -> AtlasArea {
        self.area
    }

    pub fn cells(&self) -> &AtlasGrid<MesoGuideCell> {
        &self.cells
    }
}

impl PartialEq for MesoGuideMap {
    fn eq(&self, other: &Self) -> bool {
        self.area == other.area && self.cells.values() == other.cells.values()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MesoFeatureKind {
    HillCluster,
    Basin,
    EscarpmentBand,
    TerraceBand,
}

#[derive(Debug, Clone, Copy)]
struct StructureContext {
    ridge_weight: f32,
    ridge_heading_x: f32,
    ridge_heading_z: f32,
    channel_weight: f32,
    channel_heading_x: f32,
    channel_heading_z: f32,
    confluence_weight: f32,
}

impl Default for StructureContext {
    fn default() -> Self {
        Self {
            ridge_weight: 0.0,
            ridge_heading_x: 1.0,
            ridge_heading_z: 0.0,
            channel_weight: 0.0,
            channel_heading_x: 1.0,
            channel_heading_z: 0.0,
            confluence_weight: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct FeatureWeights {
    hill_cluster: f32,
    basin: f32,
    escarpment_band: f32,
    terrace_band: f32,
}

impl FeatureWeights {
    fn strongest(self) -> f32 {
        self.hill_cluster
            .max(self.basin)
            .max(self.escarpment_band)
            .max(self.terrace_band)
    }

    fn total(self) -> f32 {
        self.hill_cluster + self.basin + self.escarpment_band + self.terrace_band
    }
}

#[derive(Debug, Clone, Copy)]
struct FeatureInstance {
    kind: MesoFeatureKind,
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    radius_x_cells: f32,
    radius_z_cells: f32,
    strength_blocks: f32,
    spacing_cells: f32,
}

pub fn meso_region_coord_for_atlas(coord: AtlasCoord) -> MesoRegionCoord {
    MesoRegionCoord::new(coord.x, coord.z)
}

pub fn meso_regions_covering_area(area: AtlasArea) -> Vec<MesoRegionCoord> {
    let origin = area.origin();
    let max_coord = AtlasCoord::new(
        origin.x + area.width() as i32 - 1,
        origin.z + area.height() as i32 - 1,
    );
    let mut regions = Vec::with_capacity(area.len());

    for z in origin.z..=max_coord.z {
        for x in origin.x..=max_coord.x {
            regions.push(MesoRegionCoord::new(x, z));
        }
    }

    regions
}

pub fn generate_meso_guides(
    meta: &WorldMeta,
    area: AtlasArea,
    fields: &super::atlas_fields::AtlasFieldMap,
    structure: &AtlasStructureMap,
) -> MesoGuideMap {
    let meso_area = atlas_area_to_meso_cell_area(area);
    let mut cells = AtlasGrid::defaulted(meso_area);
    let land_threshold = AtlasTuning::default().normalization.land_threshold;

    for region_coord in meso_regions_covering_area(area) {
        let region = MesoRegion::new(region_coord);
        emit_region_features(meta.seed, region, land_threshold, fields, structure, &mut cells);
    }

    MesoGuideMap {
        area: meso_area,
        cells,
    }
}

pub fn sample_meso_guides(guides: &MesoGuideMap, world_x: i32, world_z: i32) -> MesoGuideSample {
    let meso_span_blocks = (crate::world::CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32)
        .max(1);
    let cell_x = world_x.div_euclid(meso_span_blocks);
    let cell_z = world_z.div_euclid(meso_span_blocks);
    let frac_x = (world_x.rem_euclid(meso_span_blocks) as f32 + 0.5) / meso_span_blocks as f32;
    let frac_z = (world_z.rem_euclid(meso_span_blocks) as f32 + 0.5) / meso_span_blocks as f32;

    let c00 = guides
        .cells
        .get(AtlasCoord::new(cell_x, cell_z))
        .expect("meso guide sample must exist");
    let c10 = guides
        .cells
        .get(AtlasCoord::new(cell_x + 1, cell_z))
        .expect("meso guide east sample must exist");
    let c01 = guides
        .cells
        .get(AtlasCoord::new(cell_x, cell_z + 1))
        .expect("meso guide south sample must exist");
    let c11 = guides
        .cells
        .get(AtlasCoord::new(cell_x + 1, cell_z + 1))
        .expect("meso guide southeast sample must exist");

    let escarpment_heading = normalize_vec2(
        bilerp(
            c00.escarpment_heading_x,
            c10.escarpment_heading_x,
            c01.escarpment_heading_x,
            c11.escarpment_heading_x,
            frac_x,
            frac_z,
        ),
        bilerp(
            c00.escarpment_heading_z,
            c10.escarpment_heading_z,
            c01.escarpment_heading_z,
            c11.escarpment_heading_z,
            frac_x,
            frac_z,
        ),
    );
    let terrace_heading = normalize_vec2(
        bilerp(
            c00.terrace_heading_x,
            c10.terrace_heading_x,
            c01.terrace_heading_x,
            c11.terrace_heading_x,
            frac_x,
            frac_z,
        ),
        bilerp(
            c00.terrace_heading_z,
            c10.terrace_heading_z,
            c01.terrace_heading_z,
            c11.terrace_heading_z,
            frac_x,
            frac_z,
        ),
    );

    MesoGuideSample {
        hilliness: bilerp(
            c00.hilliness,
            c10.hilliness,
            c01.hilliness,
            c11.hilliness,
            frac_x,
            frac_z,
        ),
        hill_height: bilerp(
            c00.hill_height,
            c10.hill_height,
            c01.hill_height,
            c11.hill_height,
            frac_x,
            frac_z,
        ),
        basin_weight: bilerp(
            c00.basin_weight,
            c10.basin_weight,
            c01.basin_weight,
            c11.basin_weight,
            frac_x,
            frac_z,
        ),
        basin_depth: bilerp(
            c00.basin_depth,
            c10.basin_depth,
            c01.basin_depth,
            c11.basin_depth,
            frac_x,
            frac_z,
        ),
        escarpment_weight: bilerp(
            c00.escarpment_weight,
            c10.escarpment_weight,
            c01.escarpment_weight,
            c11.escarpment_weight,
            frac_x,
            frac_z,
        ),
        escarpment_height: bilerp(
            c00.escarpment_height,
            c10.escarpment_height,
            c01.escarpment_height,
            c11.escarpment_height,
            frac_x,
            frac_z,
        ),
        escarpment_heading_x: escarpment_heading.0,
        escarpment_heading_z: escarpment_heading.1,
        escarpment_signed_distance_cells: bilerp(
            c00.escarpment_signed_distance_cells,
            c10.escarpment_signed_distance_cells,
            c01.escarpment_signed_distance_cells,
            c11.escarpment_signed_distance_cells,
            frac_x,
            frac_z,
        ),
        terrace_weight: bilerp(
            c00.terrace_weight,
            c10.terrace_weight,
            c01.terrace_weight,
            c11.terrace_weight,
            frac_x,
            frac_z,
        ),
        terrace_step_height: bilerp(
            c00.terrace_step_height,
            c10.terrace_step_height,
            c01.terrace_step_height,
            c11.terrace_step_height,
            frac_x,
            frac_z,
        ),
        terrace_spacing_cells: bilerp(
            c00.terrace_spacing_cells,
            c10.terrace_spacing_cells,
            c01.terrace_spacing_cells,
            c11.terrace_spacing_cells,
            frac_x,
            frac_z,
        )
        .max(0.55),
        terrace_heading_x: terrace_heading.0,
        terrace_heading_z: terrace_heading.1,
        terrace_signed_distance_cells: bilerp(
            c00.terrace_signed_distance_cells,
            c10.terrace_signed_distance_cells,
            c01.terrace_signed_distance_cells,
            c11.terrace_signed_distance_cells,
            frac_x,
            frac_z,
        ),
    }
}

fn atlas_area_to_meso_cell_area(area: AtlasArea) -> AtlasArea {
    AtlasArea::new(
        AtlasCoord::new(
            area.origin().x * MESO_GUIDE_CELLS_PER_ATLAS_CELL as i32,
            area.origin().z * MESO_GUIDE_CELLS_PER_ATLAS_CELL as i32,
        ),
        area.width() * MESO_GUIDE_CELLS_PER_ATLAS_CELL,
        area.height() * MESO_GUIDE_CELLS_PER_ATLAS_CELL,
    )
    .expect("meso guide area must be non-empty")
}

fn emit_region_features(
    seed: u64,
    region: MesoRegion,
    land_threshold: f32,
    fields: &super::atlas_fields::AtlasFieldMap,
    structure: &AtlasStructureMap,
    cells: &mut AtlasGrid<MesoGuideCell>,
) {
    let origin = region.atlas_area().origin();

    for local_z in 0..MESO_REGION_EDGE_CELLS as i32 {
        for local_x in 0..MESO_REGION_EDGE_CELLS as i32 {
            let cell_coord = AtlasCoord::new(
                origin.x * MESO_GUIDE_CELLS_PER_ATLAS_CELL as i32 + local_x,
                origin.z * MESO_GUIDE_CELLS_PER_ATLAS_CELL as i32 + local_z,
            );
            let sample_point = (
                origin.x as f32
                    + (local_x as f32 + 0.5) / MESO_GUIDE_CELLS_PER_ATLAS_CELL as f32,
                origin.z as f32
                    + (local_z as f32 + 0.5) / MESO_GUIDE_CELLS_PER_ATLAS_CELL as f32,
            );
            let atlas_sample =
                sample_atlas_fields_fractional(fields, sample_point.0, sample_point.1);
            let structure_context = sample_structure_context(structure, sample_point);
            let weights = candidate_weights(atlas_sample, structure_context, land_threshold);
            if let Some(instance) = pick_feature_instance(
                seed,
                cell_coord,
                sample_point,
                atlas_sample,
                structure_context,
                weights,
            ) {
                rasterize_feature(instance, cells);
            }
        }
    }
}

fn candidate_weights(
    sample: AtlasCell,
    structure: StructureContext,
    land_threshold: f32,
) -> FeatureWeights {
    let inland = clamp01((sample.landness - land_threshold + 0.22) / 0.44);
    let coast_suppression = 1.0 - sample.coast_factor.clamp(0.0, 1.0);
    let ridge_shoulder = clamp01(structure.ridge_weight * 1.20 - structure.channel_weight * 0.40);
    let low_relief = clamp01(1.0 - sample.ruggedness * 1.05);
    let moderate_relief = clamp01(1.0 - (sample.ruggedness - 0.38).abs() * 2.4);
    let wet_bias = clamp01(sample.wetness * 0.82 + sample.lake_potential * 0.42);
    let dry_bias = clamp01(sample.aridity * 0.88 + sample.coast_factor * 0.10);

    FeatureWeights {
        hill_cluster: clamp01(
            inland
                * coast_suppression
                * (0.34
                    + moderate_relief * 0.22
                    + sample.macro_elevation * 0.16
                    + sample.continent_core_factor * 0.14
                    + (1.0 - structure.channel_weight) * 0.14
                    + (1.0 - structure.ridge_weight * 0.72) * 0.10),
        ),
        basin: clamp01(
            inland
                * coast_suppression
                * (0.26
                    + low_relief * 0.22
                    + wet_bias * 0.18
                    + (1.0 - structure.ridge_weight) * 0.16
                    + (1.0 - sample.macro_elevation) * 0.14
                    + structure.channel_weight * 0.08),
        ),
        escarpment_band: clamp01(
            (0.16
                + sample.ruggedness * 0.24
                + sample.ridge_factor * 0.16
                + ridge_shoulder * 0.22
                + sample.coast_factor * 0.10
                + sample.macro_elevation * 0.12
                - wet_bias * 0.08)
                * clamp01(0.45 + inland * 0.40 + sample.coast_factor * 0.30),
        ),
        terrace_band: clamp01(
            (0.14
                + sample.macro_elevation * 0.22
                + sample.ruggedness * 0.14
                + structure.ridge_weight * 0.12
                + sample.coast_factor * 0.16
                + dry_bias * 0.12
                + structure.channel_weight * 0.08)
                * clamp01(0.38 + inland * 0.28 + sample.coast_factor * 0.34),
        ),
    }
}

fn pick_feature_instance(
    seed: u64,
    cell_coord: AtlasCoord,
    sample_point: (f32, f32),
    sample: AtlasCell,
    structure: StructureContext,
    weights: FeatureWeights,
) -> Option<FeatureInstance> {
    let best_weight = weights.strongest();
    if best_weight < 0.30 {
        return None;
    }

    let spawn_roll =
        hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_SPAWN_SALT);
    let spawn_threshold = (0.22 + best_weight * 0.60).clamp(0.28, 0.84);
    if spawn_roll > spawn_threshold {
        return None;
    }

    let total = weights.total();
    if total <= f32::EPSILON {
        return None;
    }

    let jitter = cell_center_jitter(seed, cell_coord);
    let pick_roll =
        hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_PICK_SALT) * total;
    let kind = weighted_pick(weights, pick_roll)?;
    let heading = feature_heading(seed, cell_coord, kind, structure);

    Some(match kind {
        MesoFeatureKind::HillCluster => features::hill_cluster::build_instance(
            seed,
            cell_coord,
            sample_point,
            sample,
            heading,
        ),
        MesoFeatureKind::Basin => FeatureInstance {
            kind,
            center_x: sample_point.0 * MESO_GUIDE_CELLS_PER_ATLAS_CELL as f32 + jitter.0,
            center_z: sample_point.1 * MESO_GUIDE_CELLS_PER_ATLAS_CELL as f32 + jitter.1,
            heading_x: heading.0,
            heading_z: heading.1,
            radius_x_cells: lerp_f32(
                1.6,
                2.9,
                hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_RADIUS_X_SALT),
            ),
            radius_z_cells: lerp_f32(
                1.4,
                2.7,
                hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_RADIUS_Z_SALT),
            ),
            strength_blocks: lerp_f32(
                2.0,
                4.8,
                hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_STRENGTH_SALT),
            ) * (0.85 + sample.wetness * 0.20),
            spacing_cells: 1.0,
        },
        MesoFeatureKind::EscarpmentBand => FeatureInstance {
            kind,
            center_x: sample_point.0 * MESO_GUIDE_CELLS_PER_ATLAS_CELL as f32 + jitter.0,
            center_z: sample_point.1 * MESO_GUIDE_CELLS_PER_ATLAS_CELL as f32 + jitter.1,
            heading_x: heading.0,
            heading_z: heading.1,
            radius_x_cells: lerp_f32(
                2.4,
                4.8,
                hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_RADIUS_X_SALT),
            ),
            radius_z_cells: lerp_f32(
                0.7,
                1.3,
                hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_RADIUS_Z_SALT),
            ),
            strength_blocks: lerp_f32(
                2.8,
                6.2,
                hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_STRENGTH_SALT),
            ) * (0.78 + sample.ruggedness * 0.30),
            spacing_cells: 1.0,
        },
        MesoFeatureKind::TerraceBand => FeatureInstance {
            kind,
            center_x: sample_point.0 * MESO_GUIDE_CELLS_PER_ATLAS_CELL as f32 + jitter.0,
            center_z: sample_point.1 * MESO_GUIDE_CELLS_PER_ATLAS_CELL as f32 + jitter.1,
            heading_x: heading.0,
            heading_z: heading.1,
            radius_x_cells: lerp_f32(
                2.2,
                4.2,
                hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_RADIUS_X_SALT),
            ),
            radius_z_cells: lerp_f32(
                1.0,
                1.8,
                hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_RADIUS_Z_SALT),
            ),
            strength_blocks: lerp_f32(
                1.4,
                2.6,
                hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_STRENGTH_SALT),
            ),
            spacing_cells: lerp_f32(
                0.85,
                1.45,
                hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_SPACING_SALT),
            ),
        },
    })
}

fn weighted_pick(weights: FeatureWeights, pick_roll: f32) -> Option<MesoFeatureKind> {
    let mut cursor = 0.0_f32;
    cursor += weights.hill_cluster;
    if pick_roll <= cursor {
        return Some(MesoFeatureKind::HillCluster);
    }
    cursor += weights.basin;
    if pick_roll <= cursor {
        return Some(MesoFeatureKind::Basin);
    }
    cursor += weights.escarpment_band;
    if pick_roll <= cursor {
        return Some(MesoFeatureKind::EscarpmentBand);
    }
    cursor += weights.terrace_band;
    if pick_roll <= cursor {
        return Some(MesoFeatureKind::TerraceBand);
    }

    None
}

fn feature_heading(
    seed: u64,
    cell_coord: AtlasCoord,
    kind: MesoFeatureKind,
    structure: StructureContext,
) -> (f32, f32) {
    let fallback_angle =
        hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_HEADING_SALT) * TAU;
    let fallback = (fallback_angle.cos(), fallback_angle.sin());

    match kind {
        MesoFeatureKind::HillCluster | MesoFeatureKind::Basin => fallback,
        MesoFeatureKind::EscarpmentBand => {
            if structure.ridge_weight >= 0.12 {
                normalize_vec2(structure.ridge_heading_x, structure.ridge_heading_z)
            } else {
                fallback
            }
        }
        MesoFeatureKind::TerraceBand => {
            if structure.ridge_weight >= 0.10 {
                normalize_vec2(structure.ridge_heading_x, structure.ridge_heading_z)
            } else if structure.channel_weight >= 0.10 {
                normalize_vec2(structure.channel_heading_x, structure.channel_heading_z)
            } else {
                fallback
            }
        }
    }
}

fn cell_center_jitter(seed: u64, cell_coord: AtlasCoord) -> (f32, f32) {
    let jitter_x =
        hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_RADIUS_X_SALT) - 0.5;
    let jitter_z =
        hash01(seed, cell_coord.x as i64, cell_coord.z as i64, MESO_FEATURE_RADIUS_Z_SALT) - 0.5;
    (jitter_x * 0.38, jitter_z * 0.38)
}

fn rasterize_feature(instance: FeatureInstance, cells: &mut AtlasGrid<MesoGuideCell>) {
    let min_x = (instance.center_x - instance.radius_x_cells - 1.0).floor() as i32;
    let max_x = (instance.center_x + instance.radius_x_cells + 1.0).ceil() as i32;
    let min_z = (instance.center_z - instance.radius_x_cells - 1.0).floor() as i32;
    let max_z = (instance.center_z + instance.radius_x_cells + 1.0).ceil() as i32;
    let normal = (-instance.heading_z, instance.heading_x);

    for cell_z in min_z..=max_z {
        for cell_x in min_x..=max_x {
            let coord = AtlasCoord::new(cell_x, cell_z);
            let Some(cell) = cells.get_mut(coord) else {
                continue;
            };
            let delta_x = coord.x as f32 + 0.5 - instance.center_x;
            let delta_z = coord.z as f32 + 0.5 - instance.center_z;
            let along = delta_x * instance.heading_x + delta_z * instance.heading_z;
            let across = delta_x * normal.0 + delta_z * normal.1;

            match instance.kind {
                MesoFeatureKind::HillCluster => {
                    features::hill_cluster::rasterize(instance, cell, along, across);
                }
                MesoFeatureKind::Basin => {
                    let footprint = ellipse_footprint(
                        along,
                        across,
                        instance.radius_x_cells,
                        instance.radius_z_cells,
                    );
                    if footprint <= 0.0 {
                        continue;
                    }
                    cell.basin_weight = (cell.basin_weight + footprint).clamp(0.0, 1.0);
                    cell.basin_depth = cell.basin_depth.max(instance.strength_blocks * footprint);
                }
                MesoFeatureKind::EscarpmentBand => {
                    let footprint = band_footprint(
                        along,
                        across,
                        instance.radius_x_cells,
                        instance.radius_z_cells,
                    );
                    if footprint <= 0.0 {
                        continue;
                    }
                    if footprint > cell.escarpment_weight {
                        cell.escarpment_weight = footprint;
                        cell.escarpment_height = instance.strength_blocks;
                        cell.escarpment_heading_x = instance.heading_x;
                        cell.escarpment_heading_z = instance.heading_z;
                        cell.escarpment_signed_distance_cells = across;
                    }
                }
                MesoFeatureKind::TerraceBand => {
                    let footprint = band_footprint(
                        along,
                        across,
                        instance.radius_x_cells,
                        instance.radius_z_cells,
                    );
                    if footprint <= 0.0 {
                        continue;
                    }
                    if footprint > cell.terrace_weight {
                        cell.terrace_weight = footprint;
                        cell.terrace_step_height = instance.strength_blocks;
                        cell.terrace_spacing_cells = instance.spacing_cells;
                        cell.terrace_heading_x = instance.heading_x;
                        cell.terrace_heading_z = instance.heading_z;
                        cell.terrace_signed_distance_cells = across;
                    }
                }
            }
        }
    }
}

fn ellipse_footprint(along: f32, across: f32, radius_x: f32, radius_z: f32) -> f32 {
    if radius_x <= f32::EPSILON || radius_z <= f32::EPSILON {
        return 0.0;
    }
    let normalized = ((along / radius_x).powi(2) + (across / radius_z).powi(2)).sqrt();
    smoothstep_range(1.08, 0.0, normalized)
}

fn band_footprint(along: f32, across: f32, half_length: f32, half_width: f32) -> f32 {
    if half_length <= f32::EPSILON || half_width <= f32::EPSILON {
        return 0.0;
    }

    let along_mask =
        smoothstep_range(1.12, 0.78, along.abs() / half_length.max(f32::EPSILON));
    let across_mask =
        smoothstep_range(1.10, 0.0, across.abs() / half_width.max(f32::EPSILON));
    along_mask * across_mask
}

fn sample_atlas_fields_fractional(
    fields: &super::atlas_fields::AtlasFieldMap,
    atlas_x: f32,
    atlas_z: f32,
) -> AtlasCell {
    let area = fields.area();
    let min_x = area.origin().x;
    let min_z = area.origin().z;
    let max_x = area.origin().x + area.width() as i32 - 1;
    let max_z = area.origin().z + area.height() as i32 - 1;
    let base_x = atlas_x.floor() as i32;
    let base_z = atlas_z.floor() as i32;
    let clamped_base_x = base_x.clamp(min_x, max_x);
    let clamped_base_z = base_z.clamp(min_z, max_z);
    let east_x = (clamped_base_x + 1).min(max_x);
    let south_z = (clamped_base_z + 1).min(max_z);
    let frac_x = atlas_x - clamped_base_x as f32;
    let frac_z = atlas_z - clamped_base_z as f32;
    let c00 = fields
        .get(AtlasCoord::new(clamped_base_x, clamped_base_z))
        .expect("meso atlas sample must exist");
    let c10 = fields
        .get(AtlasCoord::new(east_x, clamped_base_z))
        .expect("meso atlas east sample must exist");
    let c01 = fields
        .get(AtlasCoord::new(clamped_base_x, south_z))
        .expect("meso atlas south sample must exist");
    let c11 = fields
        .get(AtlasCoord::new(east_x, south_z))
        .expect("meso atlas southeast sample must exist");

    AtlasCell {
        landness: bilerp(c00.landness, c10.landness, c01.landness, c11.landness, frac_x, frac_z),
        ocean_distance: bilerp(
            c00.ocean_distance,
            c10.ocean_distance,
            c01.ocean_distance,
            c11.ocean_distance,
            frac_x,
            frac_z,
        ),
        coast_factor: bilerp(
            c00.coast_factor,
            c10.coast_factor,
            c01.coast_factor,
            c11.coast_factor,
            frac_x,
            frac_z,
        ),
        continent_core_factor: bilerp(
            c00.continent_core_factor,
            c10.continent_core_factor,
            c01.continent_core_factor,
            c11.continent_core_factor,
            frac_x,
            frac_z,
        ),
        macro_elevation: bilerp(
            c00.macro_elevation,
            c10.macro_elevation,
            c01.macro_elevation,
            c11.macro_elevation,
            frac_x,
            frac_z,
        ),
        ridge_factor: bilerp(
            c00.ridge_factor,
            c10.ridge_factor,
            c01.ridge_factor,
            c11.ridge_factor,
            frac_x,
            frac_z,
        ),
        mountain_mass: bilerp(
            c00.mountain_mass,
            c10.mountain_mass,
            c01.mountain_mass,
            c11.mountain_mass,
            frac_x,
            frac_z,
        ),
        basinness: bilerp(
            c00.basinness,
            c10.basinness,
            c01.basinness,
            c11.basinness,
            frac_x,
            frac_z,
        ),
        pass_potential: bilerp(
            c00.pass_potential,
            c10.pass_potential,
            c01.pass_potential,
            c11.pass_potential,
            frac_x,
            frac_z,
        ),
        ruggedness: bilerp(
            c00.ruggedness,
            c10.ruggedness,
            c01.ruggedness,
            c11.ruggedness,
            frac_x,
            frac_z,
        ),
        river_source_potential: bilerp(
            c00.river_source_potential,
            c10.river_source_potential,
            c01.river_source_potential,
            c11.river_source_potential,
            frac_x,
            frac_z,
        ),
        river_flow_potential: bilerp(
            c00.river_flow_potential,
            c10.river_flow_potential,
            c01.river_flow_potential,
            c11.river_flow_potential,
            frac_x,
            frac_z,
        ),
        riverine_factor: bilerp(
            c00.riverine_factor,
            c10.riverine_factor,
            c01.riverine_factor,
            c11.riverine_factor,
            frac_x,
            frac_z,
        ),
        lake_potential: bilerp(
            c00.lake_potential,
            c10.lake_potential,
            c01.lake_potential,
            c11.lake_potential,
            frac_x,
            frac_z,
        ),
        temperature: bilerp(
            c00.temperature,
            c10.temperature,
            c01.temperature,
            c11.temperature,
            frac_x,
            frac_z,
        ),
        humidity: bilerp(
            c00.humidity,
            c10.humidity,
            c01.humidity,
            c11.humidity,
            frac_x,
            frac_z,
        ),
        inlandness: bilerp(
            c00.inlandness,
            c10.inlandness,
            c01.inlandness,
            c11.inlandness,
            frac_x,
            frac_z,
        ),
        aridity: bilerp(
            c00.aridity,
            c10.aridity,
            c01.aridity,
            c11.aridity,
            frac_x,
            frac_z,
        ),
        wetness: bilerp(
            c00.wetness,
            c10.wetness,
            c01.wetness,
            c11.wetness,
            frac_x,
            frac_z,
        ),
        polar_factor: bilerp(
            c00.polar_factor,
            c10.polar_factor,
            c01.polar_factor,
            c11.polar_factor,
            frac_x,
            frac_z,
        ),
        alpine_factor: bilerp(
            c00.alpine_factor,
            c10.alpine_factor,
            c01.alpine_factor,
            c11.alpine_factor,
            frac_x,
            frac_z,
        ),
        thermal: c00.thermal,
        moisture: c00.moisture,
        form: c00.form,
        overlay: c00.overlay,
        cover: c00.cover,
        coast_distance: c00.coast_distance,
        continent_id: c00.continent_id,
        slope: c00.slope,
        river_distance_estimate: c00.river_distance_estimate,
        wetland_factor: c00.wetland_factor,
        ecotone_strength: c00.ecotone_strength,
    }
}

fn sample_structure_context(structure: &AtlasStructureMap, point: (f32, f32)) -> StructureContext {
    let mut context = StructureContext::default();
    let mut best_ridge_score = f32::INFINITY;
    let mut best_channel_score = f32::INFINITY;
    let mut best_confluence_score = f32::INFINITY;

    for segment in structure.mountain_chains().segments() {
        let projection = project_point_onto_segment(point, segment.start, segment.end);
        let influence_radius = match segment.scale {
            super::structure::MountainChainScale::Major => 0.55 + segment.half_width_cells * 1.15,
            super::structure::MountainChainScale::Minor => 0.34 + segment.half_width_cells * 0.72,
        };
        let score = projection.distance_cells / influence_radius.max(f32::EPSILON);
        if score < best_ridge_score {
            best_ridge_score = score;
            context.ridge_weight = (1.0 - score).clamp(0.0, 1.0);
            context.ridge_heading_x = projection.heading_x;
            context.ridge_heading_z = projection.heading_z;
        }
    }

    for segment in structure.drainage().segments() {
        let projection = project_point_onto_segment(point, segment.start, segment.end);
        let influence_radius =
            0.070 + segment.bankfull_width_cells * 0.030 + segment.order as f32 * 0.016;
        let score = projection.distance_cells / influence_radius.max(f32::EPSILON);
        if score < best_channel_score {
            best_channel_score = score;
            context.channel_weight = (1.0 - score).clamp(0.0, 1.0);
            context.channel_heading_x = projection.heading_x;
            context.channel_heading_z = projection.heading_z;
        }
    }

    for node in structure.drainage().nodes() {
        if node.kind != DrainageNodeKind::Confluence {
            continue;
        }
        let distance = point_distance_to_node(point, node.coord);
        let score = distance / (0.08 + node.order as f32 * 0.035).max(f32::EPSILON);
        if score < best_confluence_score {
            best_confluence_score = score;
            context.confluence_weight = (1.0 - score).clamp(0.0, 1.0);
        }
    }

    context
}

#[derive(Debug, Clone, Copy)]
struct SegmentProjection {
    distance_cells: f32,
    heading_x: f32,
    heading_z: f32,
}

fn project_point_onto_segment(
    point: (f32, f32),
    start: AtlasCoord,
    end: AtlasCoord,
) -> SegmentProjection {
    let start_x = start.x as f32 + 0.5;
    let start_z = start.z as f32 + 0.5;
    let end_x = end.x as f32 + 0.5;
    let end_z = end.z as f32 + 0.5;
    let seg_x = end_x - start_x;
    let seg_z = end_z - start_z;
    let length_sq = seg_x * seg_x + seg_z * seg_z;

    if length_sq <= f32::EPSILON {
        return SegmentProjection {
            distance_cells: ((point.0 - start_x).powi(2) + (point.1 - start_z).powi(2)).sqrt(),
            heading_x: 1.0,
            heading_z: 0.0,
        };
    }

    let t = (((point.0 - start_x) * seg_x + (point.1 - start_z) * seg_z) / length_sq)
        .clamp(0.0, 1.0);
    let nearest_x = start_x + seg_x * t;
    let nearest_z = start_z + seg_z * t;
    let length = length_sq.sqrt();

    SegmentProjection {
        distance_cells: ((point.0 - nearest_x).powi(2) + (point.1 - nearest_z).powi(2)).sqrt(),
        heading_x: seg_x / length.max(f32::EPSILON),
        heading_z: seg_z / length.max(f32::EPSILON),
    }
}

fn point_distance_to_node(point: (f32, f32), coord: AtlasCoord) -> f32 {
    let node_x = coord.x as f32 + 0.5;
    let node_z = coord.z as f32 + 0.5;
    ((point.0 - node_x).powi(2) + (point.1 - node_z).powi(2)).sqrt()
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

fn bilerp(a00: f32, a10: f32, a01: f32, a11: f32, tx: f32, tz: f32) -> f32 {
    let north = lerp_f32(a00, a10, tx);
    let south = lerp_f32(a01, a11, tx);
    lerp_f32(north, south, tz)
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn smoothstep_range(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }

    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hash01(seed: u64, x: i64, z: i64, salt: u64) -> f32 {
    let (wx, wz) = domain_warp(seed ^ salt, x as f64, z as f64, 1.0 / 6.0, 1.0);
    fbm(seed ^ salt, wx / 11.0, wz / 11.0, 2, 2.0, 0.5, salt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{generate_atlas_fields, generate_atlas_structure};

    #[test]
    fn meso_generation_is_deterministic() {
        let meta = WorldMeta::new(42);
        let area = AtlasArea::new(AtlasCoord::new(-6, -6), 12, 12).unwrap();
        let fields = generate_atlas_fields(&meta, area);
        let structure = generate_atlas_structure(&meta, area);

        let first = generate_meso_guides(&meta, area, &fields, &structure);
        let second = generate_meso_guides(&meta, area, &fields, &structure);

        assert_eq!(first, second);
    }

    #[test]
    fn meso_generation_emits_wave_one_guides_for_large_area() {
        let meta = WorldMeta::new(42);
        let area = AtlasArea::new(AtlasCoord::new(-8, -8), 16, 16).unwrap();
        let fields = generate_atlas_fields(&meta, area);
        let structure = generate_atlas_structure(&meta, area);
        let guides = generate_meso_guides(&meta, area, &fields, &structure);

        assert!(guides.cells.values().iter().any(|cell| {
            cell.hilliness > 0.0
                || cell.basin_weight > 0.0
                || cell.escarpment_weight > 0.0
                || cell.terrace_weight > 0.0
        }));
    }
}
