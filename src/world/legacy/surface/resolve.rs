#[cfg(test)]
use crate::world::atlas::{AtlasCoord, RegionClassInfluence};
use crate::world::atlas::{
    ClimateRegime, CoastalContext, HydrologyContext, RegionArchetype, RegionClassCell,
    RegionClassInfluenceSet, sample_region_class_influences, sample_region_classes,
};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};
use crate::world::generation::{
    ChunkGenerationInputs, HydrologyMode, HydrologySolve, SmoothedPrototype,
};
use crate::world::{SEA_LEVEL_Y, SmoothedColumn};

use super::cover::CoverPhase;
use super::domain::{MaterialDomainInput, MaterialDomainKind, sample_material_domain};
use super::material::{MaterialPolicyDef, MaterialPolicyId, material_policy_def};
use super::seasonal::{SeasonalBiomeStateId, SeasonalPhase};

const MATERIAL_TRANSITION_HASH_K1: u64 = 0x9E37_79B9_7F4A_7C15;
const MATERIAL_TRANSITION_HASH_K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const MATERIAL_TRANSITION_SALT: u64 = 0x5A37_FACA_DE00_0001;
const MATERIAL_TRANSITION_FINE_SALT: u64 = 0x5A37_FACA_DE00_0002;
const MATERIAL_BOUNDARY_STEPPING_SALT: u64 = 0x5A37_FACA_DE00_0003;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SurfaceRuntimeContext {
    pub seasonal_phase: Option<SeasonalPhase>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceColumnPlan {
    pub owner_archetype: RegionArchetype,
    pub material_policy: MaterialPolicyId,
    pub seasonal_state: Option<SeasonalBiomeStateId>,
    pub cover_phase: CoverPhase,
    pub cover_override_key: Option<&'static str>,
    pub terrain_top_y: i32,
    pub water_top_y: Option<i32>,
    pub top_block_key: &'static str,
    pub filler_block_key: &'static str,
    pub core_block_key: &'static str,
    pub water_block_key: Option<&'static str>,
    pub filler_depth: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSurfacePlan {
    pub chunk: ChunkCoord,
    pub columns: Vec<SurfaceColumnPlan>,
}

#[derive(Debug, Clone, Copy)]
struct MaterialTransitionSelection {
    policy: MaterialPolicyId,
    strength: f32,
    boundary_value: f32,
}

#[derive(Debug, Clone, Copy)]
struct VisualBoundaryStepClaim {
    source: SurfaceColumnPlan,
    source_dx: i32,
    source_dz: i32,
    score: f32,
}

pub fn empty_chunk_surface_plan(chunk: ChunkCoord) -> ChunkSurfacePlan {
    ChunkSurfacePlan {
        chunk,
        columns: Vec::new(),
    }
}

pub fn resolve_material_policy_for_archetype(archetype: RegionArchetype) -> MaterialPolicyId {
    match archetype {
        RegionArchetype::OceanicShelf => MaterialPolicyId::OceanicShelf,
        RegionArchetype::SandyBeachPlain
        | RegionArchetype::BarrierCoast
        | RegionArchetype::LagoonCoast => MaterialPolicyId::SandyBeach,
        RegionArchetype::CoastalCliffland
        | RegionArchetype::RockyShoreCoast
        | RegionArchetype::FjordCoast => MaterialPolicyId::CoastalCliff,
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
        | RegionArchetype::BorealWetLowland => MaterialPolicyId::ColdWetland,
        RegionArchetype::TemperatePlain
        | RegionArchetype::TemperateRollingPlain
        | RegionArchetype::TemperateBasin
        | RegionArchetype::TemperateBroadValley
        | RegionArchetype::TemperateBroadleafPlain
        | RegionArchetype::TemperateMixedHills
        | RegionArchetype::BorealPlain
        | RegionArchetype::BorealHills => MaterialPolicyId::TemperateGrassland,
        RegionArchetype::TemperateHills | RegionArchetype::TemperateEscarpmentUpland => {
            MaterialPolicyId::TemperateGrassland
        }
        RegionArchetype::TemperatePlateau | RegionArchetype::MonsoonPlateau => {
            MaterialPolicyId::TemperatePlateau
        }
        RegionArchetype::SteppePlain | RegionArchetype::SteppeHills => {
            MaterialPolicyId::SteppeGrassland
        }
        RegionArchetype::DesertPlain
        | RegionArchetype::DesertDuneField
        | RegionArchetype::DesertBasin
        | RegionArchetype::DesertMesaCountry
        | RegionArchetype::DesertAlluvialFan
        | RegionArchetype::SemiDesertPediment
        | RegionArchetype::DryShrublandBadlands
        | RegionArchetype::DryShrublandKarst
        | RegionArchetype::MediterraneanShrublandHills => MaterialPolicyId::DesertSurface,
        RegionArchetype::SavannaPlain
        | RegionArchetype::SavannaHills
        | RegionArchetype::TropicalDryForestHills => MaterialPolicyId::SavannaGrassland,
        RegionArchetype::TropicalRainforestLowland => MaterialPolicyId::TropicalLowland,
        RegionArchetype::TropicalRainforestHills => MaterialPolicyId::TropicalHills,
        RegionArchetype::GlaciatedAlpine
        | RegionArchetype::SubalpineWoodedFront
        | RegionArchetype::AlpineMeadowMountain
        | RegionArchetype::GlacialValley
        | RegionArchetype::CrevassedIcefield
        | RegionArchetype::BorealRidgeCountry
        | RegionArchetype::AlpineRavineCountry => MaterialPolicyId::AlpineExposed,
        RegionArchetype::TundraPlain | RegionArchetype::PolarBarrensPlain => {
            MaterialPolicyId::TundraExposure
        }
    }
}

pub fn resolve_chunk_surface_plan(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
    smoothed: &SmoothedPrototype,
    hydrology: &HydrologySolve,
) -> ChunkSurfacePlan {
    resolve_chunk_surface_plan_with_runtime(chunk, inputs, smoothed, hydrology, None)
}

pub fn resolve_chunk_surface_plan_with_runtime(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
    smoothed: &SmoothedPrototype,
    hydrology: &HydrologySolve,
    runtime: Option<&SurfaceRuntimeContext>,
) -> ChunkSurfacePlan {
    debug_assert_eq!(inputs.chunk, chunk);
    debug_assert_eq!(smoothed.chunk, chunk);
    debug_assert_eq!(hydrology.chunk, chunk);
    debug_assert_eq!(smoothed.columns.len(), hydrology.columns.len());

    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let halo_edge = CHUNK_EDGE_I32 + 2;
    let halo_origin_x = chunk_origin_x - 1;
    let halo_origin_z = chunk_origin_z - 1;
    let mut halo_columns = Vec::with_capacity((halo_edge * halo_edge) as usize);

    for halo_z in 0..halo_edge {
        for halo_x in 0..halo_edge {
            let local_x = halo_x - 1;
            let local_z = halo_z - 1;
            let sample_x = local_x.clamp(0, CHUNK_EDGE_I32 - 1);
            let sample_z = local_z.clamp(0, CHUNK_EDGE_I32 - 1);
            let index = column_index(sample_x, sample_z);
            let world_x = chunk_origin_x + local_x;
            let world_z = chunk_origin_z + local_z;
            let region = sample_region_classes(&inputs.region_classes, world_x, world_z);
            let smoothed_column = smoothed.columns[index];
            let hydrology_column = hydrology.columns[index];
            let (material_sample_x, material_sample_z) = material_influence_sample_position(
                world_x,
                world_z,
                smoothed_column,
                hydrology_column,
            );
            let influence = sample_region_class_influences(
                &inputs.region_classes,
                material_sample_x,
                material_sample_z,
            );
            halo_columns.push(resolve_surface_column_plan_with_influence(
                region,
                &influence,
                world_x,
                world_z,
                smoothed_column,
                hydrology_column,
                runtime,
            ));
        }
    }

    apply_visual_boundary_stepping(
        &mut halo_columns,
        halo_edge,
        halo_origin_x,
        halo_origin_z,
        inputs.seed,
    );

    let mut columns = Vec::with_capacity(hydrology.columns.len());
    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            columns.push(halo_columns[grid_column_index(local_x + 1, local_z + 1, halo_edge)]);
        }
    }

    ChunkSurfacePlan { chunk, columns }
}

