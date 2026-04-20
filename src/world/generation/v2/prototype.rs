use crate::world::atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasCell, AtlasStructureMap, CoastalContext, HydrologyContext,
    MountainChainScale, MountainSpineSegment, PrototypeArchetypeHint, RegionArchetype,
    RegionClassCell, RiverPathKind, TerrainFormFamily, region_archetype_prototype_hint,
};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};

use super::super::{SEA_LEVEL_Y, WORLD_FLOOR_Y};
use super::{
    ChunkCorridorWindow, ChunkGenerationV2Inputs, RegionSampleWeight, RiverCorridorConstraint,
    sample_atlas_fields_fractional, sample_region_weights,
};

const MIN_BASE_HEIGHT_Y: f32 = WORLD_FLOOR_Y as f32 + 8.0;
const MAX_BASE_HEIGHT_Y: f32 = SEA_LEVEL_Y as f32 + 192.0;
const MIN_RELIEF_BUDGET: f32 = 4.0;
const MAX_RELIEF_BUDGET: f32 = 40.0;
const ATLAS_CELL_BLOCK_SPAN: f32 = (ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32) as f32;
const DETAIL_HASH_K1: u64 = 0x9E37_79B9_7F4A_7C15;
const DETAIL_HASH_K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const DETAIL_HASH_K3: u64 = 0x1656_67B1_9E37_79F9;
const DETAIL_SALT_LOW_FREQ: u64 = 0xD311_A501_1000_0001;
const DETAIL_SALT_ORIENTED_LOW: u64 = 0xD311_A501_1000_0002;
const DETAIL_SALT_MID_FREQ: u64 = 0xD311_A501_1000_0003;
const DETAIL_SALT_FLAT_FREQ: u64 = 0xD311_A501_1000_0004;
const DETAIL_SALT_DUNE_FREQ: u64 = 0xD311_A501_1000_0005;
const DETAIL_SALT_WARP_X: u64 = 0xD311_A501_2000_0001;
const DETAIL_SALT_WARP_Z: u64 = 0xD311_A501_2000_0002;
const DETAIL_SALT_ORIENT_WARP_X: u64 = 0xD311_A501_2000_0003;
const DETAIL_SALT_ORIENT_WARP_Z: u64 = 0xD311_A501_2000_0004;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrototypePolicyFamily {
    MarineCoastalEdge,
    LowlandBasin,
    OpenPlain,
    HillCountry,
    PlateauEscarpment,
    AridPlain,
    DuneBody,
    AlpineHighRelief,
}

#[derive(Debug, Clone, Copy, Default)]
struct BasisParameters {
    macro_height_bonus: f32,
    coastal_shelf_depth: f32,
    coastal_apron_lift: f32,
    coastal_cliff_lift: f32,
    ridge_lift: f32,
    ridge_shoulder_lift: f32,
    basin_depth: f32,
    inland_lift: f32,
    arid_lift: f32,
    wet_flatten: f32,
    low_freq_amp: f32,
    mid_freq_amp: f32,
    terrace_amp: f32,
    dune_amp: f32,
    relief_base: f32,
    relief_gain: f32,
    corridor_depth: f32,
    corridor_width_scale: f32,
    floodplain_width_scale: f32,
    outlet_open_scale: f32,
    ridge_preservation: f32,
}

impl BasisParameters {
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
    }

    fn finalize(mut self) -> Self {
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
        self.relief_base = self.relief_base.max(MIN_RELIEF_BUDGET);
        self.relief_gain = self.relief_gain.max(0.0);
        self.corridor_depth = self.corridor_depth.max(0.0);
        self.corridor_width_scale = self.corridor_width_scale.max(0.75);
        self.floodplain_width_scale = self.floodplain_width_scale.max(1.0);
        self.outlet_open_scale = self.outlet_open_scale.max(1.0);
        self.ridge_preservation = self.ridge_preservation.max(0.0);
        self
    }
}

#[derive(Debug, Clone, Copy)]
struct StructureBasisSample {
    ridge_core_influence: f32,
    ridge_shoulder_influence: f32,
    major_ridge_influence: f32,
    strongest_segment: f32,
    heading_x: f32,
    heading_z: f32,
}

impl Default for StructureBasisSample {
    fn default() -> Self {
        Self {
            ridge_core_influence: 0.0,
            ridge_shoulder_influence: 0.0,
            major_ridge_influence: 0.0,
            strongest_segment: 0.0,
            heading_x: 1.0,
            heading_z: 0.0,
        }
    }
}

