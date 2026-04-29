use std::collections::HashMap;

use crate::world::atlas::{
    AtlasCell, BiomeFamily, CoastalContext, HydrologyContext, RegionArchetype, RegionClassCell,
    TerrainFormFamily,
};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};

use super::prototype::{basis_parameters_to_realization_sample, blended_basis_parameters};
use super::{ChunkGenerationInputs, sample_atlas_fields_fractional, sample_region_weights};

pub const REALIZATION_NODE_CHUNK_SPAN: i32 = 2;
pub const REALIZATION_NODE_BLOCK_SPAN: u32 = (REALIZATION_NODE_CHUNK_SPAN * CHUNK_EDGE_I32) as u32;

const REALIZATION_NODE_BLOCK_SPAN_F32: f32 = REALIZATION_NODE_BLOCK_SPAN as f32;
const REALIZATION_PATCH_HALO_NODES: i32 = 2;
const REALIZATION_DIFFUSION_RADIUS_NODES: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RealizationMaterialSupport {
    pub wetness: f32,
    pub exposure: f32,
    pub sediment: f32,
    pub soil_cover: f32,
}

impl Default for RealizationMaterialSupport {
    fn default() -> Self {
        Self {
            wetness: 0.0,
            exposure: 0.0,
            sediment: 0.0,
            soil_cover: 0.0,
        }
    }
}

impl RealizationMaterialSupport {
    pub fn finalized(mut self) -> Self {
        self.wetness = self.wetness.clamp(0.0, 1.0);
        self.exposure = self.exposure.clamp(0.0, 1.0);
        self.sediment = self.sediment.clamp(0.0, 1.0);
        self.soil_cover = self.soil_cover.clamp(0.0, 1.0);
        self
    }

    fn add_weighted(&mut self, other: Self, weight: f32) {
        self.wetness += other.wetness * weight;
        self.exposure += other.exposure * weight;
        self.sediment += other.sediment * weight;
        self.soil_cover += other.soil_cover * weight;
    }

    fn scaled(mut self, factor: f32) -> Self {
        self.wetness *= factor;
        self.exposure *= factor;
        self.sediment *= factor;
        self.soil_cover *= factor;
        self
    }