#[cfg(test)]
fn single_region_influence(owner: RegionClassCell) -> RegionClassInfluenceSet {
    RegionClassInfluenceSet {
        dominant: RegionClassInfluence {
            coord: AtlasCoord::new(0, 0),
            class: owner,
            weight: 1.0,
        },
        neighbors: Vec::new(),
        transition_strength: 0.0,
        barrier_strength: 0.0,
    }
}

#[cfg(test)]
fn resolve_surface_column_plan(
    region: RegionClassCell,
    smoothed: SmoothedColumn,
    hydrology: crate::world::HydrologyColumn,
    runtime: Option<&SurfaceRuntimeContext>,
) -> SurfaceColumnPlan {
    let influence = single_region_influence(region);

    resolve_surface_column_plan_with_influence(
        region, &influence, 0, 0, smoothed, hydrology, runtime,
    )
}

fn resolve_surface_column_plan_with_influence(
    region: RegionClassCell,
    influence: &RegionClassInfluenceSet,
    world_x: i32,
    world_z: i32,
    smoothed: SmoothedColumn,
    hydrology: crate::world::HydrologyColumn,
    runtime: Option<&SurfaceRuntimeContext>,
) -> SurfaceColumnPlan {
    let material_domain = sample_material_domain(MaterialDomainInput {
        hard_owner: region,
        influence,
        smoothed,
        hydrology,
        world_x,
        world_z,
    });
    let material_region = material_domain.visible_owner;
    let material_policy = resolve_material_policy_for_domain(
        material_domain.visible_domain,
        material_region.archetype,
    );
    let material = material_policy_def(material_policy)
        .expect("every archetype surface policy should resolve to a definition");
    let seasonal_state = resolve_seasonal_state(
        material_region.archetype,
        material_region.climate_regime,
        runtime,
    );
    let terrain_top_y = hydrology.terrain_height.floor() as i32;
    let water_top_y = quantize_water_top_y(region, hydrology, seasonal_state, terrain_top_y);
    let cover_phase = resolve_cover_phase(
        material_region.archetype,
        material_region.climate_regime,
        seasonal_state,
        hydrology,
        water_top_y,
    );
    let cover_override_key =
        resolve_cover_override_key(material_policy, seasonal_state, cover_phase, hydrology);
    let filler_depth = filler_depth_for_policy(material_policy, hydrology);
    let water_block_key = water_top_y.map(|_| resolve_water_block_key(region, seasonal_state));
    let material_transition = resolve_material_transition(
        material_policy,
        material_region,
        influence,
        smoothed,
        hydrology,
        cover_phase,
        cover_override_key,
        water_block_key,
        world_x,
        world_z,
    );
    let (top_block_key, filler_block_key, core_block_key) = resolve_block_stack(
        material,
        material_region.archetype,
        material_region.coastal_context,
        material_region.hydrology_context,
        smoothed,
        hydrology,
        seasonal_state,
        cover_phase,
        cover_override_key,
        water_block_key,
        material_transition,
        world_x,
        world_z,
    );

    SurfaceColumnPlan {
        owner_archetype: region.archetype,
        material_policy,
        seasonal_state,
        cover_phase,
        cover_override_key,
        terrain_top_y,
        water_top_y,
        top_block_key,
        filler_block_key,
        core_block_key,
        water_block_key,
        filler_depth,
    }
}

fn resolve_material_policy_for_domain(
    domain: MaterialDomainKind,
    fallback_archetype: RegionArchetype,
) -> MaterialPolicyId {
    match domain {
        MaterialDomainKind::MarineShelf => MaterialPolicyId::OceanicShelf,
        MaterialDomainKind::SandyCoast => MaterialPolicyId::SandyBeach,
        MaterialDomainKind::RockyCoast => MaterialPolicyId::CoastalCliff,
        MaterialDomainKind::Wetland => MaterialPolicyId::ColdWetland,
        MaterialDomainKind::TemperateGreen => MaterialPolicyId::TemperateGrassland,
        MaterialDomainKind::DryGrassland => MaterialPolicyId::SteppeGrassland,
        MaterialDomainKind::SavannaGrassland => MaterialPolicyId::SavannaGrassland,
        MaterialDomainKind::TropicalForest => match fallback_archetype {
            RegionArchetype::TropicalRainforestHills => MaterialPolicyId::TropicalHills,
            _ => MaterialPolicyId::TropicalLowland,
        },
        MaterialDomainKind::DesertDry => MaterialPolicyId::DesertSurface,
        MaterialDomainKind::ColdSparse => MaterialPolicyId::TundraExposure,
        MaterialDomainKind::AlpineRock => MaterialPolicyId::AlpineExposed,
    }
}

fn apply_visual_boundary_stepping(
    columns: &mut Vec<SurfaceColumnPlan>,
    grid_edge: i32,
    origin_x: i32,
    origin_z: i32,
    seed: u64,
) {
    if columns.len() != (grid_edge * grid_edge) as usize {
        return;
    }

    let original = columns.clone();
    let mut claims: Vec<Option<VisualBoundaryStepClaim>> = vec![None; original.len()];

    for depth in 0..3 {
        let previous_claims = claims.clone();
        let mut next_claims = previous_claims.clone();
        let threshold = match depth {
            0 => 0.56,
            1 => 0.54,
            _ => 0.60,
        };

        for local_z in 1..(grid_edge - 1) {
            for local_x in 1..(grid_edge - 1) {
                let index = grid_column_index(local_x, local_z, grid_edge);
                if previous_claims[index].is_some() {
                    continue;
                }

                let current = original[index];
                let world_x = origin_x + local_x;
                let world_z = origin_z + local_z;
                let mut best: Option<VisualBoundaryStepClaim> = None;

                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let nx = local_x + dx;
                    let nz = local_z + dz;
                    if !(0..grid_edge).contains(&nx) || !(0..grid_edge).contains(&nz) {
                        continue;
                    }

                    let neighbor_index = grid_column_index(nx, nz, grid_edge);
                    if visual_boundary_source_is_halo(nx, nz, grid_edge)
                        && !visual_boundary_has_internal_boundary(
                            &original, local_x, local_z, grid_edge,
                        )
                    {
                        continue;
                    }
                    let Some(source) = visual_boundary_step_source(
                        depth,
                        &original,
                        &previous_claims,
                        neighbor_index,
                    ) else {
                        continue;
                    };
                    if !visual_boundary_step_allowed(current, source) {
                        continue;
                    }

                    let score = visual_boundary_step_score(
                        world_x,
                        world_z,
                        seed,
                        current.material_policy,
                        source.material_policy,
                        dx,
                        dz,
                    );
                    if score <= threshold {
                        continue;
                    }

                    let claim = VisualBoundaryStepClaim {
                        source,
                        source_dx: dx,
                        source_dz: dz,
                        score,
                    };
                    if claim.score > best.map(|best| best.score).unwrap_or(f32::NEG_INFINITY) {
                        best = Some(claim);
                    }
                }

                if let Some(claim) = best {
                    next_claims[index] = Some(claim);
                }
            }
        }

        claims = next_claims;
    }

    for (index, claim) in claims.into_iter().enumerate() {
        let Some(claim) = claim else {
            continue;
        };
        let _ = (claim.source_dx, claim.source_dz);
        copy_visual_material(&mut columns[index], claim.source);
    }
}