impl StructureBasisSample {
    fn heading(self) -> (f32, f32) {
        (self.heading_x, self.heading_z)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct CorridorModeWeights {
    valley: f32,
    floodplain: f32,
    basin_outlet: f32,
    coastal_exit: f32,
}

impl CorridorModeWeights {
    fn normalized(mut self) -> Self {
        let total = self.valley + self.floodplain + self.basin_outlet + self.coastal_exit;
        if total <= f32::EPSILON {
            self.valley = 1.0;
            return self;
        }

        self.valley /= total;
        self.floodplain /= total;
        self.basin_outlet /= total;
        self.coastal_exit /= total;
        self
    }
}

#[derive(Debug, Clone, Copy)]
struct CorridorPolicy {
    valley_depth: f32,
    floodplain_depth: f32,
    basin_depth: f32,
    coastal_depth: f32,
    valley_width_scale: f32,
    floodplain_width_scale: f32,
    basin_width_scale: f32,
    coastal_width_scale: f32,
    shoulder_preservation: f32,
    relief_penalty_scale: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct CorridorAdjustment {
    height_delta: f32,
    relief_budget_penalty: f32,
    strongest_influence: f32,
    blend_weight: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CorridorBranchKey {
    river_id: u32,
    kind: RiverPathKind,
    order: u8,
}

#[derive(Debug, Clone, Copy)]
struct CorridorBranchResponse {
    key: CorridorBranchKey,
    response: CorridorAdjustment,
}

#[derive(Debug, Clone, Copy)]
struct ProjectedSegmentPoint {
    distance_blocks: f32,
    tangent_x: f32,
    tangent_z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrototypeColumn {
    pub base_height: f32,
    pub relief_budget: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BaseHeightfieldPrototype {
    pub chunk: ChunkCoord,
    pub columns: Vec<PrototypeColumn>,
}

pub fn empty_base_heightfield_prototype(chunk: ChunkCoord) -> BaseHeightfieldPrototype {
    BaseHeightfieldPrototype {
        chunk,
        columns: Vec::new(),
    }
}

pub fn build_chunk_base_heightfield_prototype(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
    corridor_window: &ChunkCorridorWindow,
) -> BaseHeightfieldPrototype {
    debug_assert_eq!(inputs.chunk, chunk);
    debug_assert_eq!(corridor_window.chunk, chunk);
    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let mut columns = Vec::with_capacity((CHUNK_EDGE_I32 as usize) * (CHUNK_EDGE_I32 as usize));

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let world_x = chunk_origin_x + local_x;
            let world_z = chunk_origin_z + local_z;
            let sample_world_x = world_x as f32 + 0.5;
            let sample_world_z = world_z as f32 + 0.5;
            let field = sample_atlas_fields_fractional(&inputs.atlas_fields, sample_world_x, sample_world_z);
            let region_samples = sample_region_weights(&inputs.region_classes, sample_world_x, sample_world_z);
            let structure_sample =
                sample_structure_basis(&inputs.atlas_structure, sample_world_x, sample_world_z);
            let mut base_height = blended_base_height(
                &region_samples,
                field,
                structure_sample,
                sample_world_x,
                sample_world_z,
            );
            let corridor_adjustment = corridor_adjustment_for_column(
                field,
                structure_sample,
                local_x as f32 + 0.5,
                local_z as f32 + 0.5,
                &region_samples,
                &corridor_window.corridors,
            );
            base_height += corridor_adjustment.height_delta;
            base_height = base_height.clamp(MIN_BASE_HEIGHT_Y, MAX_BASE_HEIGHT_Y);
            base_height = snap_base_height_to_block_y(base_height);

            let relief_budget = blended_relief_budget(
                &region_samples,
                field,
                structure_sample,
                corridor_adjustment.strongest_influence,
                corridor_adjustment.relief_budget_penalty,
            );

            columns.push(PrototypeColumn {
                base_height,
                relief_budget,
            });
        }
    }

    BaseHeightfieldPrototype { chunk, columns }
}

fn blended_base_height(
    region_samples: &[RegionSampleWeight; 4],
    field: AtlasCell,
    structure: StructureBasisSample,
    world_x: f32,
    world_z: f32,
) -> f32 {
    let params = blended_basis_parameters(region_samples);
    let inland_signal = (field.continent_core_factor * 0.58 + field.inlandness * 0.42).clamp(0.0, 1.0);
    let wet_signal =
        (field.wetness * 0.56 + field.riverine_factor * 0.24 + field.lake_potential * 0.20)
            .clamp(0.0, 1.0);
    let arid_signal =
        (field.aridity * 0.72 + field.slope * 0.18 + field.ruggedness * 0.10).clamp(0.0, 1.0);
    let macro_base =
        macro_elevation_to_world_y(field.macro_elevation) + field.macro_elevation * params.macro_height_bonus
            - 6.0;
    let macro_shape =
        inland_signal * params.inland_lift + arid_signal * params.arid_lift - wet_signal * params.wet_flatten;
    let coast_term = coastal_basis(field, structure, params);
    let ridge_term = ridge_basis(field, structure, params);
    let basin_term = basin_basis(field, params);
    let detail_term = detail_basis(field, structure, params, world_x, world_z, wet_signal);

    (macro_base + macro_shape + coast_term + ridge_term + basin_term + detail_term)
        .clamp(MIN_BASE_HEIGHT_Y, MAX_BASE_HEIGHT_Y)
}

fn blended_relief_budget(
    region_samples: &[RegionSampleWeight; 4],
    field: AtlasCell,
    structure: StructureBasisSample,
    strongest_corridor_influence: f32,
    corridor_penalty: f32,
) -> f32 {
    let params = blended_basis_parameters(region_samples);
    let base_budget = params.relief_base
        + field.ruggedness * params.relief_gain
        + field.slope * (params.relief_gain * 0.52)
        + structure.ridge_shoulder_influence * 5.5
        + structure.ridge_core_influence * 3.0
        + field.aridity * 2.5
        - field.wetness * 3.8
        - field.riverine_factor * 2.2
        - strongest_corridor_influence * 4.2
        - corridor_penalty;

    base_budget.clamp(MIN_RELIEF_BUDGET, MAX_RELIEF_BUDGET)
}

fn snap_base_height_to_block_y(base_height: f32) -> f32 {
    base_height.round().clamp(MIN_BASE_HEIGHT_Y, MAX_BASE_HEIGHT_Y)
}

fn blended_basis_parameters(region_samples: &[RegionSampleWeight; 4]) -> BasisParameters {
    let mut params = BasisParameters::default();
    let mut total_weight = 0.0;

    for sample in region_samples {
        if sample.weight <= f32::EPSILON {
            continue;
        }
        params.add_weighted(basis_parameters_for_region(sample.cell), sample.weight);
        total_weight += sample.weight;
    }

    if total_weight <= f32::EPSILON {
        return basis_parameters_for_region(RegionClassCell::default()).finalize();
    }

    let inv = total_weight.recip();
    params.macro_height_bonus *= inv;
    params.coastal_shelf_depth *= inv;
    params.coastal_apron_lift *= inv;
    params.coastal_cliff_lift *= inv;
    params.ridge_lift *= inv;
    params.ridge_shoulder_lift *= inv;
    params.basin_depth *= inv;
    params.inland_lift *= inv;
    params.arid_lift *= inv;
    params.wet_flatten *= inv;
    params.low_freq_amp *= inv;
    params.mid_freq_amp *= inv;
    params.terrace_amp *= inv;
    params.dune_amp *= inv;
    params.relief_base *= inv;
    params.relief_gain *= inv;
    params.corridor_depth *= inv;
    params.corridor_width_scale *= inv;
    params.floodplain_width_scale *= inv;
    params.outlet_open_scale *= inv;
    params.ridge_preservation *= inv;
    params.finalize()
}

fn basis_parameters_for_family(family: PrototypePolicyFamily) -> BasisParameters {
    match family {
        PrototypePolicyFamily::MarineCoastalEdge => BasisParameters {
            macro_height_bonus: -34.0,
            coastal_shelf_depth: 16.0,
            coastal_apron_lift: 6.0,
            coastal_cliff_lift: 20.0,
            ridge_lift: 4.0,
            ridge_shoulder_lift: 5.0,
            basin_depth: 2.0,
            inland_lift: 1.0,
            arid_lift: 0.5,
            wet_flatten: 6.0,
            low_freq_amp: 2.5,
            mid_freq_amp: 1.5,
            terrace_amp: 0.5,
            dune_amp: 0.0,
            relief_base: 12.0,
            relief_gain: 7.0,
            corridor_depth: 4.0,
            corridor_width_scale: 1.20,
            floodplain_width_scale: 1.70,
            outlet_open_scale: 1.65,
            ridge_preservation: 1.6,
        },
        PrototypePolicyFamily::LowlandBasin => BasisParameters {
            macro_height_bonus: -12.0,
            coastal_shelf_depth: 2.0,
            coastal_apron_lift: 1.0,
            coastal_cliff_lift: 2.0,
            ridge_lift: 2.0,
            ridge_shoulder_lift: 2.5,
            basin_depth: 8.0,
            inland_lift: 2.0,
            arid_lift: 1.0,
            wet_flatten: 7.0,
            low_freq_amp: 3.0,
            mid_freq_amp: 1.8,
            terrace_amp: 1.2,
            dune_amp: 0.0,
            relief_base: 14.0,
            relief_gain: 8.0,
            corridor_depth: 5.0,
            corridor_width_scale: 1.25,
            floodplain_width_scale: 1.90,
            outlet_open_scale: 1.60,
            ridge_preservation: 1.3,
        },
        PrototypePolicyFamily::OpenPlain => BasisParameters {
            macro_height_bonus: 0.0,
            coastal_shelf_depth: 1.0,
            coastal_apron_lift: 1.5,
            coastal_cliff_lift: 3.0,
            ridge_lift: 4.0,
            ridge_shoulder_lift: 5.0,
            basin_depth: 4.0,
            inland_lift: 5.0,
            arid_lift: 2.5,
            wet_flatten: 4.0,
            low_freq_amp: 4.5,
            mid_freq_amp: 2.4,
            terrace_amp: 1.6,
            dune_amp: 0.0,
            relief_base: 20.0,
            relief_gain: 10.0,
            corridor_depth: 5.5,
            corridor_width_scale: 1.15,
            floodplain_width_scale: 1.60,
            outlet_open_scale: 1.35,
            ridge_preservation: 1.6,
        },
        PrototypePolicyFamily::HillCountry => BasisParameters {
            macro_height_bonus: 12.0,
            coastal_shelf_depth: 1.0,
            coastal_apron_lift: 0.8,
            coastal_cliff_lift: 4.0,
            ridge_lift: 12.0,
            ridge_shoulder_lift: 10.0,
            basin_depth: 3.0,
            inland_lift: 5.5,
            arid_lift: 2.0,
            wet_flatten: 2.5,
            low_freq_amp: 4.2,
            mid_freq_amp: 3.8,
            terrace_amp: 2.1,
            dune_amp: 0.0,
            relief_base: 24.0,
            relief_gain: 12.0,
            corridor_depth: 6.2,
            corridor_width_scale: 1.00,
            floodplain_width_scale: 1.45,
            outlet_open_scale: 1.25,
            ridge_preservation: 3.2,
        },
        PrototypePolicyFamily::PlateauEscarpment => BasisParameters {
            macro_height_bonus: 18.0,
            coastal_shelf_depth: 1.0,
            coastal_apron_lift: 0.6,
            coastal_cliff_lift: 5.0,
            ridge_lift: 10.0,
            ridge_shoulder_lift: 12.0,
            basin_depth: 2.5,
            inland_lift: 6.5,
            arid_lift: 2.5,
            wet_flatten: 2.0,
            low_freq_amp: 3.8,
            mid_freq_amp: 2.8,
            terrace_amp: 3.4,
            dune_amp: 0.0,
            relief_base: 18.0,
            relief_gain: 11.0,
            corridor_depth: 6.0,
            corridor_width_scale: 0.95,
            floodplain_width_scale: 1.40,
            outlet_open_scale: 1.20,
            ridge_preservation: 3.6,
        },
        PrototypePolicyFamily::AridPlain => BasisParameters {
            macro_height_bonus: 6.0,
            coastal_shelf_depth: 0.5,
            coastal_apron_lift: 0.5,
            coastal_cliff_lift: 2.0,
            ridge_lift: 5.0,
            ridge_shoulder_lift: 4.0,
            basin_depth: 3.0,
            inland_lift: 6.0,
            arid_lift: 6.5,
            wet_flatten: 1.0,
            low_freq_amp: 4.0,
            mid_freq_amp: 2.8,
            terrace_amp: 2.0,
            dune_amp: 2.0,
            relief_base: 21.0,
            relief_gain: 10.0,
            corridor_depth: 4.6,
            corridor_width_scale: 1.10,
            floodplain_width_scale: 1.30,
            outlet_open_scale: 1.25,
            ridge_preservation: 1.4,
        },
        PrototypePolicyFamily::DuneBody => BasisParameters {
            macro_height_bonus: 5.0,
            coastal_shelf_depth: 0.2,
            coastal_apron_lift: 0.2,
            coastal_cliff_lift: 1.0,
            ridge_lift: 1.5,
            ridge_shoulder_lift: 1.5,
            basin_depth: 2.0,
            inland_lift: 4.5,
            arid_lift: 8.0,
            wet_flatten: 0.5,
            low_freq_amp: 3.2,
            mid_freq_amp: 1.2,
            terrace_amp: 0.8,
            dune_amp: 6.5,
            relief_base: 22.0,
            relief_gain: 8.0,
            corridor_depth: 2.6,
            corridor_width_scale: 1.40,
            floodplain_width_scale: 1.60,
            outlet_open_scale: 1.45,
            ridge_preservation: 0.8,
        },
        PrototypePolicyFamily::AlpineHighRelief => BasisParameters {
            macro_height_bonus: 32.0,
            coastal_shelf_depth: 1.0,
            coastal_apron_lift: 0.2,
            coastal_cliff_lift: 6.0,
            ridge_lift: 16.0,
            ridge_shoulder_lift: 14.0,
            basin_depth: 4.5,
            inland_lift: 5.0,
            arid_lift: 1.0,
            wet_flatten: 1.5,
            low_freq_amp: 3.8,
            mid_freq_amp: 4.2,
            terrace_amp: 2.8,
            dune_amp: 0.0,
            relief_base: 16.0,
            relief_gain: 13.0,
            corridor_depth: 6.8,
            corridor_width_scale: 0.95,
            floodplain_width_scale: 1.35,
            outlet_open_scale: 1.20,
            ridge_preservation: 4.0,
        },
    }
}

fn basis_parameters_for_region(region: RegionClassCell) -> BasisParameters {
    let family = prototype_policy_family(region);
    let mut params = basis_parameters_for_family(family);

    match region.terrain_form_family {
        TerrainFormFamily::Delta
        | TerrainFormFamily::Floodplain
        | TerrainFormFamily::WetLowland
        | TerrainFormFamily::AlluvialLowland
        | TerrainFormFamily::EstuaryLowland => {
            params.wet_flatten += 2.8;
            params.floodplain_width_scale += 0.35;
            params.low_freq_amp = (params.low_freq_amp - 0.6).max(0.0);
            params.mid_freq_amp = (params.mid_freq_amp - 0.4).max(0.0);
            params.relief_base = (params.relief_base - 2.0).max(MIN_RELIEF_BUDGET);
        }
        TerrainFormFamily::Basin => {
            params.basin_depth += 5.0;
            params.outlet_open_scale += 0.20;
            params.relief_base = (params.relief_base - 2.0).max(MIN_RELIEF_BUDGET);
        }
        TerrainFormFamily::BroadValley
        | TerrainFormFamily::NarrowValley
        | TerrainFormFamily::GlacialValley => {
            params.corridor_depth += 1.8;
            params.floodplain_width_scale += 0.20;
        }
        TerrainFormFamily::Plateau | TerrainFormFamily::MesaCountry | TerrainFormFamily::Escarpment => {
            params.terrace_amp += 1.2;
            params.ridge_shoulder_lift += 3.5;
            params.ridge_preservation += 0.5;
        }
        TerrainFormFamily::Mountain
        | TerrainFormFamily::RidgeCountry
        | TerrainFormFamily::Canyon
        | TerrainFormFamily::RavineCountry
        | TerrainFormFamily::Icefield
        | TerrainFormFamily::CrevassedIcefield => {
            params.ridge_lift += 5.0;
            params.ridge_shoulder_lift += 2.5;
            params.mid_freq_amp += 0.8;
            params.relief_gain += 2.5;
        }
        TerrainFormFamily::DuneField => {
            params.dune_amp += 2.0;
            params.corridor_depth = (params.corridor_depth - 1.5).max(0.0);
        }
        TerrainFormFamily::MarineShelf
        | TerrainFormFamily::BeachPlain
        | TerrainFormFamily::BarrierCoast
        | TerrainFormFamily::LagoonCoast => {
            params.coastal_shelf_depth += 3.0;
            params.coastal_apron_lift += 1.5;
            params.coastal_cliff_lift = (params.coastal_cliff_lift - 2.0).max(0.0);
        }
        TerrainFormFamily::SeaCliff | TerrainFormFamily::RockyShore | TerrainFormFamily::FjordCoast => {
            params.coastal_cliff_lift += 6.0;
            params.ridge_shoulder_lift += 1.5;
        }
        TerrainFormFamily::AlluvialFan => {
            params.arid_lift += 1.5;
            params.low_freq_amp += 0.8;
        }
        _ => {}
    }

    match region.hydrology_context {
        HydrologyContext::RiverCorridor => {
            params.corridor_depth += 1.4;
            params.floodplain_width_scale += 0.20;
            params.wet_flatten += 1.2;
        }
        HydrologyContext::WetLowland => {
            params.wet_flatten += 2.4;
            params.floodplain_width_scale += 0.22;
            params.relief_base = (params.relief_base - 1.4).max(MIN_RELIEF_BUDGET);
        }
        HydrologyContext::LakeBasin => {
            params.basin_depth += 3.5;
            params.outlet_open_scale += 0.25;
        }
        HydrologyContext::Dryland => {
            params.arid_lift += 1.4;
            params.wet_flatten = (params.wet_flatten - 0.5).max(0.0);
        }
        HydrologyContext::WellDrained => {}
    }

    match region.coastal_context {
        CoastalContext::Marine => {
            params.coastal_shelf_depth += 4.0;
            params.coastal_apron_lift += 1.0;
        }
        CoastalContext::Coastal => {
            params.coastal_apron_lift += 1.0;
            params.outlet_open_scale += 0.15;
        }
        CoastalContext::NearCoast => {
            params.coastal_apron_lift += 0.4;
        }
        CoastalContext::Inland => {}
    }

    if let Some(hint) = region_archetype_prototype_hint(region.archetype) {
        params = apply_archetype_prototype_hint(params, *hint);
    }

    params.finalize()
}

fn apply_archetype_prototype_hint(
    mut params: BasisParameters,
    hint: PrototypeArchetypeHint,
) -> BasisParameters {
    params.macro_height_bonus += hint.macro_height_bonus_delta;
    params.wet_flatten = (params.wet_flatten + hint.wet_flatten_delta).max(0.0);
    params.low_freq_amp *= hint.low_freq_amp_scale.max(0.0);
    params.mid_freq_amp *= hint.mid_freq_amp_scale.max(0.0);
    params.terrace_amp *= hint.terrace_amp_scale.max(0.0);
    params.relief_base *= hint.relief_base_scale.max(0.0);
    params.relief_gain *= hint.relief_gain_scale.max(0.0);
    params.corridor_depth *= hint.corridor_depth_scale.max(0.0);
    params.floodplain_width_scale *= hint.floodplain_width_scale.max(0.0);
    params.ridge_lift *= hint.ridge_lift_scale.max(0.0);
    params.ridge_shoulder_lift *= hint.ridge_shoulder_lift_scale.max(0.0);
    params.ridge_preservation *= hint.ridge_preservation_scale.max(0.0);
    params
}

fn prototype_policy_family(region: RegionClassCell) -> PrototypePolicyFamily {
    match region.archetype {
        RegionArchetype::OceanicShelf
        | RegionArchetype::SandyBeachPlain
        | RegionArchetype::CoastalCliffland
        | RegionArchetype::RockyShoreCoast
        | RegionArchetype::BarrierCoast
        | RegionArchetype::LagoonCoast
        | RegionArchetype::EstuaryLowland
        | RegionArchetype::CoastalDelta
        | RegionArchetype::MangroveLagoon
        | RegionArchetype::MangroveDelta
        | RegionArchetype::MonsoonDelta
        | RegionArchetype::FjordCoast => PrototypePolicyFamily::MarineCoastalEdge,
        RegionArchetype::ColdWetLowland
        | RegionArchetype::TundraPlain
        | RegionArchetype::TropicalRainforestLowland
        | RegionArchetype::MarshFloodplain
        | RegionArchetype::SwampLowland
        | RegionArchetype::FloodedForestAlluvialLowland
        | RegionArchetype::FloodedForestFloodplain
        | RegionArchetype::TemperateBasin
        | RegionArchetype::TemperateBroadValley
        | RegionArchetype::BorealWetLowland
        | RegionArchetype::DesertBasin
        | RegionArchetype::MonsoonFloodplain
        | RegionArchetype::GlacialValley => PrototypePolicyFamily::LowlandBasin,
        RegionArchetype::TemperatePlain
        | RegionArchetype::SavannaPlain
        | RegionArchetype::TemperateRollingPlain
        | RegionArchetype::TemperateBroadleafPlain
        | RegionArchetype::BorealPlain
        | RegionArchetype::PolarBarrensPlain => PrototypePolicyFamily::OpenPlain,
        RegionArchetype::TemperateHills
        | RegionArchetype::TropicalRainforestHills
        | RegionArchetype::SteppeHills
        | RegionArchetype::TemperateMixedHills
        | RegionArchetype::BorealHills
        | RegionArchetype::MediterraneanShrublandHills
        | RegionArchetype::SavannaHills
        | RegionArchetype::TropicalDryForestHills
        | RegionArchetype::SubalpineWoodedFront
        | RegionArchetype::DesertAlluvialFan
        | RegionArchetype::BorealRidgeCountry
        | RegionArchetype::AlpineRavineCountry => PrototypePolicyFamily::HillCountry,
        RegionArchetype::TemperatePlateau
        | RegionArchetype::TemperateEscarpmentUpland
        | RegionArchetype::MonsoonPlateau
        | RegionArchetype::DesertMesaCountry => PrototypePolicyFamily::PlateauEscarpment,
        RegionArchetype::SteppePlain
        | RegionArchetype::DesertPlain
        | RegionArchetype::SemiDesertPediment
        | RegionArchetype::DryShrublandBadlands
        | RegionArchetype::DryShrublandKarst => PrototypePolicyFamily::AridPlain,
        RegionArchetype::DesertDuneField => PrototypePolicyFamily::DuneBody,
        RegionArchetype::GlaciatedAlpine
        | RegionArchetype::AlpineMeadowMountain
        | RegionArchetype::CrevassedIcefield => PrototypePolicyFamily::AlpineHighRelief,
    }
}

fn sample_structure_basis(
    structure: &AtlasStructureMap,
    world_x: f32,
    world_z: f32,
) -> StructureBasisSample {
    let point = (world_x, world_z);
    let mut sample = StructureBasisSample::default();
    let mut heading_accum_x = 0.0_f32;
    let mut heading_accum_z = 0.0_f32;

    for segment in structure.mountain_chains().segments() {
        let world_segment = mountain_segment_world_segment(*segment);
        let half_width_blocks = (segment.half_width_cells.max(0.6) * ATLAS_CELL_BLOCK_SPAN * 0.85).max(24.0);
        let selection_radius = half_width_blocks * 3.2 + 96.0;

        if !segment_bounds_overlap_point(world_segment.start, world_segment.end, selection_radius, point) {
            continue;
        }

        let projection = project_point_onto_segment(point, world_segment.start, world_segment.end);
        let scale_strength = match segment.scale {
            MountainChainScale::Major => 1.0,
            MountainChainScale::Minor => 0.72,
        };
        let strength = segment.strength.max(0.25) * scale_strength;
        let core = smootherstep01(1.0 - projection.distance_blocks / (half_width_blocks * 0.95 + 18.0))
            * strength;
        let shoulder =
            (smootherstep01(1.0 - projection.distance_blocks / (half_width_blocks * 2.8 + 64.0))
                * (0.55 + strength * 0.45)
                - core * 0.35)
                .max(0.0);
        let segment_strength = core.max(shoulder);

        sample.ridge_core_influence =
            soft_union(sample.ridge_core_influence, core.clamp(0.0, 1.0));
        sample.ridge_shoulder_influence =
            soft_union(sample.ridge_shoulder_influence, shoulder.clamp(0.0, 1.0));
        if matches!(segment.scale, MountainChainScale::Major) {
            sample.major_ridge_influence =
                sample.major_ridge_influence.max(segment_strength.clamp(0.0, 1.0));
        }

        if segment_strength > sample.strongest_segment {
            sample.strongest_segment = segment_strength;
        }

        if segment_strength > f32::EPSILON {
            let mut tangent_x = projection.tangent_x;
            let mut tangent_z = projection.tangent_z;
            let accum_len_sq = heading_accum_x * heading_accum_x + heading_accum_z * heading_accum_z;
            if accum_len_sq > f32::EPSILON {
                let dot = tangent_x * heading_accum_x + tangent_z * heading_accum_z;
                if dot < 0.0 {
                    tangent_x = -tangent_x;
                    tangent_z = -tangent_z;
                }
            }
            heading_accum_x += tangent_x * segment_strength;
            heading_accum_z += tangent_z * segment_strength;
        }
    }

    let heading_len_sq = heading_accum_x * heading_accum_x + heading_accum_z * heading_accum_z;
    if heading_len_sq > f32::EPSILON {
        let inv_heading_len = heading_len_sq.sqrt().recip();
        sample.heading_x = heading_accum_x * inv_heading_len;
        sample.heading_z = heading_accum_z * inv_heading_len;
    }

    sample
}

fn coastal_basis(
    field: AtlasCell,
    structure: StructureBasisSample,
    params: BasisParameters,
) -> f32 {
    let marine_signal = smootherstep01(
        (((0.58 - field.landness) * 2.4).max(0.0) + field.coast_factor * 0.35).clamp(0.0, 1.0),
    );
    let coast_signal = smootherstep01(
        (field.coast_factor * 0.86 + inverse_unit(field.coast_distance) * 0.28).clamp(0.0, 1.0),
    );
    let shelf_lowering = params.coastal_shelf_depth * marine_signal
        + params.coastal_apron_lift * coast_signal * (1.0 - field.ruggedness * 0.55);
    let cliff_lift = params.coastal_cliff_lift
        * coast_signal
        * (field.ruggedness * 0.55 + structure.major_ridge_influence * 0.45);

    cliff_lift - shelf_lowering
}

fn ridge_basis(
    field: AtlasCell,
    structure: StructureBasisSample,
    params: BasisParameters,
) -> f32 {
    let core = structure.ridge_core_influence * (0.45 + field.mountain_mass * 0.55);
    let shoulder = structure.ridge_shoulder_influence * (0.55 + field.ridge_factor * 0.45);

    params.ridge_lift * core + params.ridge_shoulder_lift * shoulder
}

fn basin_basis(field: AtlasCell, params: BasisParameters) -> f32 {
    let basin_signal = smootherstep01((field.basinness * 0.72 + field.lake_potential * 0.28).clamp(0.0, 1.0));
    -(params.basin_depth * basin_signal * basin_signal)
}

fn detail_basis(
    field: AtlasCell,
    structure: StructureBasisSample,
    params: BasisParameters,
    world_x: f32,
    world_z: f32,
    wet_signal: f32,
) -> f32 {
    let heading = structure.heading();
    let along = world_x * heading.0 + world_z * heading.1;
    let across = world_x * -heading.1 + world_z * heading.0;
    let (warped_world_x, warped_world_z) =
        seedless_domain_warp(world_x, world_z, 184.0, 14.0, DETAIL_SALT_WARP_X, DETAIL_SALT_WARP_Z);
    let (warped_along, warped_across) = seedless_domain_warp(
        along,
        across,
        132.0,
        10.0,
        DETAIL_SALT_ORIENT_WARP_X,
        DETAIL_SALT_ORIENT_WARP_Z,
    );
    let low_noise =
        seedless_value_fbm(warped_world_x, warped_world_z, 164.0, 3, 2.03, 0.52, DETAIL_SALT_LOW_FREQ);
    let oriented_low = seedless_value_fbm(
        warped_along,
        warped_across,
        108.0,
        3,
        2.08,
        0.50,
        DETAIL_SALT_ORIENTED_LOW,
    );
    let mid_noise =
        seedless_value_fbm(warped_along, warped_across, 46.0, 2, 2.15, 0.56, DETAIL_SALT_MID_FREQ);
    let flat_noise =
        seedless_value_fbm(warped_world_x, warped_world_z, 20.0, 2, 2.02, 0.54, DETAIL_SALT_FLAT_FREQ);
    let terrace_source = (oriented_low * 0.58 + mid_noise * 0.42).clamp(-1.0, 1.0);
    let flat_source = (flat_noise * 0.72 + mid_noise * 0.28).clamp(-1.0, 1.0);
    let terrace_wave = soft_terrace_noise(terrace_source, 6.0, 0.76);
    let flat_wave = soft_terrace_noise(flat_source, 5.0, 0.82);
    let readability_step = soft_terrace_noise(flat_noise, 4.0, 0.90);
    let flat_ripple = seedless_value_fbm(
        warped_world_x,
        warped_world_z,
        11.5,
        2,
        2.00,
        0.52,
        DETAIL_SALT_FLAT_FREQ.wrapping_add(DETAIL_HASH_K1),
    );
    let dune_wave =
        seedless_ridged_fbm(warped_along * 0.72, warped_across * 1.18, 68.0, 3, 2.12, 0.55, DETAIL_SALT_DUNE_FREQ);
    let ridge_noise_gate =
        (0.35 + structure.ridge_shoulder_influence * 0.45 + field.ruggedness * 0.20).clamp(0.0, 1.0);
    let basin_noise_gate = (1.0 - field.basinness * 0.45).clamp(0.40, 1.0);
    let terrace_gate =
        (0.40 + field.slope * 0.45 + structure.ridge_shoulder_influence * 0.25).clamp(0.0, 1.2);
    let flat_readability = (1.0
        - field.ruggedness * 0.72
        - field.slope * 0.46
        - structure.ridge_core_influence * 0.40)
        .clamp(0.0, 1.0);

    let detail = low_noise * params.low_freq_amp
        + oriented_low * params.low_freq_amp * 0.35
        + mid_noise * params.mid_freq_amp * ridge_noise_gate * basin_noise_gate
        + terrace_wave * params.terrace_amp * terrace_gate
        + (flat_wave * (1.10 + params.terrace_amp * 0.20)
            + flat_ripple * 0.84
            + readability_step * 1.46)
            * flat_readability
        + dune_wave * params.dune_amp * (0.55 + field.aridity * 0.45);

    detail * (1.0 - wet_signal * 0.18)
}

fn corridor_adjustment_for_column(
    field: AtlasCell,
    structure: StructureBasisSample,
    local_x: f32,
    local_z: f32,
    region_samples: &[RegionSampleWeight; 4],
    corridors: &[RiverCorridorConstraint],
) -> CorridorAdjustment {
    let params = blended_basis_parameters(region_samples);
    let mode_weights = blended_corridor_mode_weights(region_samples);
    let policy = corridor_policy(params);
    let mut adjustment = CorridorAdjustment::default();
    let mut branch_responses = Vec::<CorridorBranchResponse>::new();

    for corridor in corridors {
        let response =
            single_corridor_adjustment(policy, mode_weights, field, structure, local_x, local_z, *corridor);
        if response.strongest_influence <= f32::EPSILON {
            continue;
        }

        let key = CorridorBranchKey {
            river_id: corridor.river_id,
            kind: corridor.kind,
            order: corridor.order,
        };
        if let Some(existing) = branch_responses.iter_mut().find(|entry| entry.key == key) {
            existing.response = merge_branch_response(existing.response, response);
        } else {
            branch_responses.push(CorridorBranchResponse { key, response });
        }
    }

    for branch in branch_responses {
        adjustment.height_delta += branch.response.height_delta;
        adjustment.relief_budget_penalty += branch.response.relief_budget_penalty;
        adjustment.strongest_influence =
            adjustment.strongest_influence.max(branch.response.strongest_influence);
    }

    adjustment.height_delta = adjustment.height_delta.max(-30.0);
    adjustment.relief_budget_penalty = adjustment.relief_budget_penalty.min(18.0);
    adjustment.strongest_influence = adjustment.strongest_influence.clamp(0.0, 1.0);
    adjustment
}

fn blended_corridor_mode_weights(region_samples: &[RegionSampleWeight; 4]) -> CorridorModeWeights {
    let mut weights = CorridorModeWeights {
        valley: 0.35,
        ..CorridorModeWeights::default()
    };

    for sample in region_samples {
        if sample.weight <= f32::EPSILON {
            continue;
        }

        let region = sample.cell;
        weights.valley += sample.weight * 0.55;

        if region.coastal_context != CoastalContext::Inland
            || matches!(
                region.terrain_form_family,
                TerrainFormFamily::MarineShelf
                    | TerrainFormFamily::BeachPlain
                    | TerrainFormFamily::BarrierCoast
                    | TerrainFormFamily::LagoonCoast
                    | TerrainFormFamily::EstuaryLowland
                    | TerrainFormFamily::Delta
            )
        {
            weights.coastal_exit += sample.weight * 1.30;
            continue;
        }

        if region.hydrology_context == HydrologyContext::LakeBasin
            || matches!(region.terrain_form_family, TerrainFormFamily::Basin)
            || matches!(
                region.archetype,
                RegionArchetype::TemperateBasin | RegionArchetype::DesertBasin
            )
        {
            weights.basin_outlet += sample.weight * 1.20;
        }

        if matches!(
            region.hydrology_context,
            HydrologyContext::RiverCorridor | HydrologyContext::WetLowland
        ) || matches!(
            region.terrain_form_family,
            TerrainFormFamily::Floodplain
                | TerrainFormFamily::WetLowland
                | TerrainFormFamily::AlluvialLowland
                | TerrainFormFamily::BroadValley
                | TerrainFormFamily::GlacialValley
        ) {
            weights.floodplain += sample.weight * 1.10;
        } else {
            weights.valley += sample.weight * 0.25;
        }
    }

    weights.normalized()
}

fn corridor_policy(params: BasisParameters) -> CorridorPolicy {
    CorridorPolicy {
        valley_depth: params.corridor_depth,
        floodplain_depth: params.corridor_depth * 0.72,
        basin_depth: params.corridor_depth * 0.86,
        coastal_depth: params.corridor_depth * 0.64,
        valley_width_scale: params.corridor_width_scale,
        floodplain_width_scale: params.floodplain_width_scale,
        basin_width_scale: params.outlet_open_scale,
        coastal_width_scale: params.outlet_open_scale * 1.20,
        shoulder_preservation: params.ridge_preservation,
        relief_penalty_scale: (0.90 + params.wet_flatten * 0.05).clamp(0.75, 1.80),
    }
}

fn merge_branch_response(current: CorridorAdjustment, candidate: CorridorAdjustment) -> CorridorAdjustment {
    let current_weight = corridor_response_blend_weight(current);
    let candidate_weight = corridor_response_blend_weight(candidate);
    let total_weight = current_weight + candidate_weight;
    if total_weight <= f32::EPSILON {
        return CorridorAdjustment::default();
    }

    CorridorAdjustment {
        height_delta: (current.height_delta * current_weight + candidate.height_delta * candidate_weight)
            / total_weight,
        relief_budget_penalty: (current.relief_budget_penalty * current_weight
            + candidate.relief_budget_penalty * candidate_weight)
            / total_weight,
        strongest_influence: current.strongest_influence.max(candidate.strongest_influence),
        blend_weight: total_weight,
    }
}

fn single_corridor_adjustment(
    policy: CorridorPolicy,
    mode_weights: CorridorModeWeights,
    field: AtlasCell,
    structure: StructureBasisSample,
    local_x: f32,
    local_z: f32,
    corridor: RiverCorridorConstraint,
) -> CorridorAdjustment {
    let projection = project_point_onto_segment(
        (local_x, local_z),
        (corridor.start_x, corridor.start_z),
        (corridor.end_x, corridor.end_z),
    );
    let base_width = corridor.half_width_blocks.max(1.0);
    let valley_influence =
        smootherstep01(1.0 - projection.distance_blocks / (base_width * policy.valley_width_scale));
    let floodplain_influence = smootherstep01(
        1.0 - projection.distance_blocks / (base_width * policy.floodplain_width_scale),
    );
    let basin_influence =
        smootherstep01(1.0 - projection.distance_blocks / (base_width * policy.basin_width_scale));
    let coastal_influence = smootherstep01(
        1.0 - projection.distance_blocks / (base_width * policy.coastal_width_scale),
    );
    let strongest_influence = (valley_influence * mode_weights.valley)
        .max(floodplain_influence * mode_weights.floodplain)
        .max(basin_influence * mode_weights.basin_outlet)
        .max(coastal_influence * mode_weights.coastal_exit)
        .clamp(0.0, 1.0);

    if strongest_influence <= 0.0 {
        return CorridorAdjustment::default();
    }

    let width_depth = corridor_depth_from_width(corridor.half_width_blocks);
    let valley_drop = valley_influence
        * mode_weights.valley
        * (policy.valley_depth + width_depth + corridor.downstream_grade_per_block * 180.0);
    let floodplain_drop = floodplain_influence
        * mode_weights.floodplain
        * (policy.floodplain_depth + corridor.downstream_grade_per_block * 90.0);
    let basin_drop = basin_influence
        * mode_weights.basin_outlet
        * (policy.basin_depth + field.basinness * 4.0);
    let coastal_drop = coastal_influence
        * mode_weights.coastal_exit
        * (policy.coastal_depth + field.coast_factor * 4.0);
    let drop_strength =
        0.92 + field.river_flow_potential * 0.18 + corridor.downstream_grade_per_block * 240.0;
    let drop = (valley_drop + floodplain_drop + basin_drop + coastal_drop) * drop_strength;
    let shoulder_influence =
        (floodplain_influence.max(basin_influence) * (1.0 - valley_influence * 0.82)).clamp(0.0, 1.0);
    let shoulder_gain = shoulder_influence
        * policy.shoulder_preservation
        * (0.35 + structure.ridge_shoulder_influence * 0.65 + field.ruggedness * 0.25);
    let relief_budget_penalty = strongest_influence
        * (3.5 * mode_weights.valley
            + 6.0 * mode_weights.floodplain
            + 6.5 * mode_weights.basin_outlet
            + 5.5 * mode_weights.coastal_exit)
        * policy.relief_penalty_scale;

    CorridorAdjustment {
        height_delta: shoulder_gain - drop,
        relief_budget_penalty,
        strongest_influence,
        blend_weight: corridor_segment_blend_weight(strongest_influence),
    }
}

fn corridor_response_blend_weight(adjustment: CorridorAdjustment) -> f32 {
    if adjustment.blend_weight > f32::EPSILON {
        adjustment.blend_weight
    } else {
        corridor_segment_blend_weight(adjustment.strongest_influence)
    }
}

fn corridor_segment_blend_weight(strongest_influence: f32) -> f32 {
    smootherstep01(strongest_influence.clamp(0.0, 1.0)).max(0.001)
}

fn corridor_depth_from_width(half_width_blocks: f32) -> f32 {
    2.0 + half_width_blocks.max(1.0).sqrt() * 0.50
}

fn macro_elevation_to_world_y(macro_elevation: f32) -> f32 {
    (SEA_LEVEL_Y as f32 - 8.0 + macro_elevation * 140.0).clamp(MIN_BASE_HEIGHT_Y, MAX_BASE_HEIGHT_Y)
}

fn seedless_splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(DETAIL_HASH_K1);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn seedless_hash01(x: i64, z: i64, salt: u64) -> f32 {
    let x_bits = (x as u64).wrapping_mul(DETAIL_HASH_K2);
    let z_bits = (z as u64).wrapping_mul(DETAIL_HASH_K3);
    let bits = seedless_splitmix64(salt ^ x_bits ^ z_bits) >> 11;
    let max = ((1_u64 << 53) - 1) as f64;
    (bits as f64 / max) as f32
}

fn seedless_value_noise01(x: f32, z: f32, lattice_scale: f32, salt: u64) -> f32 {
    let sample_x = x / lattice_scale.max(1.0);
    let sample_z = z / lattice_scale.max(1.0);
    let x0 = sample_x.floor() as i64;
    let z0 = sample_z.floor() as i64;
    let tx = smootherstep01(sample_x - x0 as f32);
    let tz = smootherstep01(sample_z - z0 as f32);
    let n00 = seedless_hash01(x0, z0, salt);
    let n10 = seedless_hash01(x0 + 1, z0, salt);
    let n01 = seedless_hash01(x0, z0 + 1, salt);
    let n11 = seedless_hash01(x0 + 1, z0 + 1, salt);
    let nx0 = n00 + (n10 - n00) * tx;
    let nx1 = n01 + (n11 - n01) * tx;
    nx0 + (nx1 - nx0) * tz
}

fn seedless_value_noise_signed(x: f32, z: f32, lattice_scale: f32, salt: u64) -> f32 {
    seedless_value_noise01(x, z, lattice_scale, salt) * 2.0 - 1.0
}

fn seedless_value_fbm(
    x: f32,
    z: f32,
    base_scale: f32,
    octaves: u32,
    lacunarity: f32,
    gain: f32,
    salt: u64,
) -> f32 {
    let mut amplitude = 1.0_f32;
    let mut scale = base_scale.max(1.0);
    let mut total = 0.0_f32;
    let mut amplitude_sum = 0.0_f32;

    for octave in 0..octaves {
        let octave_salt = salt.wrapping_add((octave as u64).wrapping_mul(DETAIL_HASH_K1));
        total += seedless_value_noise_signed(x, z, scale, octave_salt) * amplitude;
        amplitude_sum += amplitude;
        amplitude *= gain;
        scale /= lacunarity.max(1.01);
    }

    if amplitude_sum <= f32::EPSILON {
        0.0
    } else {
        (total / amplitude_sum).clamp(-1.0, 1.0)
    }
}

fn seedless_ridged_fbm(
    x: f32,
    z: f32,
    base_scale: f32,
    octaves: u32,
    lacunarity: f32,
    gain: f32,
    salt: u64,
) -> f32 {
    let mut amplitude = 1.0_f32;
    let mut scale = base_scale.max(1.0);
    let mut total = 0.0_f32;
    let mut amplitude_sum = 0.0_f32;

    for octave in 0..octaves {
        let octave_salt = salt.wrapping_add((octave as u64).wrapping_mul(DETAIL_HASH_K2));
        let signal = seedless_value_noise_signed(x, z, scale, octave_salt);
        let ridged = 1.0 - signal.abs();
        total += (ridged * 2.0 - 1.0) * amplitude;
        amplitude_sum += amplitude;
        amplitude *= gain;
        scale /= lacunarity.max(1.01);
    }

    if amplitude_sum <= f32::EPSILON {
        0.0
    } else {
        (total / amplitude_sum).clamp(-1.0, 1.0)
    }
}

fn seedless_domain_warp(x: f32, z: f32, warp_scale: f32, amplitude: f32, salt_x: u64, salt_z: u64) -> (f32, f32) {
    let dx = seedless_value_fbm(x, z, warp_scale, 3, 2.0, 0.5, salt_x) * amplitude;
    let dz = seedless_value_fbm(x, z, warp_scale, 3, 2.0, 0.5, salt_z) * amplitude;
    (x + dx, z + dz)
}

fn soft_terrace_noise(signal: f32, steps: f32, sharpness: f32) -> f32 {
    let normalized = signal.clamp(-1.0, 1.0) * 0.5 + 0.5;
    let steps = steps.max(1.0);
    let terraced = (normalized * steps).floor() / steps;
    let blend = sharpness.clamp(0.0, 1.0);
    let mixed = terraced * blend + normalized * (1.0 - blend);
    mixed * 2.0 - 1.0
}

fn inverse_unit(value: f32) -> f32 {
    1.0 - value.clamp(0.0, 1.0)
}

fn smootherstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn soft_union(current: f32, candidate: f32) -> f32 {
    1.0 - (1.0 - current.clamp(0.0, 1.0)) * (1.0 - candidate.clamp(0.0, 1.0))
}

#[derive(Debug, Clone, Copy)]
struct WorldSegmentLine {
    start: (f32, f32),
    end: (f32, f32),
}

fn mountain_segment_world_segment(segment: MountainSpineSegment) -> WorldSegmentLine {
    WorldSegmentLine {
        start: atlas_coord_to_world_center(segment.start.x, segment.start.z),
        end: atlas_coord_to_world_center(segment.end.x, segment.end.z),
    }
}

fn atlas_coord_to_world_center(x: i32, z: i32) -> (f32, f32) {
    (
        x as f32 * ATLAS_CELL_BLOCK_SPAN + ATLAS_CELL_BLOCK_SPAN * 0.5,
        z as f32 * ATLAS_CELL_BLOCK_SPAN + ATLAS_CELL_BLOCK_SPAN * 0.5,
    )
}

fn segment_bounds_overlap_point(
    start: (f32, f32),
    end: (f32, f32),
    padding_blocks: f32,
    point: (f32, f32),
) -> bool {
    let min_x = start.0.min(end.0) - padding_blocks;
    let max_x = start.0.max(end.0) + padding_blocks;
    let min_z = start.1.min(end.1) - padding_blocks;
    let max_z = start.1.max(end.1) + padding_blocks;

    point.0 >= min_x && point.0 <= max_x && point.1 >= min_z && point.1 <= max_z
}

fn project_point_onto_segment(
    point: (f32, f32),
    start: (f32, f32),
    end: (f32, f32),
) -> ProjectedSegmentPoint {
    let seg_x = end.0 - start.0;
    let seg_z = end.1 - start.1;
    let length_sq = seg_x * seg_x + seg_z * seg_z;

    if length_sq <= f32::EPSILON {
        return ProjectedSegmentPoint {
            distance_blocks: distance_between_points(point, start),
            tangent_x: 1.0,
            tangent_z: 0.0,
        };
    }

    let inv_length = length_sq.sqrt().recip();
    let tangent_x = seg_x * inv_length;
    let tangent_z = seg_z * inv_length;
    let t = (((point.0 - start.0) * seg_x + (point.1 - start.1) * seg_z) / length_sq).clamp(0.0, 1.0);
    let projected = (start.0 + seg_x * t, start.1 + seg_z * t);

    ProjectedSegmentPoint {
        distance_blocks: distance_between_points(point, projected),
        tangent_x,
        tangent_z,
    }
}

fn distance_between_points(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dz = a.1 - b.1;
    (dx * dx + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::v2::{
        build_chunk_corridor_window, empty_chunk_corridor_window, prepare_chunk_v2_inputs,
    };
    use crate::world::meta::WorldMeta;

    #[test]
    fn base_heightfield_is_deterministic_and_emits_a_full_grid() {
        let meta = WorldMeta::new(42);
        let chunk = ChunkCoord(4, 0, -3);
        let inputs = prepare_chunk_v2_inputs(chunk, &meta);
        let corridor_window = build_chunk_corridor_window(chunk, &inputs);
        let a = build_chunk_base_heightfield_prototype(chunk, &inputs, &corridor_window);
        let b = build_chunk_base_heightfield_prototype(chunk, &inputs, &corridor_window);

        assert_eq!(a, b);
        assert_eq!(a.chunk, chunk);
        assert_eq!(a.columns.len(), (CHUNK_EDGE_I32 as usize) * (CHUNK_EDGE_I32 as usize));
        assert!(a
            .columns
            .iter()
            .all(|column| column.base_height.is_finite() && column.relief_budget.is_finite()));
        assert!(a
            .columns
            .iter()
            .all(|column| (column.base_height - column.base_height.round()).abs() <= 0.0001));
        assert!(a
            .columns
            .iter()
            .all(|column| column.relief_budget >= MIN_RELIEF_BUDGET));
    }

    #[test]
    fn corridor_influence_lowers_the_prototype_near_a_corridor() {
        let meta = WorldMeta::new(42);
        let (chunk, inputs, corridor_window) = chunk_with_corridor_window(&meta);
        let with_corridor =
            build_chunk_base_heightfield_prototype(chunk, &inputs, &corridor_window);
        let without_corridor =
            build_chunk_base_heightfield_prototype(chunk, &inputs, &empty_chunk_corridor_window(chunk));
        let focus = corridor_focus_index(corridor_window.corridors[0]);

        assert!(
            with_corridor.columns[focus].base_height < without_corridor.columns[focus].base_height
        );
        assert!(
            with_corridor.columns[focus].relief_budget
                <= without_corridor.columns[focus].relief_budget
        );
    }

    #[test]
    fn temperate_plain_archetype_hint_flattens_plain_family_defaults() {
        let mut hinted_region = RegionClassCell::default();
        hinted_region.archetype = RegionArchetype::TemperatePlain;
        hinted_region.terrain_form_family = TerrainFormFamily::Plain;
        hinted_region.hydrology_context = HydrologyContext::WellDrained;

        let mut neutral_region = hinted_region;
        neutral_region.archetype = RegionArchetype::SavannaPlain;

        let neutral = basis_parameters_for_region(neutral_region);
        let hinted = basis_parameters_for_region(hinted_region);

        assert!(hinted.wet_flatten > neutral.wet_flatten);
        assert!(hinted.low_freq_amp < neutral.low_freq_amp);
        assert!(hinted.mid_freq_amp < neutral.mid_freq_amp);
        assert!(hinted.relief_base < neutral.relief_base);
    }

    #[test]
    fn glaciated_alpine_archetype_hint_emphasizes_mountain_family_defaults() {
        let mut hinted_region = RegionClassCell::default();
        hinted_region.archetype = RegionArchetype::GlaciatedAlpine;
        hinted_region.terrain_form_family = TerrainFormFamily::Icefield;
        hinted_region.hydrology_context = HydrologyContext::WellDrained;

        let mut neutral_region = hinted_region;
        neutral_region.archetype = RegionArchetype::AlpineMeadowMountain;

        let neutral = basis_parameters_for_region(neutral_region);
        let hinted = basis_parameters_for_region(hinted_region);

        assert!(hinted.macro_height_bonus > neutral.macro_height_bonus);
        assert!(hinted.ridge_lift > neutral.ridge_lift);
        assert!(hinted.mid_freq_amp > neutral.mid_freq_amp);
        assert!(hinted.relief_gain > neutral.relief_gain);
    }

    #[test]
    fn prototype_output_varies_across_the_chunk_surface() {
        let meta = WorldMeta::new(42);
        let chunk = ChunkCoord(4, 0, -3);
        let inputs = prepare_chunk_v2_inputs(chunk, &meta);
        let corridor_window = build_chunk_corridor_window(chunk, &inputs);
        let prototype = build_chunk_base_heightfield_prototype(chunk, &inputs, &corridor_window);
        let min_height = prototype
            .columns
            .iter()
            .map(|column| column.base_height)
            .fold(f32::INFINITY, f32::min);
        let max_height = prototype
            .columns
            .iter()
            .map(|column| column.base_height)
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(max_height > min_height);
    }

    #[test]
    fn shared_edge_transition_stays_close_to_neighboring_local_slopes() {
        let meta = WorldMeta::new(42);
        let left = build_prototype(ChunkCoord(4, 0, -3), &meta);
        let right = build_prototype(ChunkCoord(5, 0, -3), &meta);

        assert_shared_edge_is_gradual(&left, &right);
    }

    #[test]
    fn atlas_cell_edge_transition_stays_close_to_neighboring_local_slopes() {
        let meta = WorldMeta::new(42);
        let left = build_prototype(ChunkCoord(15, 0, 15), &meta);
        let right = build_prototype(ChunkCoord(16, 0, 15), &meta);

        assert_shared_edge_is_gradual(&left, &right);
    }

    #[test]
    fn canonical_region_edge_transition_stays_close_to_neighboring_local_slopes() {
        let meta = WorldMeta::new(42);
        let left = build_prototype(ChunkCoord(127, 0, 0), &meta);
        let right = build_prototype(ChunkCoord(128, 0, 0), &meta);

        assert_shared_edge_is_gradual(&left, &right);
    }

    #[test]
    #[ignore = "diagnostic output for the reported wall strip"]
    fn dump_reported_wall_strip() {
        let meta = WorldMeta::new(42);
        let scan = scan_reported_wall_area(&meta);

        println!(
            "reported wall scan: strongest {} delta {:.3} at world ({}, {}) between {:.3} and {:.3}",
            scan.axis,
            scan.delta,
            scan.world_x,
            scan.world_z,
            scan.left_or_back_height,
            scan.right_or_front_height
        );

        let min_world_x = scan.world_x.saturating_sub(4);
        let max_world_x = scan.world_x.saturating_add(4);
        let min_world_z = scan.world_z.saturating_sub(2);
        let max_world_z = scan.world_z.saturating_add(2);
        let mut cache = std::collections::HashMap::new();
        for world_z in min_world_z..=max_world_z {
            print!("z={world_z:>5}:");
            for world_x in min_world_x..=max_world_x {
                print!(
                    " {:>7.2}",
                    sampled_world_height(world_x, world_z, &meta, &mut cache)
                );
            }
            println!();
        }

        dump_world_sample_details(scan.world_x, scan.world_z - 1, &meta);
        dump_world_sample_details(scan.world_x, scan.world_z, &meta);
    }


    #[test]
    fn reported_wall_strip_stays_below_large_vertical_step_threshold() {
        let meta = WorldMeta::new(42);
        let scan = scan_reported_wall_area(&meta);

        assert!(
            scan.delta <= 5.0,
            "reported wall strip still has a large {} delta of {:.3} at world ({}, {})",
            scan.axis,
            scan.delta,
            scan.world_x,
            scan.world_z
        );
    }

    #[test]
    fn merge_branch_response_softly_blends_similar_segment_responses() {
        let current = CorridorAdjustment {
            height_delta: -18.0,
            relief_budget_penalty: 8.5,
            strongest_influence: 0.95,
            blend_weight: corridor_segment_blend_weight(0.95),
        };
        let candidate = CorridorAdjustment {
            height_delta: -14.0,
            relief_budget_penalty: 6.0,
            strongest_influence: 0.92,
            blend_weight: corridor_segment_blend_weight(0.92),
        };

        let merged = merge_branch_response(current, candidate);
        let reverse_merged = merge_branch_response(candidate, current);

        assert!(
            merged.height_delta > current.height_delta && merged.height_delta < candidate.height_delta,
            "expected soft blended height delta between {:.3} and {:.3}, got {:.3}",
            current.height_delta,
            candidate.height_delta,
            merged.height_delta
        );
        assert!(
            merged.relief_budget_penalty < current.relief_budget_penalty
                && merged.relief_budget_penalty > candidate.relief_budget_penalty,
            "expected soft blended relief penalty between {:.3} and {:.3}, got {:.3}",
            candidate.relief_budget_penalty,
            current.relief_budget_penalty,
            merged.relief_budget_penalty
        );
        assert!(
            (merged.height_delta - reverse_merged.height_delta).abs() <= 0.0001
                && (merged.relief_budget_penalty - reverse_merged.relief_budget_penalty).abs()
                    <= 0.0001,
            "expected branch-response blend to be order-independent"
        );
    }

    #[test]
    fn seedless_value_noise_helpers_are_deterministic_and_bounded() {
        let a = seedless_value_fbm(123.5, -88.25, 96.0, 3, 2.0, 0.5, DETAIL_SALT_LOW_FREQ);
        let b = seedless_value_fbm(123.5, -88.25, 96.0, 3, 2.0, 0.5, DETAIL_SALT_LOW_FREQ);
        let c = seedless_value_fbm(124.5, -88.25, 96.0, 3, 2.0, 0.5, DETAIL_SALT_LOW_FREQ);
        let terraced = soft_terrace_noise(a, 5.0, 0.85);

        assert_eq!(a.to_bits(), b.to_bits());
        assert!(a.is_finite() && c.is_finite() && terraced.is_finite());
        assert!(a >= -1.0 && a <= 1.0);
        assert!(terraced >= -1.0 && terraced <= 1.0);
        assert_ne!(a.to_bits(), c.to_bits());
    }

    #[test]
    fn horizontal_boundary_band_near_neg56_pos93_stays_below_large_step_threshold() {
        let meta = WorldMeta::new(42);
        let min_chunk_x = -66;
        let max_chunk_x = -46;
        let min_boundary_chunk_z = 84;
        let max_boundary_chunk_z = 103;
        let min_world_x = min_chunk_x * CHUNK_EDGE_I32;
        let max_world_x = (max_chunk_x + 1) * CHUNK_EDGE_I32 - 1;
        let mut cache = std::collections::HashMap::new();
        let mut strongest_delta = 0.0_f32;
        let mut strongest_world_x = min_world_x;
        let mut strongest_world_z = min_boundary_chunk_z * CHUNK_EDGE_I32;

        for boundary_chunk_z in min_boundary_chunk_z..=max_boundary_chunk_z {
            let world_z = boundary_chunk_z * CHUNK_EDGE_I32;
            for world_x in min_world_x..=max_world_x {
                let back = sampled_world_height(world_x, world_z - 1, &meta, &mut cache);
                let front = sampled_world_height(world_x, world_z, &meta, &mut cache);
                let delta = (front - back).abs();
                if delta > strongest_delta {
                    strongest_delta = delta;
                    strongest_world_x = world_x;
                    strongest_world_z = world_z;
                }
            }
        }

        assert!(
            strongest_delta <= 1.0,
            "horizontal boundary band near cx=-56 cz=93 still has a large step of {:.3} at world ({}, {})",
            strongest_delta,
            strongest_world_x,
            strongest_world_z
        );
    }

    #[test]
    fn low_relief_chunks_still_show_subchunk_layer_variation() {
        let meta = WorldMeta::new(42);
        let prototype = low_relief_prototype(&meta);
        let mut max_adjacent_delta = 0.0_f32;

        for row in 0..CHUNK_EDGE_I32 as usize {
            for col in 1..CHUNK_EDGE_I32 as usize {
                let current = prototype.columns[row * CHUNK_EDGE_I32 as usize + col].base_height;
                let prev = prototype.columns[row * CHUNK_EDGE_I32 as usize + col - 1].base_height;
                max_adjacent_delta = max_adjacent_delta.max((current - prev).abs());
            }
        }

        assert!(
            max_adjacent_delta >= 0.25,
            "expected visible subchunk layer variation, found max adjacent delta {max_adjacent_delta}"
        );
    }

    fn build_prototype(chunk: ChunkCoord, meta: &WorldMeta) -> BaseHeightfieldPrototype {
        let inputs = prepare_chunk_v2_inputs(chunk, meta);
        let corridor_window = build_chunk_corridor_window(chunk, &inputs);
        build_chunk_base_heightfield_prototype(chunk, &inputs, &corridor_window)
    }

    fn chunk_with_corridor_window(
        meta: &WorldMeta,
    ) -> (ChunkCoord, ChunkGenerationV2Inputs, ChunkCorridorWindow) {
        let candidates = [
            ChunkCoord(4, 0, -3),
            ChunkCoord(0, 0, 0),
            ChunkCoord(15, 0, 15),
            ChunkCoord(16, 0, 16),
            ChunkCoord(-1, 0, -1),
        ];

        for chunk in candidates {
            let inputs = prepare_chunk_v2_inputs(chunk, meta);
            let corridor_window = build_chunk_corridor_window(chunk, &inputs);
            if !corridor_window.corridors.is_empty() {
                return (chunk, inputs, corridor_window);
            }
        }

        panic!("expected at least one sampled chunk to carry corridor influence");
    }

    fn low_relief_prototype(meta: &WorldMeta) -> BaseHeightfieldPrototype {
        let candidates = [
            ChunkCoord(30, 0, -20),
            ChunkCoord(0, 0, 0),
            ChunkCoord(8, 0, 8),
            ChunkCoord(24, 0, -8),
            ChunkCoord(-8, 0, 12),
        ];

        for chunk in candidates {
            let inputs = prepare_chunk_v2_inputs(chunk, meta);
            let corridor_window = empty_chunk_corridor_window(chunk);
            let prototype = build_chunk_base_heightfield_prototype(chunk, &inputs, &corridor_window);
            let max_adjacent_delta = prototype
                .columns
                .chunks(CHUNK_EDGE_I32 as usize)
                .flat_map(|row| row.windows(2))
                .map(|pair| (pair[1].base_height - pair[0].base_height).abs())
                .fold(0.0_f32, f32::max);
            if max_adjacent_delta > 0.0 {
                return prototype;
            }
        }

        panic!("expected at least one low-relief prototype candidate");
    }

    fn sampled_world_height(
        world_x: i32,
        world_z: i32,
        meta: &WorldMeta,
        cache: &mut std::collections::HashMap<ChunkCoord, BaseHeightfieldPrototype>,
    ) -> f32 {
        let chunk = ChunkCoord(
            world_x.div_euclid(CHUNK_EDGE_I32),
            0,
            world_z.div_euclid(CHUNK_EDGE_I32),
        );
        let local_x = world_x.rem_euclid(CHUNK_EDGE_I32) as usize;
        let local_z = world_z.rem_euclid(CHUNK_EDGE_I32) as usize;
        let prototype = cache.entry(chunk).or_insert_with(|| build_prototype(chunk, meta));
        prototype.columns[local_z * CHUNK_EDGE_I32 as usize + local_x].base_height
    }

    fn scan_reported_wall_area(meta: &WorldMeta) -> ReportedWallScan {
        let min_chunk_x = 20;
        let max_chunk_x = 30;
        let min_chunk_z = -22;
        let max_chunk_z = -21;
        let min_world_x = min_chunk_x * CHUNK_EDGE_I32;
        let max_world_x = (max_chunk_x + 1) * CHUNK_EDGE_I32 - 1;
        let min_world_z = min_chunk_z * CHUNK_EDGE_I32;
        let max_world_z = (max_chunk_z + 1) * CHUNK_EDGE_I32 - 1;
        let width = usize::try_from(max_world_x - min_world_x + 1).expect("valid wall scan width");
        let depth = usize::try_from(max_world_z - min_world_z + 1).expect("valid wall scan depth");
        let mut heights = vec![0.0_f32; width * depth];
        let mut cache = std::collections::HashMap::new();

        for world_z in min_world_z..=max_world_z {
            for world_x in min_world_x..=max_world_x {
                let dz = usize::try_from(world_z - min_world_z).expect("valid wall scan row");
                let dx = usize::try_from(world_x - min_world_x).expect("valid wall scan column");
                heights[dz * width + dx] =
                    sampled_world_height(world_x, world_z, meta, &mut cache);
            }
        }

        let mut strongest = ReportedWallScan {
            axis: "horizontal",
            delta: 0.0,
            world_x: min_world_x,
            world_z: min_world_z,
            left_or_back_height: heights[0],
            right_or_front_height: heights[0],
        };

        for dz in 0..depth {
            let world_z = min_world_z + dz as i32;
            for dx in 1..width {
                let world_x = min_world_x + dx as i32;
                let left = heights[dz * width + dx - 1];
                let right = heights[dz * width + dx];
                let delta = (right - left).abs();
                if delta > strongest.delta {
                    strongest = ReportedWallScan {
                        axis: "horizontal",
                        delta,
                        world_x,
                        world_z,
                        left_or_back_height: left,
                        right_or_front_height: right,
                    };
                }
            }
        }

        for dz in 1..depth {
            let world_z = min_world_z + dz as i32;
            for dx in 0..width {
                let world_x = min_world_x + dx as i32;
                let back = heights[(dz - 1) * width + dx];
                let front = heights[dz * width + dx];
                let delta = (front - back).abs();
                if delta > strongest.delta {
                    strongest = ReportedWallScan {
                        axis: "vertical",
                        delta,
                        world_x,
                        world_z,
                        left_or_back_height: back,
                        right_or_front_height: front,
                    };
                }
            }
        }

        strongest
    }

    fn corridor_focus_index(corridor: RiverCorridorConstraint) -> usize {
        let focus_x = corridor
            .center_x
            .floor()
            .clamp(0.0, CHUNK_EDGE_I32 as f32 - 1.0) as usize;
        let focus_z = corridor
            .center_z
            .floor()
            .clamp(0.0, CHUNK_EDGE_I32 as f32 - 1.0) as usize;

        focus_z * CHUNK_EDGE_I32 as usize + focus_x
    }

    fn assert_shared_edge_is_gradual(
        left: &BaseHeightfieldPrototype,
        right: &BaseHeightfieldPrototype,
    ) {
        for row in 0..CHUNK_EDGE_I32 as usize {
            let left_last = left.columns[row * CHUNK_EDGE_I32 as usize + (CHUNK_EDGE_I32 as usize - 1)]
                .base_height;
            let left_prev = left.columns[row * CHUNK_EDGE_I32 as usize + (CHUNK_EDGE_I32 as usize - 2)]
                .base_height;
            let right_first = right.columns[row * CHUNK_EDGE_I32 as usize].base_height;
            let right_next = right.columns[row * CHUNK_EDGE_I32 as usize + 1].base_height;
            let seam_delta = (right_first - left_last).abs();
            let local_delta = (left_last - left_prev)
                .abs()
                .max((right_next - right_first).abs());

            assert!(
                seam_delta <= local_delta + 10.0,
                "shared edge delta {seam_delta} should stay close to neighboring local slope {local_delta} at row {row}"
            );
        }
    }

    fn dump_world_sample_details(world_x: i32, world_z: i32, meta: &WorldMeta) {
        let chunk = ChunkCoord(
            world_x.div_euclid(CHUNK_EDGE_I32),
            0,
            world_z.div_euclid(CHUNK_EDGE_I32),
        );
        let local_x = world_x.rem_euclid(CHUNK_EDGE_I32) as usize;
        let local_z = world_z.rem_euclid(CHUNK_EDGE_I32) as usize;
        let sample_world_x = world_x as f32 + 0.5;
        let sample_world_z = world_z as f32 + 0.5;
        let inputs = prepare_chunk_v2_inputs(chunk, meta);
        let corridor_window = build_chunk_corridor_window(chunk, &inputs);
        let field = sample_atlas_fields_fractional(&inputs.atlas_fields, sample_world_x, sample_world_z);
        let region_samples = sample_region_weights(&inputs.region_classes, sample_world_x, sample_world_z);
        let structure_sample =
            sample_structure_basis(&inputs.atlas_structure, sample_world_x, sample_world_z);
        let base_before_corridor =
            blended_base_height(&region_samples, field, structure_sample, sample_world_x, sample_world_z);
        let corridor_adjustment = corridor_adjustment_for_column(
            field,
            structure_sample,
            local_x as f32 + 0.5,
            local_z as f32 + 0.5,
            &region_samples,
            &corridor_window.corridors,
        );

        println!(
            "sample world=({}, {}) chunk=({}, {}) local=({}, {}) base_before_corridor={:.3} corridor_delta={:.3} final={:.3} corridors={}",
            world_x,
            world_z,
            chunk.0,
            chunk.2,
            local_x,
            local_z,
            base_before_corridor,
            corridor_adjustment.height_delta,
            base_before_corridor + corridor_adjustment.height_delta,
            corridor_window.corridors.len()
        );
        println!(
            "  field macro={:.3} slope={:.3} rugged={:.3} mountain={:.3} basin={:.3} river={:.3} coast={:.3} wetness={:.3}",
            field.macro_elevation,
            field.slope,
            field.ruggedness,
            field.mountain_mass,
            field.basinness,
            field.river_flow_potential,
            field.coast_factor,
            field.wetness
        );
        println!(
            "  structure ridge_core={:.3} ridge_shoulder={:.3} major_ridge={:.3} heading=({:.3}, {:.3})",
            structure_sample.ridge_core_influence,
            structure_sample.ridge_shoulder_influence,
            structure_sample.major_ridge_influence,
            structure_sample.heading().0,
            structure_sample.heading().1
        );

        for sample in region_samples {
            if sample.weight <= f32::EPSILON {
                continue;
            }
            println!(
                "  region weight={:.3} archetype={:?} terrain={:?} hydro={:?} coastal={:?} family={:?}",
                sample.weight,
                sample.cell.archetype,
                sample.cell.terrain_form_family,
                sample.cell.hydrology_context,
                sample.cell.coastal_context,
                prototype_policy_family(sample.cell)
            );
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct ReportedWallScan {
        axis: &'static str,
        delta: f32,
        world_x: i32,
        world_z: i32,
        left_or_back_height: f32,
        right_or_front_height: f32,
    }
}