    fn blend_toward(self, anchor: Self, amount: f32) -> Self {
        let t = amount.clamp(0.0, 1.0);
        Self {
            wetness: lerp_f32(self.wetness, anchor.wetness, t),
            exposure: lerp_f32(self.exposure, anchor.exposure, t),
            sediment: lerp_f32(self.sediment, anchor.sediment, t),
            soil_cover: lerp_f32(self.soil_cover, anchor.soil_cover, t),
        }
        .finalized()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RealizationSample {
    pub macro_height_bonus: f32,
    pub coastal_shelf_depth: f32,
    pub coastal_apron_lift: f32,
    pub coastal_cliff_lift: f32,
    pub ridge_lift: f32,
    pub ridge_shoulder_lift: f32,
    pub basin_depth: f32,
    pub inland_lift: f32,
    pub arid_lift: f32,
    pub wet_flatten: f32,
    pub low_freq_amp: f32,
    pub mid_freq_amp: f32,
    pub terrace_amp: f32,
    pub dune_amp: f32,
    pub relief_base: f32,
    pub relief_gain: f32,
    pub corridor_depth: f32,
    pub corridor_width_scale: f32,
    pub floodplain_width_scale: f32,
    pub outlet_open_scale: f32,
    pub ridge_preservation: f32,
    pub meso_relief_reserve: f32,
    pub material_support: RealizationMaterialSupport,
}

impl Default for RealizationSample {
    fn default() -> Self {
        Self {
            macro_height_bonus: 0.0,
            coastal_shelf_depth: 0.0,
            coastal_apron_lift: 0.0,
            coastal_cliff_lift: 0.0,
            ridge_lift: 0.0,
            ridge_shoulder_lift: 0.0,
            basin_depth: 0.0,
            inland_lift: 0.0,
            arid_lift: 0.0,
            wet_flatten: 0.0,
            low_freq_amp: 0.0,
            mid_freq_amp: 0.0,
            terrace_amp: 0.0,
            dune_amp: 0.0,
            relief_base: 0.0,
            relief_gain: 0.0,
            corridor_depth: 0.0,
            corridor_width_scale: 0.0,
            floodplain_width_scale: 0.0,
            outlet_open_scale: 0.0,
            ridge_preservation: 0.0,
            meso_relief_reserve: 0.0,
            material_support: RealizationMaterialSupport::default(),
        }
    }
}

impl RealizationSample {
    fn add_weighted(&mut self, other: Self, weight: f32) {
        self.macro_height_bonus += other.macro_height_bonus * weight;
        self.coastal_shelf_depth += other.coastal_shelf_depth * weight;
        self.coastal_apron_lift += other.coastal_apron_lift * weight;
        self.coastal_cliff_lift += other.coastal_cliff_lift * weight;
        self.ridge_lift += other.ridge_lift * weight;
        self.ridge_shoulder_lift += other.ridge_shoulder_lift * weight;
        self.basin_depth += other.basin_depth * weight;
        self.inland_lift += other.inland_lift * weight;
        self.arid_lift += other.arid_lift * weight;
        self.wet_flatten += other.wet_flatten * weight;
        self.low_freq_amp += other.low_freq_amp * weight;
        self.mid_freq_amp += other.mid_freq_amp * weight;
        self.terrace_amp += other.terrace_amp * weight;
        self.dune_amp += other.dune_amp * weight;
        self.relief_base += other.relief_base * weight;
        self.relief_gain += other.relief_gain * weight;
        self.corridor_depth += other.corridor_depth * weight;
        self.corridor_width_scale += other.corridor_width_scale * weight;
        self.floodplain_width_scale += other.floodplain_width_scale * weight;
        self.outlet_open_scale += other.outlet_open_scale * weight;
        self.ridge_preservation += other.ridge_preservation * weight;
        self.meso_relief_reserve += other.meso_relief_reserve * weight;
        self.material_support
            .add_weighted(other.material_support, weight);
    }

    fn scaled(self, factor: f32) -> Self {
        let mut scaled = self;
        scaled.macro_height_bonus *= factor;
        scaled.coastal_shelf_depth *= factor;
        scaled.coastal_apron_lift *= factor;
        scaled.coastal_cliff_lift *= factor;
        scaled.ridge_lift *= factor;
        scaled.ridge_shoulder_lift *= factor;
        scaled.basin_depth *= factor;
        scaled.inland_lift *= factor;
        scaled.arid_lift *= factor;
        scaled.wet_flatten *= factor;
        scaled.low_freq_amp *= factor;
        scaled.mid_freq_amp *= factor;
        scaled.terrace_amp *= factor;
        scaled.dune_amp *= factor;
        scaled.relief_base *= factor;
        scaled.relief_gain *= factor;
        scaled.corridor_depth *= factor;
        scaled.corridor_width_scale *= factor;
        scaled.floodplain_width_scale *= factor;
        scaled.outlet_open_scale *= factor;
        scaled.ridge_preservation *= factor;
        scaled.meso_relief_reserve *= factor;
        scaled.material_support = scaled.material_support.scaled(factor);
        scaled
    }

    fn blend_toward(self, anchor: Self, amount: f32) -> Self {
        let t = amount.clamp(0.0, 1.0);
        Self {
            macro_height_bonus: lerp_f32(self.macro_height_bonus, anchor.macro_height_bonus, t),
            coastal_shelf_depth: lerp_f32(self.coastal_shelf_depth, anchor.coastal_shelf_depth, t),
            coastal_apron_lift: lerp_f32(self.coastal_apron_lift, anchor.coastal_apron_lift, t),
            coastal_cliff_lift: lerp_f32(self.coastal_cliff_lift, anchor.coastal_cliff_lift, t),
            ridge_lift: lerp_f32(self.ridge_lift, anchor.ridge_lift, t),
            ridge_shoulder_lift: lerp_f32(self.ridge_shoulder_lift, anchor.ridge_shoulder_lift, t),
            basin_depth: lerp_f32(self.basin_depth, anchor.basin_depth, t),
            inland_lift: lerp_f32(self.inland_lift, anchor.inland_lift, t),
            arid_lift: lerp_f32(self.arid_lift, anchor.arid_lift, t),
            wet_flatten: lerp_f32(self.wet_flatten, anchor.wet_flatten, t),
            low_freq_amp: lerp_f32(self.low_freq_amp, anchor.low_freq_amp, t),
            mid_freq_amp: lerp_f32(self.mid_freq_amp, anchor.mid_freq_amp, t),
            terrace_amp: lerp_f32(self.terrace_amp, anchor.terrace_amp, t),
            dune_amp: lerp_f32(self.dune_amp, anchor.dune_amp, t),
            relief_base: lerp_f32(self.relief_base, anchor.relief_base, t),
            relief_gain: lerp_f32(self.relief_gain, anchor.relief_gain, t),
            corridor_depth: lerp_f32(self.corridor_depth, anchor.corridor_depth, t),
            corridor_width_scale: lerp_f32(
                self.corridor_width_scale,
                anchor.corridor_width_scale,
                t,
            ),
            floodplain_width_scale: lerp_f32(
                self.floodplain_width_scale,
                anchor.floodplain_width_scale,
                t,
            ),
            outlet_open_scale: lerp_f32(self.outlet_open_scale, anchor.outlet_open_scale, t),
            ridge_preservation: lerp_f32(self.ridge_preservation, anchor.ridge_preservation, t),
            meso_relief_reserve: lerp_f32(self.meso_relief_reserve, anchor.meso_relief_reserve, t),
            material_support: self
                .material_support
                .blend_toward(anchor.material_support, t),
        }
    }

    fn finalized(mut self) -> Self {
        self.coastal_shelf_depth = self.coastal_shelf_depth.max(0.0);
        self.coastal_apron_lift = self.coastal_apron_lift.max(0.0);
        self.coastal_cliff_lift = self.coastal_cliff_lift.max(0.0);
        self.ridge_lift = self.ridge_lift.max(0.0);
        self.ridge_shoulder_lift = self.ridge_shoulder_lift.max(0.0);
        self.basin_depth = self.basin_depth.max(0.0);
        self.low_freq_amp = self.low_freq_amp.max(0.0);
        self.mid_freq_amp = self.mid_freq_amp.max(0.0);
        self.terrace_amp = self.terrace_amp.max(0.0);
        self.dune_amp = self.dune_amp.max(0.0);
        self.relief_base = self.relief_base.max(4.0);
        self.relief_gain = self.relief_gain.max(0.0);
        self.corridor_depth = self.corridor_depth.max(0.0);
        self.corridor_width_scale = self.corridor_width_scale.max(0.75);
        self.floodplain_width_scale = self.floodplain_width_scale.max(1.0);
        self.outlet_open_scale = self.outlet_open_scale.max(1.0);
        self.ridge_preservation = self.ridge_preservation.max(0.0);
        self.meso_relief_reserve = self.meso_relief_reserve.max(0.0);
        self.material_support = self.material_support.finalized();
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RealizationFieldNode {
    pub node_x: i32,
    pub node_z: i32,
    pub source: RealizationSample,
    pub solved: RealizationSample,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkRealizationFieldPatch {
    pub chunk: ChunkCoord,
    pub node_origin_x: i32,
    pub node_origin_z: i32,
    pub width: u32,
    pub height: u32,
    pub node_block_span: u32,
    pub nodes: Vec<RealizationFieldNode>,
}

#[derive(Debug, Clone, Copy)]
struct RealizationSourceHint {
    region: RegionClassCell,
    field: AtlasCell,
    sample: RealizationSample,
    anchor_weight: f32,
}

pub fn empty_chunk_realization_field_patch(chunk: ChunkCoord) -> ChunkRealizationFieldPatch {
    ChunkRealizationFieldPatch {
        chunk,
        node_origin_x: 0,
        node_origin_z: 0,
        width: 0,
        height: 0,
        node_block_span: REALIZATION_NODE_BLOCK_SPAN,
        nodes: Vec::new(),
    }
}

pub fn build_chunk_realization_field_patch(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
) -> ChunkRealizationFieldPatch {
    debug_assert_eq!(inputs.chunk, chunk);

    let (min_node_x, min_node_z, max_node_x, max_node_z) = patch_node_bounds(chunk);
    let width = (max_node_x - min_node_x + 1) as u32;
    let height = (max_node_z - min_node_z + 1) as u32;
    let mut source_cache = HashMap::<(i32, i32), RealizationSourceHint>::new();
    let mut nodes = Vec::with_capacity((width * height) as usize);

    for node_z in min_node_z..=max_node_z {
        for node_x in min_node_x..=max_node_x {
            let source = source_hint_for_node(node_x, node_z, inputs, &mut source_cache);
            let solved = solve_realization_node(node_x, node_z, inputs, &mut source_cache);
            nodes.push(RealizationFieldNode {
                node_x,
                node_z,
                source: source.sample,
                solved,
            });
        }
    }

    ChunkRealizationFieldPatch {
        chunk,
        node_origin_x: min_node_x,
        node_origin_z: min_node_z,
        width,
        height,
        node_block_span: REALIZATION_NODE_BLOCK_SPAN,
        nodes,
    }
}

pub fn sample_chunk_realization_field(
    patch: &ChunkRealizationFieldPatch,
    world_x: f32,
    world_z: f32,
) -> RealizationSample {
    let sample_x = world_x / REALIZATION_NODE_BLOCK_SPAN_F32;
    let sample_z = world_z / REALIZATION_NODE_BLOCK_SPAN_F32;
    let base_x = sample_x.floor() as i32;
    let base_z = sample_z.floor() as i32;
    let east_x = base_x + 1;
    let south_z = base_z + 1;
    let tx = smootherstep01(sample_x - base_x as f32);
    let tz = smootherstep01(sample_z - base_z as f32);
    let s00 = patch
        .sample_at(base_x, base_z)
        .expect("realization base sample must exist");
    let s10 = patch
        .sample_at(east_x, base_z)
        .expect("realization east sample must exist");
    let s01 = patch
        .sample_at(base_x, south_z)
        .expect("realization south sample must exist");
    let s11 = patch
        .sample_at(east_x, south_z)
        .expect("realization southeast sample must exist");

    bilerp_realization_samples(s00, s10, s01, s11, tx, tz).finalized()
}

impl ChunkRealizationFieldPatch {
    fn sample_at(&self, node_x: i32, node_z: i32) -> Option<RealizationSample> {
        let local_x = usize::try_from(node_x - self.node_origin_x).ok()?;
        let local_z = usize::try_from(node_z - self.node_origin_z).ok()?;
        if local_x >= self.width as usize || local_z >= self.height as usize {
            return None;
        }

        self.nodes
            .get(local_z * self.width as usize + local_x)
            .map(|node| node.solved)
    }
}

fn patch_node_bounds(chunk: ChunkCoord) -> (i32, i32, i32, i32) {
    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let min_sample_x = chunk_origin_x as f32 + 0.5;
    let max_sample_x = (chunk_origin_x + CHUNK_EDGE_I32 - 1) as f32 + 0.5;
    let min_sample_z = chunk_origin_z as f32 + 0.5;
    let max_sample_z = (chunk_origin_z + CHUNK_EDGE_I32 - 1) as f32 + 0.5;
    let min_base_x = world_to_realization_node_coord(min_sample_x);
    let max_base_x = world_to_realization_node_coord(max_sample_x);
    let min_base_z = world_to_realization_node_coord(min_sample_z);
    let max_base_z = world_to_realization_node_coord(max_sample_z);

    (
        min_base_x - REALIZATION_PATCH_HALO_NODES,
        min_base_z - REALIZATION_PATCH_HALO_NODES,
        max_base_x + 1 + REALIZATION_PATCH_HALO_NODES,
        max_base_z + 1 + REALIZATION_PATCH_HALO_NODES,
    )
}

fn solve_realization_node(
    node_x: i32,
    node_z: i32,
    inputs: &ChunkGenerationInputs,
    source_cache: &mut HashMap<(i32, i32), RealizationSourceHint>,
) -> RealizationSample {
    let center = source_hint_for_node(node_x, node_z, inputs, source_cache);
    let mut accumulated = RealizationSample::default();
    let mut total_weight = 0.0_f32;

    accumulated.add_weighted(center.sample, center.anchor_weight);
    total_weight += center.anchor_weight;

    for dz in -REALIZATION_DIFFUSION_RADIUS_NODES..=REALIZATION_DIFFUSION_RADIUS_NODES {
        for dx in -REALIZATION_DIFFUSION_RADIUS_NODES..=REALIZATION_DIFFUSION_RADIUS_NODES {
            if dx == 0 && dz == 0 {
                continue;
            }

            let distance = ((dx * dx + dz * dz) as f32).sqrt();
            let kernel =
                smootherstep01(1.0 - distance / (REALIZATION_DIFFUSION_RADIUS_NODES as f32 + 0.35));
            if kernel <= f32::EPSILON {
                continue;
            }

            let neighbor = source_hint_for_node(node_x + dx, node_z + dz, inputs, source_cache);
            let permeability = realization_permeability(center, neighbor);
            let weight = kernel * permeability;
            if weight <= f32::EPSILON {
                continue;
            }

            accumulated.add_weighted(neighbor.sample, weight);
            total_weight += weight;
        }
    }

    if total_weight <= f32::EPSILON {
        return center.sample.finalized();
    }

    let smoothed = accumulated.scaled(total_weight.recip());
    let anchor_pull = (0.28
        + center.field.ecotone_strength * 0.32
        + center.field.coast_factor * 0.18
        + center.field.ridge_factor * 0.10)
        .clamp(0.22, 0.78);

    smoothed
        .blend_toward(center.sample, anchor_pull)
        .finalized()
}

fn source_hint_for_node(
    node_x: i32,
    node_z: i32,
    inputs: &ChunkGenerationInputs,
    source_cache: &mut HashMap<(i32, i32), RealizationSourceHint>,
) -> RealizationSourceHint {
    *source_cache.entry((node_x, node_z)).or_insert_with(|| {
        let world_x = node_world_center(node_x);
        let world_z = node_world_center(node_z);
        let field = sample_atlas_fields_fractional(&inputs.atlas_fields, world_x, world_z);
        let region_samples = sample_region_weights(&inputs.region_classes, world_x, world_z);
        let dominant_region = dominant_region_cell(&region_samples);
        let mut source =
            basis_parameters_to_realization_sample(blended_basis_parameters(&region_samples));
        source.material_support = blended_material_support(&region_samples, field);
        let sample = modulate_source_hint(source, field).finalized();
        let anchor_weight = (1.20
            + field.ecotone_strength * 0.40
            + field.coast_factor * 0.18
            + field.ridge_factor * 0.16
            + field.basinness * 0.08)
            .clamp(1.05, 1.85);

        RealizationSourceHint {
            region: dominant_region,
            field,
            sample,
            anchor_weight,
        }
    })
}

fn dominant_region_cell(region_samples: &[super::RegionSampleWeight]) -> RegionClassCell {
    let mut dominant = RegionClassCell::default();
    let mut best_weight = f32::NEG_INFINITY;

    for sample in region_samples {
        if sample.weight > best_weight {
            dominant = sample.cell;
            best_weight = sample.weight;
        }
    }

    dominant
}

fn blended_material_support(
    region_samples: &[super::RegionSampleWeight],
    field: AtlasCell,
) -> RealizationMaterialSupport {
    let mut support = RealizationMaterialSupport::default();
    let mut total_weight = 0.0_f32;

    for sample in region_samples {
        let weight = sample.weight.max(0.0);
        if weight <= f32::EPSILON {
            continue;
        }

        support.add_weighted(material_support_for_region(sample.cell, field), weight);
        total_weight += weight;
    }

    if total_weight <= f32::EPSILON {
        return material_support_for_region(RegionClassCell::default(), field);
    }

    support.scaled(total_weight.recip()).finalized()
}

fn material_support_for_region(
    region: RegionClassCell,
    field: AtlasCell,
) -> RealizationMaterialSupport {
    let wet_semantic: f32 = if matches!(
        region.hydrology_context,
        HydrologyContext::RiverCorridor
            | HydrologyContext::LakeBasin
            | HydrologyContext::WetLowland
    ) || matches!(
        region.terrain_form_family,
        TerrainFormFamily::Floodplain
            | TerrainFormFamily::WetLowland
            | TerrainFormFamily::AlluvialLowland
            | TerrainFormFamily::EstuaryLowland
            | TerrainFormFamily::Delta
            | TerrainFormFamily::Basin
    ) || matches!(
        region.biome_family,
        BiomeFamily::Marsh
            | BiomeFamily::Swamp
            | BiomeFamily::FloodedForest
            | BiomeFamily::Mangrove
            | BiomeFamily::EstuarineCoast
    ) || matches!(
        region.archetype,
        RegionArchetype::ColdWetLowland
            | RegionArchetype::MarshFloodplain
            | RegionArchetype::SwampLowland
            | RegionArchetype::EstuaryLowland
            | RegionArchetype::CoastalDelta
            | RegionArchetype::MangroveLagoon
            | RegionArchetype::MangroveDelta
            | RegionArchetype::FloodedForestAlluvialLowland
            | RegionArchetype::FloodedForestFloodplain
            | RegionArchetype::MonsoonFloodplain
            | RegionArchetype::MonsoonDelta
            | RegionArchetype::BorealWetLowland
    ) {
        0.76
    } else {
        0.0
    };
    let wet_field = field.wetness * 0.38
        + field.wetland_factor * 0.28
        + field.riverine_factor * 0.18
        + field.lake_potential * 0.16;

    let alpine_semantic: f32 = if matches!(
        region.archetype,
        RegionArchetype::GlaciatedAlpine
            | RegionArchetype::SubalpineWoodedFront
            | RegionArchetype::AlpineMeadowMountain
            | RegionArchetype::GlacialValley
            | RegionArchetype::CrevassedIcefield
            | RegionArchetype::BorealRidgeCountry
            | RegionArchetype::AlpineRavineCountry
    ) || matches!(
        region.terrain_form_family,
        TerrainFormFamily::Mountain
            | TerrainFormFamily::MountainFront
            | TerrainFormFamily::RidgeCountry
            | TerrainFormFamily::GlacialValley
            | TerrainFormFamily::Icefield
            | TerrainFormFamily::CrevassedIcefield
            | TerrainFormFamily::RavineCountry
            | TerrainFormFamily::SeaCliff
            | TerrainFormFamily::RockyShore
    ) || matches!(
        region.biome_family,
        BiomeFamily::AlpineMeadow
            | BiomeFamily::SubalpineWoodland
            | BiomeFamily::PolarIce
            | BiomeFamily::RockyCoast
    ) {
        0.74
    } else {
        0.0
    };
    let exposure_field = field.alpine_factor * 0.28
        + field.ruggedness * 0.24
        + field.ridge_factor * 0.18
        + field.mountain_mass * 0.16
        + field.slope * 0.14;

    let coastal_sediment: f32 = if matches!(
        region.coastal_context,
        CoastalContext::Marine | CoastalContext::Coastal
    ) {
        0.48
    } else {
        0.0
    };
    let dry_sediment: f32 = if matches!(
        region.archetype,
        RegionArchetype::DesertPlain
            | RegionArchetype::DesertDuneField
            | RegionArchetype::DesertBasin
            | RegionArchetype::DesertMesaCountry
            | RegionArchetype::DesertAlluvialFan
            | RegionArchetype::SemiDesertPediment
            | RegionArchetype::DryShrublandBadlands
            | RegionArchetype::DryShrublandKarst
    ) || matches!(
        region.terrain_form_family,
        TerrainFormFamily::DuneField
            | TerrainFormFamily::MesaCountry
            | TerrainFormFamily::Badlands
            | TerrainFormFamily::Pediment
    ) {
        0.42
    } else {
        0.0
    };
    let sediment_field = field.riverine_factor * 0.22
        + field.coast_factor * 0.18
        + field.slope * 0.18
        + field.ruggedness * 0.16
        + field.basinness * 0.12
        + field.aridity * 0.14;

    let green_semantic: f32 = if matches!(
        region.biome_family,
        BiomeFamily::TemperateGrassland
            | BiomeFamily::Savanna
            | BiomeFamily::TropicalRainforest
            | BiomeFamily::MonsoonForest
            | BiomeFamily::TropicalDryForest
            | BiomeFamily::BorealForest
            | BiomeFamily::Tundra
            | BiomeFamily::AlpineMeadow
            | BiomeFamily::SubalpineWoodland
    ) {
        0.68
    } else {
        0.0
    };
    let exposure = alpine_semantic.max(exposure_field).clamp(0.0, 1.0);
    let wetness = wet_semantic.max(wet_field).clamp(0.0, 1.0);
    let sediment = coastal_sediment
        .max(dry_sediment)
        .max(sediment_field)
        .clamp(0.0, 1.0);
    let soil_cover = (green_semantic
        .max(0.32 + field.humidity * 0.28 + field.wetness * 0.10 - field.aridity * 0.18)
        * (1.0 - exposure * 0.46)
        * (1.0 - sediment * 0.16))
        .clamp(0.0, 1.0);

    RealizationMaterialSupport {
        wetness,
        exposure,
        sediment,
        soil_cover,
    }
    .finalized()
}

fn modulate_source_hint(mut sample: RealizationSample, field: AtlasCell) -> RealizationSample {
    let uplift_delta =
        (field.macro_elevation - 0.5) * 8.0 + (field.continent_core_factor - 0.5) * 3.5;
    sample.macro_height_bonus += uplift_delta;
    sample.inland_lift *= 0.88 + field.continent_core_factor * 0.28;
    sample.arid_lift *= 0.84 + field.aridity * 0.30;
    sample.wet_flatten *= 0.80 + field.wetness * 0.34 + field.lake_potential * 0.12;
    sample.low_freq_amp *= 0.84 + field.macro_elevation * 0.10 + field.ruggedness * 0.14;
    sample.mid_freq_amp *= 0.82 + field.ruggedness * 0.26 + field.slope * 0.16;
    sample.terrace_amp *= 0.82 + field.ruggedness * 0.22 + field.slope * 0.18;
    sample.dune_amp *= 0.72 + field.aridity * 0.34;
    sample.relief_base *= 0.88 + field.macro_elevation * 0.10 + field.ruggedness * 0.16;
    sample.relief_gain *= 0.84 + field.ruggedness * 0.30 + field.slope * 0.16;
    sample.ridge_lift *= 0.84 + field.ridge_factor * 0.26 + field.mountain_mass * 0.18;
    sample.ridge_shoulder_lift *= 0.84 + field.ridge_factor * 0.18 + field.mountain_mass * 0.14;
    sample.ridge_preservation *= 0.88 + field.ridge_factor * 0.16 + field.ruggedness * 0.10;
    sample.basin_depth *= 0.86 + field.basinness * 0.28 + field.lake_potential * 0.10;
    sample.corridor_depth *=
        0.88 + field.river_flow_potential * 0.18 + field.riverine_factor * 0.08;
    sample.corridor_width_scale *= 0.90 + field.river_flow_potential * 0.12 + field.wetness * 0.06;
    sample.floodplain_width_scale *= 0.90 + field.wetness * 0.16 + field.riverine_factor * 0.12;
    sample.outlet_open_scale *= 0.92 + field.coast_factor * 0.10 + field.basinness * 0.08;
    sample.meso_relief_reserve = (sample.relief_base * 0.65 + sample.relief_gain * 0.35)
        * (0.82 + field.ruggedness * 0.18)
        * (1.0 - field.wetness * 0.12);
    sample.material_support.wetness =
        (sample.material_support.wetness * (0.84 + field.wetness * 0.22)).clamp(0.0, 1.0);
    sample.material_support.exposure = (sample.material_support.exposure
        * (0.82 + field.ruggedness * 0.14 + field.slope * 0.10))
        .clamp(0.0, 1.0);
    sample.material_support.sediment = (sample.material_support.sediment
        * (0.84 + field.riverine_factor * 0.10 + field.coast_factor * 0.10))
        .clamp(0.0, 1.0);
    sample.material_support.soil_cover = (sample.material_support.soil_cover
        * (0.88 + field.humidity * 0.10 - field.aridity * 0.08))
        .clamp(0.0, 1.0);
    sample
}

fn realization_permeability(a: RealizationSourceHint, b: RealizationSourceHint) -> f32 {
    let mut permeability = 0.06_f32;

    if a.region.archetype == b.region.archetype {
        permeability += 0.26;
    }
    if a.region.terrain_form_family == b.region.terrain_form_family {
        permeability += 0.22;
    }
    if a.region.biome_family == b.region.biome_family {
        permeability += 0.16;
    }
    if a.region.hydrology_context == b.region.hydrology_context {
        permeability += 0.10;
    }
    if a.region.coastal_context == b.region.coastal_context {
        permeability += 0.08;
    }

    permeability += scalar_similarity(a.field.macro_elevation, b.field.macro_elevation) * 0.08;
    permeability += scalar_similarity(a.field.wetness, b.field.wetness) * 0.08;
    permeability += scalar_similarity(a.field.ruggedness, b.field.ruggedness) * 0.08;
    permeability += scalar_similarity(a.field.basinness, b.field.basinness) * 0.05;
    permeability += scalar_similarity(a.field.ridge_factor, b.field.ridge_factor) * 0.05;

    let marine_transition = a.region.coastal_context == CoastalContext::Marine
        || b.region.coastal_context == CoastalContext::Marine;
    let inland_transition = a.region.coastal_context == CoastalContext::Inland
        || b.region.coastal_context == CoastalContext::Inland;
    if marine_transition && inland_transition {
        permeability *= 0.18;
    } else if (a.field.coast_factor - b.field.coast_factor).abs() > 0.38 {
        permeability *= 0.55;
    }

    if (a.field.ruggedness - b.field.ruggedness).abs() > 0.34
        && (a.field.ridge_factor - b.field.ridge_factor).abs() > 0.24
    {
        permeability *= 0.70;
    }

    if basin_wall_transition(a.region, b.region) {
        permeability *= 0.62;
    }

    permeability.clamp(0.02, 1.0)
}

fn basin_wall_transition(a: RegionClassCell, b: RegionClassCell) -> bool {
    matches!(
        (a.terrain_form_family, b.terrain_form_family),
        (TerrainFormFamily::Basin, TerrainFormFamily::Escarpment)
            | (TerrainFormFamily::Escarpment, TerrainFormFamily::Basin)
            | (TerrainFormFamily::Basin, TerrainFormFamily::RidgeCountry)
            | (TerrainFormFamily::RidgeCountry, TerrainFormFamily::Basin)
    ) || matches!(
        (a.hydrology_context, b.hydrology_context),
        (HydrologyContext::LakeBasin, HydrologyContext::Dryland)
            | (HydrologyContext::Dryland, HydrologyContext::LakeBasin)
    )
}

fn bilerp_realization_samples(
    s00: RealizationSample,
    s10: RealizationSample,
    s01: RealizationSample,
    s11: RealizationSample,
    tx: f32,
    tz: f32,
) -> RealizationSample {
    let weights = bilerp_weights(tx, tz);
    let mut sample = RealizationSample::default();
    sample.add_weighted(s00, weights.0);
    sample.add_weighted(s10, weights.1);
    sample.add_weighted(s01, weights.2);
    sample.add_weighted(s11, weights.3);
    sample.finalized()
}

fn bilerp_weights(tx: f32, tz: f32) -> (f32, f32, f32, f32) {
    let smooth_x = smootherstep01(tx);
    let smooth_z = smootherstep01(tz);
    let inv_x = 1.0 - smooth_x;
    let inv_z = 1.0 - smooth_z;
    (
        inv_x * inv_z,
        smooth_x * inv_z,
        inv_x * smooth_z,
        smooth_x * smooth_z,
    )
}

fn world_to_realization_node_coord(world: f32) -> i32 {
    (world / REALIZATION_NODE_BLOCK_SPAN_F32).floor() as i32
}

fn node_world_center(node: i32) -> f32 {
    node as f32 * REALIZATION_NODE_BLOCK_SPAN_F32 + REALIZATION_NODE_BLOCK_SPAN_F32 * 0.5
}

fn scalar_similarity(a: f32, b: f32) -> f32 {
    (1.0 - (a - b).abs()).clamp(0.0, 1.0)
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn smootherstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::prepare_chunk_generation_inputs;
    use crate::world::meta::WorldMeta;

    #[test]
    #[ignore = "slow generation realization-field pipeline smoke test"]
    fn realization_field_patch_is_deterministic() {
        let meta = WorldMeta::new(42);
        let chunk = ChunkCoord(4, 0, -3);
        let inputs = prepare_chunk_generation_inputs(chunk, &meta);

        let a = build_chunk_realization_field_patch(chunk, &inputs);
        let b = build_chunk_realization_field_patch(chunk, &inputs);

        assert_eq!(a, b);
        assert_eq!(a.chunk, chunk);
        assert!(!a.nodes.is_empty());
    }

    #[test]
    #[ignore = "slow generation realization-field seam smoke test"]
    fn realization_field_samples_match_across_neighboring_chunk_contexts() {
        let meta = WorldMeta::new(42);
        let left_chunk = ChunkCoord(127, 0, 0);
        let right_chunk = ChunkCoord(128, 0, 0);
        let left_inputs = prepare_chunk_generation_inputs(left_chunk, &meta);
        let right_inputs = prepare_chunk_generation_inputs(right_chunk, &meta);
        let left_patch = build_chunk_realization_field_patch(left_chunk, &left_inputs);
        let right_patch = build_chunk_realization_field_patch(right_chunk, &right_inputs);
        let boundary_world_x = right_chunk.0 * CHUNK_EDGE_I32;

        for offset_z in 0..CHUNK_EDGE_I32 {
            let world_z = offset_z as f32 + 0.5;
            let left =
                sample_chunk_realization_field(&left_patch, boundary_world_x as f32 - 0.5, world_z);
            let right = sample_chunk_realization_field(
                &right_patch,
                boundary_world_x as f32 + 0.5,
                world_z,
            );

            assert!(
                (left.relief_base - right.relief_base).abs() <= 2.5
                    && (left.relief_gain - right.relief_gain).abs() <= 2.5
                    && (left.wet_flatten - right.wet_flatten).abs() <= 1.5,
                "realization field drifted too sharply across neighboring chunk contexts at z={offset_z}\nleft={left:?}\nright={right:?}"
            );
        }
    }
}