fn visual_boundary_source_is_halo(local_x: i32, local_z: i32, grid_edge: i32) -> bool {
    local_x == 0 || local_z == 0 || local_x == grid_edge - 1 || local_z == grid_edge - 1
}

fn visual_boundary_has_internal_boundary(
    original: &[SurfaceColumnPlan],
    local_x: i32,
    local_z: i32,
    grid_edge: i32,
) -> bool {
    let current = original[grid_column_index(local_x, local_z, grid_edge)];
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let nx = local_x + dx;
        let nz = local_z + dz;
        if nx <= 0 || nz <= 0 || nx >= grid_edge - 1 || nz >= grid_edge - 1 {
            continue;
        }

        let neighbor = original[grid_column_index(nx, nz, grid_edge)];
        if current.top_block_key != neighbor.top_block_key
            || current.material_policy != neighbor.material_policy
        {
            return true;
        }
    }

    false
}

fn visual_boundary_step_source(
    depth: usize,
    original: &[SurfaceColumnPlan],
    previous_claims: &[Option<VisualBoundaryStepClaim>],
    neighbor_index: usize,
) -> Option<SurfaceColumnPlan> {
    if depth == 0 {
        Some(original[neighbor_index])
    } else {
        previous_claims[neighbor_index].map(|claim| claim.source)
    }
}

fn visual_boundary_step_allowed(current: SurfaceColumnPlan, neighbor: SurfaceColumnPlan) -> bool {
    if current.top_block_key == neighbor.top_block_key
        && current.material_policy == neighbor.material_policy
    {
        return false;
    }
    if current.water_top_y.is_some()
        || neighbor.water_top_y.is_some()
        || current.water_block_key.is_some()
        || neighbor.water_block_key.is_some()
    {
        return false;
    }
    if current.cover_override_key.is_some() || neighbor.cover_override_key.is_some() {
        return false;
    }
    if current.terrain_top_y.abs_diff(neighbor.terrain_top_y) > 8 {
        return false;
    }

    true
}

fn visual_boundary_step_score(
    world_x: i32,
    world_z: i32,
    seed: u64,
    current: MaterialPolicyId,
    neighbor: MaterialPolicyId,
    dx: i32,
    dz: i32,
) -> f32 {
    let salt = MATERIAL_BOUNDARY_STEPPING_SALT
        ^ seed
        ^ ((current as u64).wrapping_mul(0xA24B_AED4_963E_E407))
        ^ ((neighbor as u64).wrapping_mul(0x9FB2_1C65_1E98_DF25))
        ^ ((dx as i64 as u64).wrapping_mul(0xD6E8_FD9D_52C7_129B))
        ^ ((dz as i64 as u64).wrapping_mul(0xA5A3_56B9_77E3_45CF));
    let short = value_noise_2d(world_x as f32, world_z as f32, 3.0, salt.rotate_left(13));
    let medium = value_noise_2d(
        world_x as f32 + 11.0,
        world_z as f32 - 7.0,
        5.0,
        salt.rotate_left(37),
    );
    let bias = hash_lattice_to_unit(
        world_x.div_euclid(2) + dx,
        world_z.div_euclid(2) + dz,
        salt.rotate_left(51),
    );

    short * 0.38 + medium * 0.44 + bias * 0.18
}

fn copy_visual_material(target: &mut SurfaceColumnPlan, source: SurfaceColumnPlan) {
    target.material_policy = source.material_policy;
    target.seasonal_state = source.seasonal_state;
    target.cover_phase = source.cover_phase;
    target.cover_override_key = source.cover_override_key;
    target.top_block_key = source.top_block_key;
    target.filler_block_key = source.filler_block_key;
    target.core_block_key = source.core_block_key;
    target.filler_depth = source.filler_depth;
}

fn resolve_seasonal_state(
    archetype: RegionArchetype,
    climate: ClimateRegime,
    runtime: Option<&SurfaceRuntimeContext>,
) -> Option<SeasonalBiomeStateId> {
    match runtime.and_then(|ctx| ctx.seasonal_phase) {
        Some(SeasonalPhase::Winter) => match archetype {
            RegionArchetype::ColdWetLowland => Some(SeasonalBiomeStateId::ColdFrozenWetland),
            RegionArchetype::TemperatePlain
            | RegionArchetype::TemperateHills
            | RegionArchetype::TemperatePlateau
            | RegionArchetype::SteppePlain => Some(SeasonalBiomeStateId::TemperateSnowy),
            RegionArchetype::GlaciatedAlpine => Some(SeasonalBiomeStateId::AlpineSnowpack),
            RegionArchetype::CoastalCliffland | RegionArchetype::SandyBeachPlain => {
                Some(SeasonalBiomeStateId::CoastalStormSeason)
            }
            _ if matches!(climate, ClimateRegime::Polar | ClimateRegime::ColdAlpine) => {
                Some(SeasonalBiomeStateId::AlpineSnowpack)
            }
            _ => None,
        },
        Some(SeasonalPhase::Spring) | Some(SeasonalPhase::Summer) => match archetype {
            RegionArchetype::TemperatePlain
            | RegionArchetype::TemperateHills
            | RegionArchetype::TemperatePlateau => Some(SeasonalBiomeStateId::TemperateGrowing),
            _ => None,
        },
        Some(SeasonalPhase::Autumn) => match archetype {
            RegionArchetype::CoastalCliffland | RegionArchetype::SandyBeachPlain => {
                Some(SeasonalBiomeStateId::CoastalStormSeason)
            }
            _ => None,
        },
        Some(SeasonalPhase::WetSeason) => match archetype {
            RegionArchetype::SavannaPlain
            | RegionArchetype::TropicalRainforestLowland
            | RegionArchetype::TropicalRainforestHills => {
                Some(SeasonalBiomeStateId::TropicalWetSeason)
            }
            _ => None,
        },
        Some(SeasonalPhase::DrySeason) => match archetype {
            RegionArchetype::SavannaPlain
            | RegionArchetype::TropicalRainforestLowland
            | RegionArchetype::TropicalRainforestHills => {
                Some(SeasonalBiomeStateId::TropicalDrySeason)
            }
            _ => None,
        },
        Some(SeasonalPhase::Thaw) => match archetype {
            RegionArchetype::GlaciatedAlpine => Some(SeasonalBiomeStateId::AlpineSnowpack),
            _ => None,
        },
        None => None,
    }
}

fn quantize_water_top_y(
    region: crate::world::RegionClassCell,
    hydrology: crate::world::HydrologyColumn,
    _seasonal_state: Option<SeasonalBiomeStateId>,
    terrain_top_y: i32,
) -> Option<i32> {
    let hydrology_surface = hydrology
        .water_surface_height
        .and_then(|height| height.is_finite().then_some(height.ceil() as i32));
    let marine_surface = if matches!(region.archetype, RegionArchetype::OceanicShelf)
        || (matches!(region.coastal_context, CoastalContext::Marine)
            && hydrology.terrain_height < SEA_LEVEL_Y as f32 - 0.25)
        || (matches!(region.coastal_context, CoastalContext::Coastal)
            && hydrology.terrain_height < SEA_LEVEL_Y as f32 - 0.90)
    {
        Some(SEA_LEVEL_Y)
    } else {
        None
    };

    hydrology_surface
        .or(marine_surface)
        .filter(|water_top_y| *water_top_y > terrain_top_y)
}

fn resolve_cover_phase(
    archetype: RegionArchetype,
    climate: ClimateRegime,
    seasonal_state: Option<SeasonalBiomeStateId>,
    hydrology: crate::world::HydrologyColumn,
    water_top_y: Option<i32>,
) -> CoverPhase {
    if matches!(
        seasonal_state,
        Some(SeasonalBiomeStateId::TemperateSnowy | SeasonalBiomeStateId::AlpineSnowpack)
    ) || matches!(archetype, RegionArchetype::GlaciatedAlpine)
    {
        return CoverPhase::SnowCovered;
    }

    if matches!(
        seasonal_state,
        Some(SeasonalBiomeStateId::ColdFrozenWetland)
    ) || freezes_surface_water(archetype, seasonal_state)
    {
        return CoverPhase::Frozen;
    }

    let strong_surface_wetness = water_top_y.is_some()
        || matches!(hydrology.mode, HydrologyMode::Lake | HydrologyMode::Wetland)
        || (matches!(
            hydrology.mode,
            HydrologyMode::Channel | HydrologyMode::Floodplain
        ) && hydrology.saturation >= 0.70)
        || hydrology.saturation >= 0.72;
    if strong_surface_wetness {
        return CoverPhase::Saturated;
    }

    if matches!(
        seasonal_state,
        Some(SeasonalBiomeStateId::TropicalDrySeason)
    ) || matches!(climate, ClimateRegime::AridHot)
    {
        return CoverPhase::Dry;
    }

    if matches!(
        seasonal_state,
        Some(SeasonalBiomeStateId::TemperateGrowing | SeasonalBiomeStateId::TropicalWetSeason)
    ) {
        return CoverPhase::Growing;
    }

    CoverPhase::Dormant
}

fn resolve_cover_override_key(
    material_policy: MaterialPolicyId,
    seasonal_state: Option<SeasonalBiomeStateId>,
    cover_phase: CoverPhase,
    hydrology: crate::world::HydrologyColumn,
) -> Option<&'static str> {
    match seasonal_state {
        Some(SeasonalBiomeStateId::TemperateSnowy)
            if matches!(
                material_policy,
                MaterialPolicyId::TemperateGrassland
                    | MaterialPolicyId::TemperatePlateau
                    | MaterialPolicyId::SteppeGrassland
                    | MaterialPolicyId::SavannaGrassland
            ) =>
        {
            Some("snowy_grass")
        }
        Some(SeasonalBiomeStateId::ColdFrozenWetland)
            if matches!(cover_phase, CoverPhase::Frozen) && hydrology.saturation >= 0.40 =>
        {
            Some("frozen_mud")
        }
        Some(SeasonalBiomeStateId::TropicalWetSeason)
            if matches!(
                material_policy,
                MaterialPolicyId::SavannaGrassland
                    | MaterialPolicyId::TropicalLowland
                    | MaterialPolicyId::TropicalHills
            ) =>
        {
            Some("wet_season_greening")
        }
        _ => None,
    }
}

fn resolve_block_stack(
    material: &MaterialPolicyDef,
    archetype: RegionArchetype,
    coastal_context: CoastalContext,
    hydrology_context: HydrologyContext,
    smoothed: SmoothedColumn,
    hydrology: crate::world::HydrologyColumn,
    _seasonal_state: Option<SeasonalBiomeStateId>,
    cover_phase: CoverPhase,
    cover_override_key: Option<&'static str>,
    water_block_key: Option<&'static str>,
    material_transition: Option<MaterialTransitionSelection>,
    _world_x: i32,
    _world_z: i32,
) -> (&'static str, &'static str, &'static str) {
    let mut top = material.default_top_block;
    let mut filler = material.subsoil_block;
    let mut core = material.deep_block;
    let steepness = smoothed.local_slope;
    let steep_threshold = rock_exposure_threshold(material.id);
    let exposure_breakup = smoothstep_range(0.20, 1.10, smoothed.concavity.max(0.0)) * 0.12
        + smoothstep_range(0.26, 0.58, hydrology.gravel_bar_strength) * 0.10;
    let exposed_steepness = steepness + exposure_breakup;
    let hydrology_override_dominates = water_block_key.is_some()
        || matches!(hydrology.mode, HydrologyMode::Lake | HydrologyMode::Wetland)
        || (matches!(
            hydrology.mode,
            HydrologyMode::Channel | HydrologyMode::Floodplain
        ) && hydrology.saturation >= 0.70)
        || hydrology.saturation >= 0.82;
    if hydrology.gravel_bar_strength >= 0.38 {
        top = if water_block_key.is_some()
            && matches!(
                coastal_context,
                CoastalContext::Coastal | CoastalContext::Marine
            ) {
            "wet_gravel"
        } else {
            "gravel"
        };
        filler = if matches!(
            coastal_context,
            CoastalContext::Coastal | CoastalContext::Marine
        ) {
            "gravel"
        } else {
            material.sediment_block
        };
    } else if let Some(transition) = material_transition {
        let transition_material = material_policy_def(transition.policy)
            .expect("transition surface policy should resolve to a definition");
        let (transition_top, transition_filler, transition_core) = transition_block_stack(
            material,
            transition_material,
            transition,
            smoothed,
            cover_phase,
        );
        top = transition_top;
        filler = transition_filler;
        core = transition_core;
    } else if hydrology_override_dominates {
        let (wet_top, wet_filler, wet_core) = hydrology_stack_override(
            material.id,
            archetype,
            hydrology_context,
            hydrology,
            water_block_key,
        );
        top = wet_top;
        filler = wet_filler;
        core = wet_core;
    } else if matches!(cover_phase, CoverPhase::Frozen) {
        top = material.frozen_top_block;
    } else if matches!(cover_phase, CoverPhase::Dry) {
        top = material.dry_top_block;
    } else if matches!(cover_phase, CoverPhase::Saturated) {
        top = material.wet_top_block;
    }

    match cover_override_key {
        Some("snowy_grass") => {
            top = "snow";
        }
        Some("frozen_mud") => {
            top = if water_block_key.is_some() {
                "ice"
            } else {
                "snow"
            };
            filler = "mud";
        }
        Some("wet_season_greening") => {
            top = match material.id {
                MaterialPolicyId::SavannaGrassland => "grass",
                MaterialPolicyId::TropicalLowland => "jungle_grass",
                MaterialPolicyId::TropicalHills => "moss",
                _ => top,
            };
        }
        _ => {}
    }

    if water_block_key.is_none() {
        let relative_sea_height = hydrology.terrain_height - SEA_LEVEL_Y as f32;
        match material.id {
            MaterialPolicyId::OceanicShelf if matches!(top, "sand" | "silt") => {
                if relative_sea_height >= 1.15 || exposed_steepness >= 0.58 {
                    top = if exposed_steepness >= 0.82 {
                        "rock"
                    } else {
                        "gravel"
                    };
                    filler = "sand";
                } else if relative_sea_height >= 0.35 && smoothed.concavity.max(0.0) >= 0.22 {
                    top = "wet_gravel";
                    filler = "sand";
                }
            }
            MaterialPolicyId::SandyBeach if matches!(top, "sand" | "wet_sand" | "silt") => {
                if exposed_steepness >= 0.72 || hydrology.gravel_bar_strength >= 0.30 {
                    top = if matches!(top, "wet_sand" | "silt") {
                        "wet_gravel"
                    } else {
                        "gravel"
                    };
                    filler = "sand";
                }
            }
            MaterialPolicyId::CoastalCliff if matches!(top, "gravel") => {
                if exposed_steepness >= steep_threshold - 0.10 {
                    top = "rock";
                    filler = "gravel";
                }
            }
            _ => {}
        }
    }

    (top, filler, core)
}

fn material_influence_sample_position(
    world_x: i32,
    world_z: i32,
    _smoothed: SmoothedColumn,
    _hydrology: crate::world::HydrologyColumn,
) -> (f32, f32) {
    let x = world_x as f32 + 0.5;
    let z = world_z as f32 + 0.5;

    (x, z)
}

fn resolve_material_transition(
    owner_policy: MaterialPolicyId,
    owner_region: RegionClassCell,
    influence: &RegionClassInfluenceSet,
    smoothed: SmoothedColumn,
    hydrology: crate::world::HydrologyColumn,
    cover_phase: CoverPhase,
    cover_override_key: Option<&'static str>,
    water_block_key: Option<&'static str>,
    world_x: i32,
    world_z: i32,
) -> Option<MaterialTransitionSelection> {
    if influence.dominant.class.archetype != owner_region.archetype
        || cover_override_key.is_some()
        || matches!(cover_phase, CoverPhase::Frozen | CoverPhase::SnowCovered)
        || hydrology_material_dominates(hydrology, water_block_key)
    {
        return None;
    }

    let owner_weight = region_influence_policy_weight(influence, owner_policy);
    let (candidate_policy, candidate_region, candidate_weight) =
        strongest_non_owner_policy(influence, owner_policy)?;
    if candidate_weight < 0.14 || owner_weight >= 0.90 {
        return None;
    }

    let compatibility = material_transition_compatibility(
        owner_policy,
        candidate_policy,
        owner_region,
        candidate_region,
    );
    if compatibility <= 0.40 {
        return None;
    }

    let mixed_weight = (candidate_weight * 1.25
        + (1.0 - owner_weight).max(0.0) * 0.55
        + influence.transition_strength.clamp(0.0, 1.0) * 0.18)
        .clamp(0.0, 1.0);
    let strength = (smoothstep_range(0.14, 0.58, mixed_weight) * compatibility).clamp(0.0, 1.0);
    if strength < 0.16 {
        return None;
    }

    let boundary_displacement = material_transition_boundary_displacement(
        world_x,
        world_z,
        owner_region.archetype,
        candidate_region.archetype,
    );
    let _ = smoothed;
    let coarse_edge_score = candidate_weight
        - owner_weight * (0.66 + influence.barrier_strength.clamp(0.0, 1.0) * 0.18)
        + influence.transition_strength.clamp(0.0, 1.0) * 0.08;
    let edge_score = coarse_edge_score + boundary_displacement * 0.025;
    if edge_score <= 0.0 {
        return None;
    }
    let boundary_value = (edge_score / 0.34).clamp(0.0, 1.0);

    Some(MaterialTransitionSelection {
        policy: candidate_policy,
        strength,
        boundary_value,
    })
}

fn transition_block_stack(
    owner: &MaterialPolicyDef,
    candidate: &MaterialPolicyDef,
    transition: MaterialTransitionSelection,
    _smoothed: SmoothedColumn,
    _cover_phase: CoverPhase,
) -> (&'static str, &'static str, &'static str) {
    let boundary_phase = transition.boundary_value;
    let candidate_selected = boundary_phase < (0.58 + transition.strength * 0.18).clamp(0.58, 0.76);
    let top = if candidate_selected {
        candidate.default_top_block
    } else {
        owner.default_top_block
    };

    let filler = if candidate_selected {
        candidate.subsoil_block
    } else {
        owner.subsoil_block
    };
    let core = if candidate_selected && boundary_phase < transition.strength * 0.42 {
        candidate.deep_block
    } else {
        owner.deep_block
    };

    (top, filler, core)
}

fn hydrology_stack_override(
    material_policy: MaterialPolicyId,
    archetype: RegionArchetype,
    hydrology_context: HydrologyContext,
    hydrology: crate::world::HydrologyColumn,
    water_block_key: Option<&'static str>,
) -> (&'static str, &'static str, &'static str) {
    if hydrology.gravel_bar_strength >= 0.52 {
        return (
            if water_block_key.is_some() {
                "wet_gravel"
            } else {
                "gravel"
            },
            "gravel",
            "stone",
        );
    }

    match material_policy {
        MaterialPolicyId::OceanicShelf => ("silt", "clay", "stone"),
        MaterialPolicyId::SandyBeach => {
            if matches!(
                hydrology.mode,
                HydrologyMode::Lake | HydrologyMode::Floodplain
            ) {
                ("silt", "sand", "stone")
            } else {
                ("wet_sand", "sand", "stone")
            }
        }
        MaterialPolicyId::CoastalCliff => ("wet_gravel", "gravel", "stone"),
        MaterialPolicyId::TemperateGrassland | MaterialPolicyId::TemperatePlateau => {
            if matches!(hydrology.mode, HydrologyMode::Wetland | HydrologyMode::Lake) {
                ("mud", "clay", "stone")
            } else {
                ("silt", "dirt", "stone")
            }
        }
        MaterialPolicyId::SteppeGrassland | MaterialPolicyId::SavannaGrassland => {
            if matches!(hydrology.mode, HydrologyMode::Lake | HydrologyMode::Wetland) {
                ("mud", "clay", "stone")
            } else {
                ("silt", "dirt", "stone")
            }
        }
        MaterialPolicyId::DesertSurface => match hydrology.mode {
            HydrologyMode::Lake => ("clay", "silt", "sandstone"),
            HydrologyMode::Floodplain => ("wet_sand", "sand", "sandstone"),
            _ => ("gravel", "sand", "sandstone"),
        },
        MaterialPolicyId::TropicalLowland | MaterialPolicyId::TropicalHills => {
            if matches!(hydrology.mode, HydrologyMode::Wetland) {
                (
                    "peat",
                    "mud",
                    if matches!(archetype, RegionArchetype::TropicalRainforestHills) {
                        "stone"
                    } else {
                        "dirt"
                    },
                )
            } else {
                (
                    "mud",
                    "clay",
                    if matches!(archetype, RegionArchetype::TropicalRainforestHills) {
                        "stone"
                    } else {
                        "dirt"
                    },
                )
            }
        }
        MaterialPolicyId::ColdWetland => {
            if matches!(hydrology.mode, HydrologyMode::Wetland | HydrologyMode::Lake)
                || matches!(
                    hydrology_context,
                    HydrologyContext::WetLowland | HydrologyContext::LakeBasin
                )
            {
                ("peat", "mud", "dirt")
            } else {
                ("mud", "clay", "dirt")
            }
        }
        MaterialPolicyId::AlpineExposed | MaterialPolicyId::TundraExposure => {
            if water_block_key == Some("ice") {
                ("ice", "gravel", "stone")
            } else {
                ("gravel", "moraine", "stone")
            }
        }
    }
}

fn resolve_water_block_key(
    region: crate::world::RegionClassCell,
    seasonal_state: Option<SeasonalBiomeStateId>,
) -> &'static str {
    if freezes_surface_water(region.archetype, seasonal_state)
        || (matches!(
            region.climate_regime,
            ClimateRegime::Polar | ClimateRegime::ColdAlpine
        ) && !matches!(region.archetype, RegionArchetype::OceanicShelf))
    {
        "ice"
    } else {
        "water"
    }
}

fn freezes_surface_water(
    archetype: RegionArchetype,
    seasonal_state: Option<SeasonalBiomeStateId>,
) -> bool {
    matches!(
        seasonal_state,
        Some(SeasonalBiomeStateId::ColdFrozenWetland | SeasonalBiomeStateId::AlpineSnowpack)
    ) || matches!(
        archetype,
        RegionArchetype::GlaciatedAlpine
            | RegionArchetype::GlacialValley
            | RegionArchetype::CrevassedIcefield
            | RegionArchetype::AlpineRavineCountry
    )
}

fn filler_depth_for_policy(
    material_policy: MaterialPolicyId,
    hydrology: crate::world::HydrologyColumn,
) -> u8 {
    let base: u8 = match material_policy {
        MaterialPolicyId::OceanicShelf => 3,
        MaterialPolicyId::SandyBeach => 3,
        MaterialPolicyId::CoastalCliff => 2,
        MaterialPolicyId::TemperateGrassland => 4,
        MaterialPolicyId::TemperatePlateau => 3,
        MaterialPolicyId::SteppeGrassland => 3,
        MaterialPolicyId::DesertSurface => 3,
        MaterialPolicyId::SavannaGrassland => 3,
        MaterialPolicyId::TropicalLowland => 5,
        MaterialPolicyId::TropicalHills => 4,
        MaterialPolicyId::ColdWetland => 4,
        MaterialPolicyId::AlpineExposed => 2,
        MaterialPolicyId::TundraExposure => 3,
    };

    if matches!(hydrology.mode, HydrologyMode::Wetland | HydrologyMode::Lake) {
        base.saturating_add(1)
    } else {
        base
    }
}

fn rock_exposure_threshold(material_policy: MaterialPolicyId) -> f32 {
    match material_policy {
        MaterialPolicyId::CoastalCliff | MaterialPolicyId::AlpineExposed => 0.75,
        MaterialPolicyId::TemperatePlateau | MaterialPolicyId::TropicalHills => 0.90,
        MaterialPolicyId::TundraExposure => 0.95,
        MaterialPolicyId::DesertSurface => 1.05,
        MaterialPolicyId::TemperateGrassland
        | MaterialPolicyId::SteppeGrassland
        | MaterialPolicyId::SavannaGrassland
        | MaterialPolicyId::TropicalLowland
        | MaterialPolicyId::ColdWetland
        | MaterialPolicyId::OceanicShelf
        | MaterialPolicyId::SandyBeach => 1.15,
    }
}

fn strongest_region_for_policy(
    influence: &RegionClassInfluenceSet,
    policy: MaterialPolicyId,
) -> RegionClassCell {
    let mut strongest = influence.dominant.class;
    let mut strongest_weight =
        if resolve_material_policy_for_archetype(strongest.archetype) == policy {
            influence.dominant.weight
        } else {
            f32::NEG_INFINITY
        };

    for sample in &influence.neighbors {
        if resolve_material_policy_for_archetype(sample.class.archetype) == policy
            && sample.weight > strongest_weight
        {
            strongest = sample.class;
            strongest_weight = sample.weight;
        }
    }

    strongest
}

fn region_influence_policy_weight(
    influence: &RegionClassInfluenceSet,
    policy: MaterialPolicyId,
) -> f32 {
    let dominant_weight =
        if resolve_material_policy_for_archetype(influence.dominant.class.archetype) == policy {
            influence.dominant.weight
        } else {
            0.0
        };

    dominant_weight
        + influence
            .neighbors
            .iter()
            .filter(|sample| {
                resolve_material_policy_for_archetype(sample.class.archetype) == policy
            })
            .map(|sample| sample.weight)
            .sum::<f32>()
}

fn strongest_non_owner_policy(
    influence: &RegionClassInfluenceSet,
    owner_policy: MaterialPolicyId,
) -> Option<(MaterialPolicyId, RegionClassCell, f32)> {
    let mut best = None;
    let mut best_weight = 0.0_f32;

    for sample in std::iter::once(&influence.dominant).chain(influence.neighbors.iter()) {
        let policy = resolve_material_policy_for_archetype(sample.class.archetype);
        if policy == owner_policy {
            continue;
        }

        let weight = region_influence_policy_weight(influence, policy);
        if weight > best_weight {
            best = Some((
                policy,
                strongest_region_for_policy(influence, policy),
                weight,
            ));
            best_weight = weight;
        }
    }

    best
}

fn material_transition_compatibility(
    owner_policy: MaterialPolicyId,
    candidate_policy: MaterialPolicyId,
    owner_region: RegionClassCell,
    candidate_region: RegionClassCell,
) -> f32 {
    if owner_policy == candidate_policy {
        return 0.0;
    }

    let mut compatibility: f32 = match (owner_policy, candidate_policy) {
        (MaterialPolicyId::OceanicShelf, _) | (_, MaterialPolicyId::OceanicShelf) => 0.12,
        (MaterialPolicyId::CoastalCliff, MaterialPolicyId::SandyBeach)
        | (MaterialPolicyId::SandyBeach, MaterialPolicyId::CoastalCliff) => 0.48,
        (MaterialPolicyId::SandyBeach, MaterialPolicyId::DesertSurface)
        | (MaterialPolicyId::DesertSurface, MaterialPolicyId::SandyBeach) => 0.82,
        (MaterialPolicyId::TemperateGrassland, MaterialPolicyId::SteppeGrassland)
        | (MaterialPolicyId::SteppeGrassland, MaterialPolicyId::TemperateGrassland)
        | (MaterialPolicyId::TemperateGrassland, MaterialPolicyId::SavannaGrassland)
        | (MaterialPolicyId::SavannaGrassland, MaterialPolicyId::TemperateGrassland)
        | (MaterialPolicyId::SteppeGrassland, MaterialPolicyId::SavannaGrassland)
        | (MaterialPolicyId::SavannaGrassland, MaterialPolicyId::SteppeGrassland)
        | (MaterialPolicyId::TemperatePlateau, MaterialPolicyId::TemperateGrassland)
        | (MaterialPolicyId::TemperateGrassland, MaterialPolicyId::TemperatePlateau)
        | (MaterialPolicyId::TemperatePlateau, MaterialPolicyId::SteppeGrassland)
        | (MaterialPolicyId::SteppeGrassland, MaterialPolicyId::TemperatePlateau)
        | (MaterialPolicyId::TemperatePlateau, MaterialPolicyId::SavannaGrassland)
        | (MaterialPolicyId::SavannaGrassland, MaterialPolicyId::TemperatePlateau) => 0.86,
        (MaterialPolicyId::DesertSurface, MaterialPolicyId::SteppeGrassland)
        | (MaterialPolicyId::SteppeGrassland, MaterialPolicyId::DesertSurface)
        | (MaterialPolicyId::DesertSurface, MaterialPolicyId::SavannaGrassland)
        | (MaterialPolicyId::SavannaGrassland, MaterialPolicyId::DesertSurface)
        | (MaterialPolicyId::DesertSurface, MaterialPolicyId::TemperatePlateau)
        | (MaterialPolicyId::TemperatePlateau, MaterialPolicyId::DesertSurface)
        | (MaterialPolicyId::DesertSurface, MaterialPolicyId::TemperateGrassland)
        | (MaterialPolicyId::TemperateGrassland, MaterialPolicyId::DesertSurface) => 0.74,
        (MaterialPolicyId::TropicalLowland, MaterialPolicyId::TropicalHills)
        | (MaterialPolicyId::TropicalHills, MaterialPolicyId::TropicalLowland) => 0.88,
        (MaterialPolicyId::TropicalLowland, MaterialPolicyId::SavannaGrassland)
        | (MaterialPolicyId::SavannaGrassland, MaterialPolicyId::TropicalLowland)
        | (MaterialPolicyId::TropicalHills, MaterialPolicyId::SavannaGrassland)
        | (MaterialPolicyId::SavannaGrassland, MaterialPolicyId::TropicalHills) => 0.58,
        (MaterialPolicyId::ColdWetland, MaterialPolicyId::TemperateGrassland)
        | (MaterialPolicyId::TemperateGrassland, MaterialPolicyId::ColdWetland)
        | (MaterialPolicyId::ColdWetland, MaterialPolicyId::TundraExposure)
        | (MaterialPolicyId::TundraExposure, MaterialPolicyId::ColdWetland) => 0.46,
        (MaterialPolicyId::ColdWetland, MaterialPolicyId::AlpineExposed)
        | (MaterialPolicyId::AlpineExposed, MaterialPolicyId::ColdWetland) => 0.44,
        (MaterialPolicyId::AlpineExposed, MaterialPolicyId::TundraExposure)
        | (MaterialPolicyId::TundraExposure, MaterialPolicyId::AlpineExposed)
        | (MaterialPolicyId::AlpineExposed, MaterialPolicyId::TemperatePlateau)
        | (MaterialPolicyId::TemperatePlateau, MaterialPolicyId::AlpineExposed) => 0.50,
        (MaterialPolicyId::CoastalCliff, _) | (_, MaterialPolicyId::CoastalCliff) => 0.30,
        _ => 0.34,
    };

    if owner_region.biome_family == candidate_region.biome_family {
        compatibility += 0.10;
    }
    if owner_region.terrain_form_family == candidate_region.terrain_form_family {
        compatibility += 0.08;
    }
    if owner_region.coastal_context != candidate_region.coastal_context
        && (matches!(owner_region.coastal_context, CoastalContext::Marine)
            || matches!(candidate_region.coastal_context, CoastalContext::Marine))
    {
        compatibility *= 0.45;
    }
    if matches!(
        (
            owner_region.hydrology_context,
            candidate_region.hydrology_context
        ),
        (HydrologyContext::Dryland, HydrologyContext::WetLowland)
            | (HydrologyContext::WetLowland, HydrologyContext::Dryland)
            | (HydrologyContext::Dryland, HydrologyContext::LakeBasin)
            | (HydrologyContext::LakeBasin, HydrologyContext::Dryland)
    ) {
        compatibility *= 0.72;
    }

    compatibility.clamp(0.0, 1.0)
}

fn hydrology_material_dominates(
    hydrology: crate::world::HydrologyColumn,
    water_block_key: Option<&'static str>,
) -> bool {
    water_block_key.is_some()
        || hydrology.water_surface_height.is_some()
        || hydrology.saturation >= 0.70
        || hydrology.gravel_bar_strength >= 0.18
        || matches!(
            hydrology.mode,
            HydrologyMode::Channel | HydrologyMode::Lake | HydrologyMode::Wetland
        )
}

fn material_transition_boundary_displacement(
    world_x: i32,
    world_z: i32,
    owner: RegionArchetype,
    candidate: RegionArchetype,
) -> f32 {
    let salt = MATERIAL_TRANSITION_SALT
        ^ ((owner as u64).wrapping_mul(0xA24B_AED4_963E_E407))
        ^ ((candidate as u64).wrapping_mul(0x9FB2_1C65_1E98_DF25));
    let medium = signed_value_noise_2d(
        world_x as f32 + 31.0,
        world_z as f32 - 17.0,
        227.0,
        salt.rotate_left(17),
    );
    let broad = signed_value_noise_2d(
        world_x as f32 - 103.0,
        world_z as f32 + 71.0,
        557.0,
        salt.rotate_left(31),
    );
    let subchunk = signed_value_noise_2d(
        world_x as f32 + 29.0,
        world_z as f32 - 11.0,
        21.0,
        (salt ^ MATERIAL_TRANSITION_FINE_SALT).rotate_left(9),
    );
    let fine = signed_value_noise_2d(
        world_x as f32 - 7.0,
        world_z as f32 + 13.0,
        7.0,
        (salt ^ MATERIAL_TRANSITION_FINE_SALT).rotate_left(39),
    );

    ((medium * 0.34 + broad * 0.22 + subchunk * 0.28 + fine * 0.16) * 0.34).clamp(-1.0, 1.0)
}

fn signed_value_noise_2d(world_x: f32, world_z: f32, period: f32, salt: u64) -> f32 {
    value_noise_2d(world_x, world_z, period, salt) * 2.0 - 1.0
}

fn value_noise_2d(world_x: f32, world_z: f32, period: f32, salt: u64) -> f32 {
    let sample_x = world_x / period.max(1.0);
    let sample_z = world_z / period.max(1.0);
    let base_x = sample_x.floor() as i32;
    let base_z = sample_z.floor() as i32;
    let tx = smootherstep01(sample_x - base_x as f32);
    let tz = smootherstep01(sample_z - base_z as f32);
    let v00 = hash_lattice_to_unit(base_x, base_z, salt);
    let v10 = hash_lattice_to_unit(base_x + 1, base_z, salt);
    let v01 = hash_lattice_to_unit(base_x, base_z + 1, salt);
    let v11 = hash_lattice_to_unit(base_x + 1, base_z + 1, salt);
    let north = lerp_f32(v00, v10, tx);
    let south = lerp_f32(v01, v11, tx);

    lerp_f32(north, south, tz)
}

fn hash_lattice_to_unit(x: i32, z: i32, salt: u64) -> f32 {
    let mut hash = salt;
    hash ^= (x as i64 as u64).wrapping_mul(MATERIAL_TRANSITION_HASH_K1);
    hash = hash.rotate_left(27);
    hash ^= (z as i64 as u64).wrapping_mul(MATERIAL_TRANSITION_HASH_K2);
    hash = splitmix64(hash);

    ((hash >> 40) as u32 as f32) / ((1_u32 << 24) as f32)
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn smoothstep_range(edge0: f32, edge1: f32, value: f32) -> f32 {
    let width = edge1 - edge0;
    if width.abs() <= f32::EPSILON {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }

    smootherstep01((value - edge0) / width)
}

fn smootherstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn column_index(local_x: i32, local_z: i32) -> usize {
    grid_column_index(local_x, local_z, CHUNK_EDGE_I32)
}

fn grid_column_index(local_x: i32, local_z: i32, grid_edge: i32) -> usize {
    local_z as usize * grid_edge as usize + local_x as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::atlas::{
        BiomeFamily, ClimateRegime, CoastalContext, ElevationBand, HydrologyContext, MoistureBand,
        RegionClassCell, ReliefClass, TemperatureBand, TerrainFormFamily,
    };

    fn test_region(archetype: RegionArchetype) -> RegionClassCell {
        let mut region = RegionClassCell {
            temperature_band: TemperatureBand::Temperate,
            moisture_band: MoistureBand::Subhumid,
            elevation_band: ElevationBand::Low,
            relief_class: ReliefClass::Plain,
            hydrology_context: HydrologyContext::WellDrained,
            coastal_context: CoastalContext::Inland,
            climate_regime: ClimateRegime::TemperateSeasonal,
            biome_family: BiomeFamily::TemperateGrassland,
            terrain_form_family: TerrainFormFamily::Plain,
            archetype,
        };

        match archetype {
            RegionArchetype::DesertPlain | RegionArchetype::DesertDuneField => {
                region.temperature_band = TemperatureBand::Hot;
                region.moisture_band = MoistureBand::Arid;
                region.hydrology_context = HydrologyContext::Dryland;
                region.climate_regime = ClimateRegime::AridHot;
                region.biome_family = BiomeFamily::Desert;
                region.terrain_form_family = TerrainFormFamily::Plain;
            }
            RegionArchetype::SteppePlain => {
                region.moisture_band = MoistureBand::SemiArid;
                region.hydrology_context = HydrologyContext::Dryland;
                region.biome_family = BiomeFamily::Steppe;
            }
            _ => {}
        }

        region
    }

    fn test_smoothed_column() -> SmoothedColumn {
        SmoothedColumn {
            height: 8.4,
            remaining_relief_budget: 4.0,
            local_slope: 0.25,
            concavity: 0.10,
            material_support: crate::world::RealizationMaterialSupport {
                wetness: 0.18,
                exposure: 0.22,
                sediment: 0.20,
                soil_cover: 0.62,
            },
        }
    }

    fn dry_hydrology_column() -> crate::world::HydrologyColumn {
        crate::world::HydrologyColumn {
            terrain_height: 8.4,
            water_surface_height: None,
            channel_floor_height: None,
            saturation: 0.0,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Dry,
        }
    }

    fn mixed_influence(
        owner: RegionClassCell,
        candidate: RegionClassCell,
        candidate_weight: f32,
    ) -> RegionClassInfluenceSet {
        let candidate_weight = candidate_weight.clamp(0.0, 1.0);
        let owner_weight = 1.0 - candidate_weight;

        RegionClassInfluenceSet {
            dominant: RegionClassInfluence {
                coord: AtlasCoord::new(0, 0),
                class: owner,
                weight: owner_weight,
            },
            neighbors: vec![RegionClassInfluence {
                coord: AtlasCoord::new(1, 0),
                class: candidate,
                weight: candidate_weight,
            }],
            transition_strength: candidate_weight,
            barrier_strength: 0.0,
        }
    }

    fn find_dry_transition_plan(
        owner: RegionClassCell,
        candidate: RegionClassCell,
    ) -> (i32, i32, SurfaceColumnPlan) {
        let influence = mixed_influence(owner, candidate, 0.48);
        let smoothed = test_smoothed_column();
        let hydrology = dry_hydrology_column();

        for world_z in -32..=32 {
            for world_x in -32..=32 {
                let plan = resolve_surface_column_plan_with_influence(
                    owner, &influence, world_x, world_z, smoothed, hydrology, None,
                );
                if plan.top_block_key != "grass" {
                    return (world_x, world_z, plan);
                }
            }
        }

        panic!("expected deterministic transition mask to select a non-owner top block");
    }

    #[test]
    fn launch_archetypes_resolve_to_defined_material_policies() {
        let launch = [
            RegionArchetype::OceanicShelf,
            RegionArchetype::SandyBeachPlain,
            RegionArchetype::CoastalCliffland,
            RegionArchetype::ColdWetLowland,
            RegionArchetype::TemperatePlain,
            RegionArchetype::TemperateHills,
            RegionArchetype::TemperatePlateau,
            RegionArchetype::SteppePlain,
            RegionArchetype::DesertPlain,
            RegionArchetype::DesertDuneField,
            RegionArchetype::SavannaPlain,
            RegionArchetype::TropicalRainforestLowland,
            RegionArchetype::TropicalRainforestHills,
            RegionArchetype::GlaciatedAlpine,
            RegionArchetype::TundraPlain,
        ];

        for archetype in launch {
            let policy = resolve_material_policy_for_archetype(archetype);
            assert!(
                material_policy_def(policy).is_some(),
                "missing policy for {archetype:?}"
            );
        }
    }

    #[test]
    fn runtime_winter_turns_temperate_grass_cover_snowy() {
        let region = RegionClassCell {
            temperature_band: TemperatureBand::Temperate,
            moisture_band: MoistureBand::Subhumid,
            elevation_band: ElevationBand::Low,
            relief_class: ReliefClass::Plain,
            hydrology_context: HydrologyContext::WellDrained,
            coastal_context: CoastalContext::Inland,
            climate_regime: ClimateRegime::TemperateSeasonal,
            biome_family: crate::world::BiomeFamily::TemperateGrassland,
            terrain_form_family: TerrainFormFamily::Plain,
            archetype: RegionArchetype::TemperatePlain,
        };
        let smoothed = SmoothedColumn {
            height: 8.4,
            remaining_relief_budget: 4.0,
            local_slope: 0.25,
            concavity: 0.0,
            material_support: crate::world::RealizationMaterialSupport {
                wetness: 0.12,
                exposure: 0.18,
                sediment: 0.16,
                soil_cover: 0.70,
            },
        };
        let hydrology = crate::world::HydrologyColumn {
            terrain_height: 8.4,
            water_surface_height: None,
            channel_floor_height: None,
            saturation: 0.0,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Dry,
        };
        let runtime = SurfaceRuntimeContext {
            seasonal_phase: Some(SeasonalPhase::Winter),
        };

        let plan = resolve_surface_column_plan(region, smoothed, hydrology, Some(&runtime));

        assert_eq!(
            plan.seasonal_state,
            Some(SeasonalBiomeStateId::TemperateSnowy)
        );
        assert_eq!(plan.cover_override_key, Some("snowy_grass"));
        assert_eq!(plan.top_block_key, "snow");
    }

    #[test]
    fn hydrology_water_quantizes_above_terrain_top() {
        let region = test_region(RegionArchetype::TemperatePlain);
        let hydrology = crate::world::HydrologyColumn {
            terrain_height: 8.92,
            water_surface_height: Some(8.96),
            channel_floor_height: Some(8.2),
            saturation: 0.85,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Channel,
        };

        let plan = resolve_surface_column_plan(region, test_smoothed_column(), hydrology, None);

        assert_eq!(plan.terrain_top_y, 8);
        assert_eq!(plan.water_top_y, Some(9));
        assert_eq!(plan.water_block_key, Some("water"));
    }

    #[test]
    fn water_quantization_does_not_raise_planned_surface_over_terrain() {
        let region = test_region(RegionArchetype::TemperatePlain);
        let hydrology = crate::world::HydrologyColumn {
            terrain_height: 9.2,
            water_surface_height: Some(8.96),
            channel_floor_height: Some(7.2),
            saturation: 0.85,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Channel,
        };

        let plan = resolve_surface_column_plan(region, test_smoothed_column(), hydrology, None);

        assert_eq!(plan.terrain_top_y, 9);
        assert_eq!(plan.water_top_y, None);
        assert_eq!(plan.water_block_key, None);
    }

    #[test]
    fn weak_floodplain_context_keeps_temperate_surface_grass() {
        let region = test_region(RegionArchetype::TemperatePlain);
        let hydrology = crate::world::HydrologyColumn {
            terrain_height: 8.4,
            water_surface_height: None,
            channel_floor_height: Some(7.2),
            saturation: 0.58,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Floodplain,
        };

        let plan = resolve_surface_column_plan(region, test_smoothed_column(), hydrology, None);

        assert_eq!(plan.cover_phase, CoverPhase::Dormant);
        assert_eq!(plan.top_block_key, "grass");
        assert_eq!(plan.filler_block_key, "dirt");
    }

    #[test]
    fn tropical_lowland_defaults_to_jungle_grass() {
        let region = test_region(RegionArchetype::TropicalRainforestLowland);
        let plan = resolve_surface_column_plan(
            region,
            test_smoothed_column(),
            dry_hydrology_column(),
            None,
        );

        assert_eq!(plan.material_policy, MaterialPolicyId::TropicalLowland);
        assert_eq!(plan.top_block_key, "jungle_grass");
        assert_eq!(plan.filler_block_key, "humus");
    }

    #[test]
    fn mixed_region_influence_can_select_transition_blocks_without_changing_owner() {
        let owner = test_region(RegionArchetype::TemperatePlain);
        let candidate = test_region(RegionArchetype::DesertPlain);
        let (_, _, plan) = find_dry_transition_plan(owner, candidate);

        assert_eq!(plan.owner_archetype, RegionArchetype::TemperatePlain);
        assert_eq!(plan.material_policy, MaterialPolicyId::TemperateGrassland);
        assert!(matches!(
            plan.top_block_key,
            "gravel" | "coarse_dirt" | "sand" | "exposed_rock"
        ));
        assert_ne!(plan.top_block_key, "grass");
    }

    #[test]
    fn hydrology_overrides_mixed_region_transition_materials() {
        let owner = test_region(RegionArchetype::TemperatePlain);
        let candidate = test_region(RegionArchetype::DesertPlain);
        let (world_x, world_z, dry_plan) = find_dry_transition_plan(owner, candidate);
        let hydrology = crate::world::HydrologyColumn {
            terrain_height: 8.4,
            water_surface_height: Some(10.2),
            channel_floor_height: Some(7.2),
            saturation: 0.95,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Lake,
        };

        let wet_plan = resolve_surface_column_plan_with_influence(
            owner,
            &mixed_influence(owner, candidate, 0.48),
            world_x,
            world_z,
            test_smoothed_column(),
            hydrology,
            None,
        );

        assert_ne!(dry_plan.top_block_key, "grass");
        assert_eq!(wet_plan.owner_archetype, RegionArchetype::TemperatePlain);
        assert_eq!(wet_plan.top_block_key, "peat");
        assert_eq!(wet_plan.filler_block_key, "mud");
        assert_eq!(wet_plan.core_block_key, "dirt");
        assert_eq!(wet_plan.water_block_key, Some("water"));
    }
}
