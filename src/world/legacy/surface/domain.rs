use crate::world::atlas::{
    BiomeFamily, CoastalContext, HydrologyContext, RegionArchetype, RegionClassCell,
    RegionClassInfluenceSet, TerrainFormFamily,
};
use crate::world::generation::SEA_LEVEL_Y;
use crate::world::generation::{HydrologyColumn, HydrologyMode, SmoothedColumn};

const DOMAIN_BOUNDARY_HASH_K1: u64 = 0x9E37_79B9_7F4A_7C15;
const DOMAIN_BOUNDARY_HASH_K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const DOMAIN_BOUNDARY_SALT: u64 = 0x51A7_D0A1_0000_0001;
const DOMAIN_BOUNDARY_FINE_SALT: u64 = 0x51A7_D0A1_0000_0002;
const FOREIGN_SUPPORT_MIN: f32 = 0.24;
const HARD_BARRIER_LIMIT: f32 = 0.84;
const STRONG_SUPPORT: f32 = 0.72;
const LOCAL_SUPPORT_MIN: f32 = 0.42;
const LOCAL_SUPPORT_MARGIN: f32 = 0.14;
const LOCAL_SUPPORT_SCORE_MIN: f32 = 0.46;

const LOCAL_OVERRIDE_DOMAINS: [MaterialDomainKind; 8] = [
    MaterialDomainKind::Wetland,
    MaterialDomainKind::TemperateGreen,
    MaterialDomainKind::DryGrassland,
    MaterialDomainKind::SavannaGrassland,
    MaterialDomainKind::TropicalForest,
    MaterialDomainKind::DesertDry,
    MaterialDomainKind::ColdSparse,
    MaterialDomainKind::AlpineRock,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialDomainKind {
    MarineShelf,
    SandyCoast,
    RockyCoast,
    Wetland,
    TemperateGreen,
    DryGrassland,
    SavannaGrassland,
    TropicalForest,
    DesertDry,
    ColdSparse,
    AlpineRock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialDomainTransitionReason {
    HardOwner,
    SameDomain,
    LocalSupportInterior,
    UnsupportedWarpOnly,
    BarrierRejected,
    BoundaryDisplacement,
    TerrainSupportedBoundary,
    HydrologySupportedBoundary,
    CoastalSupportedBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialDomainSupport {
    pub generation: f32,
    pub terrain: f32,
    pub hydrology: f32,
    pub coastal: f32,
    pub compatibility: f32,
    pub barrier: f32,
    pub total: f32,
}

impl Default for MaterialDomainSupport {
    fn default() -> Self {
        Self {
            generation: 0.0,
            terrain: 0.0,
            hydrology: 0.0,
            coastal: 0.0,
            compatibility: 1.0,
            barrier: 0.0,
            total: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MaterialDomainInput<'a> {
    pub hard_owner: RegionClassCell,
    pub influence: &'a RegionClassInfluenceSet,
    pub smoothed: SmoothedColumn,
    pub hydrology: HydrologyColumn,
    pub world_x: i32,
    pub world_z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialDomainSample {
    pub hard_owner: RegionClassCell,
    pub hard_domain: MaterialDomainKind,
    pub visible_owner: RegionClassCell,
    pub visible_domain: MaterialDomainKind,
    pub candidate_owner: Option<RegionClassCell>,
    pub candidate_domain: Option<MaterialDomainKind>,
    pub transition_strength: f32,
    pub transition_phase: f32,
    pub boundary_displacement: f32,
    pub support: MaterialDomainSupport,
    pub reason: MaterialDomainTransitionReason,
    pub accepted_foreign_owner: bool,
}

#[derive(Debug, Clone, Copy)]
struct DomainCandidate {
    owner: RegionClassCell,
    domain: MaterialDomainKind,
    weight: f32,
    owner_weight: f32,
    score: f32,
}

#[derive(Debug, Clone, Copy)]
struct LocalDomainCandidate {
    domain: MaterialDomainKind,
    absolute_support: f32,
    hard_support: f32,
    support: MaterialDomainSupport,
    score: f32,
}

pub fn sample_material_domain(input: MaterialDomainInput<'_>) -> MaterialDomainSample {
    let hard_domain = material_domain_kind_for_region(input.hard_owner);
    let visible_anchor = input.influence.dominant.class;
    let anchor_domain = material_domain_kind_for_region(visible_anchor);
    let anchor_weight = influence_weight_for_domain(input.influence, anchor_domain);
    let same_domain_visible = strongest_same_domain_owner(input.influence, anchor_domain);
    let local_candidate = best_local_supported_domain(input, visible_anchor, anchor_domain);
    if let Some(candidate) = local_candidate {
        return local_supported_sample(input, visible_anchor, hard_domain, candidate);
    }
    let Some(candidate) =
        best_visible_domain_candidate(input, visible_anchor, anchor_domain, anchor_weight)
    else {
        let visible_owner = same_domain_visible.unwrap_or(visible_anchor);
        return MaterialDomainSample {
            hard_owner: input.hard_owner,
            hard_domain,
            visible_owner,
            visible_domain: material_domain_kind_for_region(visible_owner),
            candidate_owner: None,
            candidate_domain: None,
            transition_strength: input.influence.transition_strength.clamp(0.0, 1.0),
            transition_phase: 0.0,
            boundary_displacement: 0.0,
            support: MaterialDomainSupport::default(),
            reason: if visible_owner.archetype == input.hard_owner.archetype {
                MaterialDomainTransitionReason::HardOwner
            } else {
                MaterialDomainTransitionReason::SameDomain
            },
            accepted_foreign_owner: false,
        };
    };

    let support = material_domain_support(
        visible_anchor,
        anchor_domain,
        candidate.owner,
        candidate.domain,
        input.smoothed,
        input.hydrology,
        input.influence.barrier_strength,
    );
    let boundary_displacement = material_domain_boundary_displacement(
        input.world_x,
        input.world_z,
        anchor_domain,
        candidate.domain,
    );
    let transition_phase =
        material_domain_transition_phase(candidate, support, boundary_displacement);
    let transition_strength =
        (candidate.weight * (0.55 + support.total * 0.45) * (1.0 - support.barrier * 0.42))
            .clamp(0.0, 1.0);

    if support.total < FOREIGN_SUPPORT_MIN {
        return rejected_sample(
            input,
            visible_anchor,
            hard_domain,
            anchor_domain,
            candidate,
            transition_strength,
            transition_phase,
            boundary_displacement,
            support,
            MaterialDomainTransitionReason::UnsupportedWarpOnly,
        );
    }

    if support.barrier >= HARD_BARRIER_LIMIT && support.total < STRONG_SUPPORT {
        return rejected_sample(
            input,
            visible_anchor,
            hard_domain,
            anchor_domain,
            candidate,
            transition_strength,
            transition_phase,
            boundary_displacement,
            support,
            MaterialDomainTransitionReason::BarrierRejected,
        );
    }

    if transition_phase < 0.0 {
        return rejected_sample(
            input,
            visible_anchor,
            hard_domain,
            anchor_domain,
            candidate,
            transition_strength,
            transition_phase,
            boundary_displacement,
            support,
            MaterialDomainTransitionReason::HardOwner,
        );
    }

    MaterialDomainSample {
        hard_owner: input.hard_owner,
        hard_domain,
        visible_owner: candidate.owner,
        visible_domain: candidate.domain,
        candidate_owner: Some(candidate.owner),
        candidate_domain: Some(candidate.domain),
        transition_strength,
        transition_phase,
        boundary_displacement,
        support,
        reason: accepted_reason(support),
        accepted_foreign_owner: true,
    }
}

pub fn material_domain_kind_for_region(region: RegionClassCell) -> MaterialDomainKind {
    if matches!(region.archetype, RegionArchetype::OceanicShelf)
        || matches!(region.biome_family, BiomeFamily::Oceanic)
        || matches!(region.terrain_form_family, TerrainFormFamily::MarineShelf)
        || matches!(region.coastal_context, CoastalContext::Marine)
    {
        return MaterialDomainKind::MarineShelf;
    }

    if matches!(
        region.archetype,
        RegionArchetype::CoastalCliffland
            | RegionArchetype::RockyShoreCoast
            | RegionArchetype::FjordCoast
    ) || matches!(region.biome_family, BiomeFamily::RockyCoast)
        || matches!(
            region.terrain_form_family,
            TerrainFormFamily::RockyShore
                | TerrainFormFamily::SeaCliff
                | TerrainFormFamily::FjordCoast
        )
    {
        return MaterialDomainKind::RockyCoast;
    }

    if matches!(
        region.archetype,
        RegionArchetype::SandyBeachPlain
            | RegionArchetype::BarrierCoast
            | RegionArchetype::LagoonCoast
    ) || matches!(
        region.biome_family,
        BiomeFamily::SandyCoast | BiomeFamily::LagoonCoast
    ) || matches!(
        region.terrain_form_family,
        TerrainFormFamily::BeachPlain
            | TerrainFormFamily::BarrierCoast
            | TerrainFormFamily::LagoonCoast
    ) {
        return MaterialDomainKind::SandyCoast;
    }

    if matches!(
        region.hydrology_context,
        HydrologyContext::RiverCorridor
            | HydrologyContext::LakeBasin
            | HydrologyContext::WetLowland
    ) || matches!(
        region.biome_family,
        BiomeFamily::Marsh
            | BiomeFamily::Swamp
            | BiomeFamily::FloodedForest
            | BiomeFamily::EstuarineCoast
            | BiomeFamily::Mangrove
    ) || matches!(
        region.terrain_form_family,
        TerrainFormFamily::Floodplain
            | TerrainFormFamily::WetLowland
            | TerrainFormFamily::AlluvialLowland
            | TerrainFormFamily::EstuaryLowland
            | TerrainFormFamily::Delta
            | TerrainFormFamily::Basin
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
        return MaterialDomainKind::Wetland;
    }

    if matches!(
        region.archetype,
        RegionArchetype::GlaciatedAlpine
            | RegionArchetype::SubalpineWoodedFront
            | RegionArchetype::AlpineMeadowMountain
            | RegionArchetype::GlacialValley
            | RegionArchetype::CrevassedIcefield
            | RegionArchetype::BorealRidgeCountry
            | RegionArchetype::AlpineRavineCountry
    ) || matches!(
        region.biome_family,
        BiomeFamily::SubalpineWoodland | BiomeFamily::AlpineMeadow | BiomeFamily::PolarIce
    ) || matches!(
        region.terrain_form_family,
        TerrainFormFamily::Mountain
            | TerrainFormFamily::MountainFront
            | TerrainFormFamily::RidgeCountry
            | TerrainFormFamily::GlacialValley
            | TerrainFormFamily::Icefield
            | TerrainFormFamily::CrevassedIcefield
            | TerrainFormFamily::RavineCountry
    ) {
        return MaterialDomainKind::AlpineRock;
    }

    if matches!(
        region.archetype,
        RegionArchetype::TundraPlain | RegionArchetype::PolarBarrensPlain
    ) || matches!(
        region.biome_family,
        BiomeFamily::Tundra | BiomeFamily::PolarBarrens | BiomeFamily::BorealForest
    ) {
        return MaterialDomainKind::ColdSparse;
    }

    if matches!(
        region.archetype,
        RegionArchetype::DesertPlain
            | RegionArchetype::DesertDuneField
            | RegionArchetype::DesertBasin
            | RegionArchetype::DesertMesaCountry
            | RegionArchetype::DesertAlluvialFan
            | RegionArchetype::SemiDesertPediment
            | RegionArchetype::DryShrublandBadlands
            | RegionArchetype::DryShrublandKarst
            | RegionArchetype::MediterraneanShrublandHills
    ) || matches!(
        region.biome_family,
        BiomeFamily::Desert
            | BiomeFamily::SemiDesert
            | BiomeFamily::DryShrubland
            | BiomeFamily::MediterraneanShrubland
    ) || matches!(
        region.terrain_form_family,
        TerrainFormFamily::DuneField
            | TerrainFormFamily::MesaCountry
            | TerrainFormFamily::Badlands
            | TerrainFormFamily::Karst
            | TerrainFormFamily::Pediment
    ) {
        return MaterialDomainKind::DesertDry;
    }

    if matches!(
        region.archetype,
        RegionArchetype::SavannaPlain
            | RegionArchetype::SavannaHills
            | RegionArchetype::TropicalDryForestHills
    ) || matches!(
        region.biome_family,
        BiomeFamily::Savanna | BiomeFamily::TropicalDryForest
    ) {
        return MaterialDomainKind::SavannaGrassland;
    }

    if matches!(
        region.archetype,
        RegionArchetype::TropicalRainforestLowland | RegionArchetype::TropicalRainforestHills
    ) || matches!(
        region.biome_family,
        BiomeFamily::TropicalRainforest | BiomeFamily::MonsoonForest
    ) {
        return MaterialDomainKind::TropicalForest;
    }

    if matches!(
        region.archetype,
        RegionArchetype::SteppePlain | RegionArchetype::SteppeHills
    ) || matches!(region.biome_family, BiomeFamily::Steppe)
    {
        return MaterialDomainKind::DryGrassland;
    }

    MaterialDomainKind::TemperateGreen
}

fn local_supported_sample(
    input: MaterialDomainInput<'_>,
    visible_anchor: RegionClassCell,
    hard_domain: MaterialDomainKind,
    candidate: LocalDomainCandidate,
) -> MaterialDomainSample {
    let support_delta = candidate.absolute_support - candidate.hard_support;
    let transition_strength = (smoothstep_range(LOCAL_SUPPORT_MARGIN, 0.42, support_delta)
        * (0.42 + candidate.support.total * 0.58))
        .clamp(0.0, 1.0);
    let visible_owner = if influence_weight_for_domain(input.influence, candidate.domain) > 0.0 {
        strongest_owner_for_domain(input.influence, candidate.domain)
    } else {
        visible_anchor
    };

    MaterialDomainSample {
        hard_owner: input.hard_owner,
        hard_domain,
        visible_owner,
        visible_domain: candidate.domain,
        candidate_owner: None,
        candidate_domain: Some(candidate.domain),
        transition_strength,
        transition_phase: 0.0,
        boundary_displacement: 0.0,
        support: candidate.support,
        reason: MaterialDomainTransitionReason::LocalSupportInterior,
        accepted_foreign_owner: visible_owner.archetype != visible_anchor.archetype,
    }
}

fn rejected_sample(
    input: MaterialDomainInput<'_>,
    visible_anchor: RegionClassCell,
    hard_domain: MaterialDomainKind,
    anchor_domain: MaterialDomainKind,
    candidate: DomainCandidate,
    transition_strength: f32,
    transition_phase: f32,
    boundary_displacement: f32,
    support: MaterialDomainSupport,
    reason: MaterialDomainTransitionReason,
) -> MaterialDomainSample {
    MaterialDomainSample {
        hard_owner: input.hard_owner,
        hard_domain,
        visible_owner: visible_anchor,
        visible_domain: anchor_domain,
        candidate_owner: Some(candidate.owner),
        candidate_domain: Some(candidate.domain),
        transition_strength,
        transition_phase,
        boundary_displacement,
        support,
        reason,
        accepted_foreign_owner: false,
    }
}

fn strongest_same_domain_owner(
    influence: &RegionClassInfluenceSet,
    domain: MaterialDomainKind,
) -> Option<RegionClassCell> {
    let mut best = None;
    let mut best_weight = 0.0_f32;

    for sample in std::iter::once(&influence.dominant).chain(influence.neighbors.iter()) {
        if material_domain_kind_for_region(sample.class) != domain {
            continue;
        }

        if sample.weight > best_weight {
            best = Some(sample.class);
            best_weight = sample.weight;
        }
    }

    best
}

fn best_local_supported_domain(
    input: MaterialDomainInput<'_>,
    visible_anchor: RegionClassCell,
    anchor_domain: MaterialDomainKind,
) -> Option<LocalDomainCandidate> {
    let hard_support = material_support_for_domain(anchor_domain, input.smoothed, input.hydrology);
    let mut best = None;
    let mut best_score = f32::NEG_INFINITY;

    for domain in LOCAL_OVERRIDE_DOMAINS {
        if domain == anchor_domain {
            continue;
        }

        let absolute_support = material_support_for_domain(domain, input.smoothed, input.hydrology);
        let support_delta = absolute_support - hard_support;
        if absolute_support < LOCAL_SUPPORT_MIN || support_delta < LOCAL_SUPPORT_MARGIN {
            continue;
        }

        let support = material_domain_support(
            visible_anchor,
            anchor_domain,
            visible_anchor,
            domain,
            input.smoothed,
            input.hydrology,
            0.0,
        );
        if support.total < LOCAL_SUPPORT_MIN {
            continue;
        }

        let score = absolute_support * 0.42 + support.total * 0.44 + support.compatibility * 0.14;
        if score > best_score + 0.0001 {
            best = Some(LocalDomainCandidate {
                domain,
                absolute_support,
                hard_support,
                support,
                score,
            });
            best_score = score;
        }
    }

    best.filter(|candidate| candidate.score >= LOCAL_SUPPORT_SCORE_MIN)
}

fn best_visible_domain_candidate(
    input: MaterialDomainInput<'_>,
    visible_anchor: RegionClassCell,
    anchor_domain: MaterialDomainKind,
    owner_weight: f32,
) -> Option<DomainCandidate> {
    let mut best = None;
    let mut best_score = f32::NEG_INFINITY;

    for sample in std::iter::once(&input.influence.dominant).chain(input.influence.neighbors.iter())
    {
        let domain = material_domain_kind_for_region(sample.class);
        if domain == anchor_domain {
            continue;
        }

        let weight = influence_weight_for_domain(input.influence, domain);
        let owner = strongest_owner_for_domain(input.influence, domain);
        let score = domain_candidate_score(
            domain,
            weight,
            owner_weight,
            visible_anchor,
            owner,
            input,
            anchor_domain,
        );

        if score > best_score + 0.0001
            || ((score - best_score).abs() <= 0.0001
                && weight
                    > best
                        .map(|candidate: DomainCandidate| candidate.weight)
                        .unwrap_or(0.0))
        {
            best = Some(DomainCandidate {
                owner,
                domain,
                weight,
                owner_weight,
                score,
            });
            best_score = score;
        }
    }

    best
}

fn domain_candidate_score(
    domain: MaterialDomainKind,
    domain_weight: f32,
    owner_weight: f32,
    hard_owner: RegionClassCell,
    candidate_owner: RegionClassCell,
    input: MaterialDomainInput<'_>,
    hard_domain: MaterialDomainKind,
) -> f32 {
    let absolute_support = material_support_for_domain(domain, input.smoothed, input.hydrology);

    let support = material_domain_support(
        hard_owner,
        hard_domain,
        candidate_owner,
        domain,
        input.smoothed,
        input.hydrology,
        input.influence.barrier_strength,
    );
    let boundary_displacement =
        material_domain_boundary_displacement(input.world_x, input.world_z, hard_domain, domain);

    (domain_weight * 0.28
        + absolute_support * 0.16
        + support.total * 0.46
        + support.compatibility * 0.10
        + boundary_displacement.max(0.0) * 0.04
        - owner_weight * 0.10)
        .clamp(0.0, 1.4)
}

fn strongest_owner_for_domain(
    influence: &RegionClassInfluenceSet,
    domain: MaterialDomainKind,
) -> RegionClassCell {
    let mut strongest = influence.dominant.class;
    let mut strongest_weight = 0.0_f32;

    for sample in std::iter::once(&influence.dominant).chain(influence.neighbors.iter()) {
        if material_domain_kind_for_region(sample.class) == domain
            && sample.weight > strongest_weight
        {
            strongest = sample.class;
            strongest_weight = sample.weight;
        }
    }

    strongest
}

fn influence_weight_for_domain(
    influence: &RegionClassInfluenceSet,
    domain: MaterialDomainKind,
) -> f32 {
    std::iter::once(&influence.dominant)
        .chain(influence.neighbors.iter())
        .filter(|sample| material_domain_kind_for_region(sample.class) == domain)
        .map(|sample| sample.weight.max(0.0))
        .sum::<f32>()
        .clamp(0.0, 1.0)
}

fn material_domain_support(
    hard_owner: RegionClassCell,
    hard_domain: MaterialDomainKind,
    candidate_owner: RegionClassCell,
    candidate_domain: MaterialDomainKind,
    smoothed: SmoothedColumn,
    hydrology: HydrologyColumn,
    barrier: f32,
) -> MaterialDomainSupport {
    let terrain = terrain_support(hard_domain, candidate_domain, smoothed).max(
        upper_terrace_support(hard_domain, candidate_domain, smoothed, hydrology),
    );
    let generation = generation_support(hard_domain, candidate_domain, smoothed, hydrology);
    let hydrology_support = if hydrology_supports_domain_pair(hard_domain, candidate_domain) {
        hydrology_support(hydrology)
    } else {
        0.0
    };
    let coastal = coastal_support(
        hard_owner,
        hard_domain,
        candidate_owner,
        candidate_domain,
        smoothed,
        hydrology,
    );
    let compatibility =
        domain_compatibility(hard_domain, candidate_domain, hard_owner, candidate_owner);
    let barrier = barrier.clamp(0.0, 1.0);
    let raw_support = generation.max(terrain).max(hydrology_support).max(coastal);
    let total =
        (raw_support * (0.58 + compatibility * 0.42) * (1.0 - barrier * 0.46)).clamp(0.0, 1.0);

    MaterialDomainSupport {
        generation,
        terrain,
        hydrology: hydrology_support,
        coastal,
        compatibility,
        barrier,
        total,
    }
}

fn generation_support(
    hard_domain: MaterialDomainKind,
    candidate_domain: MaterialDomainKind,
    smoothed: SmoothedColumn,
    hydrology: HydrologyColumn,
) -> f32 {
    let hard_support = material_support_for_domain(hard_domain, smoothed, hydrology);
    let candidate_support = material_support_for_domain(candidate_domain, smoothed, hydrology);
    let relative = smoothstep_range(0.08, 0.34, candidate_support - hard_support);
    let absolute = smoothstep_range(0.32, 0.82, candidate_support);

    if relative <= f32::EPSILON {
        0.0
    } else {
        (relative * 0.76 + absolute * 0.24).clamp(0.0, 1.0)
    }
}

fn material_support_for_domain(
    domain: MaterialDomainKind,
    smoothed: SmoothedColumn,
    hydrology: HydrologyColumn,
) -> f32 {
    let support = smoothed.material_support;
    let hydro = hydrology_support(hydrology);
    let coastal = coastal_terrain_signal(smoothed, hydrology);

    match domain {
        MaterialDomainKind::MarineShelf => coastal.max(hydro * 0.88).max(support.wetness * 0.58),
        MaterialDomainKind::SandyCoast => {
            (support.sediment * 0.62 + coastal * 0.34 + hydro * 0.14).clamp(0.0, 1.0)
        }
        MaterialDomainKind::RockyCoast => {
            (support.exposure * 0.68 + coastal * 0.30).clamp(0.0, 1.0)
        }
        MaterialDomainKind::Wetland => support.wetness.max(hydro),
        MaterialDomainKind::TemperateGreen
        | MaterialDomainKind::SavannaGrassland
        | MaterialDomainKind::TropicalForest => {
            (support.soil_cover * 0.84 + support.wetness * 0.10).clamp(0.0, 1.0)
        }
        MaterialDomainKind::DryGrassland | MaterialDomainKind::DesertDry => {
            (support.sediment * 0.40 + support.soil_cover * 0.32 + support.exposure * 0.22
                - support.wetness * 0.18)
                .clamp(0.0, 1.0)
        }
        MaterialDomainKind::ColdSparse => {
            (support.soil_cover * 0.46 + support.exposure * 0.30 + support.wetness * 0.14)
                .clamp(0.0, 1.0)
        }
        MaterialDomainKind::AlpineRock => support.exposure,
    }
}

fn coastal_terrain_signal(smoothed: SmoothedColumn, hydrology: HydrologyColumn) -> f32 {
    let sea_level = smoothstep_range(
        6.0,
        0.0,
        (hydrology.terrain_height - SEA_LEVEL_Y as f32).abs(),
    );
    let water = if hydrology.water_surface_height.is_some() {
        0.86
    } else {
        0.0
    };
    let gravel = smoothstep_range(0.12, 0.34, hydrology.gravel_bar_strength) * 0.82;

    let _ = smoothed;

    sea_level.max(water).max(gravel).clamp(0.0, 1.0)
}

fn terrain_support(
    hard_domain: MaterialDomainKind,
    candidate_domain: MaterialDomainKind,
    smoothed: SmoothedColumn,
) -> f32 {
    let support = smoothed.material_support;
    let exposed_domain = is_exposed_domain(hard_domain) || is_exposed_domain(candidate_domain);
    let dry_or_rock_boundary = matches!(
        hard_domain,
        MaterialDomainKind::DesertDry
            | MaterialDomainKind::DryGrassland
            | MaterialDomainKind::AlpineRock
            | MaterialDomainKind::RockyCoast
    ) || matches!(
        candidate_domain,
        MaterialDomainKind::DesertDry
            | MaterialDomainKind::DryGrassland
            | MaterialDomainKind::AlpineRock
            | MaterialDomainKind::RockyCoast
    );
    let wet_boundary = matches!(hard_domain, MaterialDomainKind::Wetland)
        || matches!(candidate_domain, MaterialDomainKind::Wetland);
    let green_boundary = matches!(
        hard_domain,
        MaterialDomainKind::TemperateGreen
            | MaterialDomainKind::SavannaGrassland
            | MaterialDomainKind::TropicalForest
    ) || matches!(
        candidate_domain,
        MaterialDomainKind::TemperateGreen
            | MaterialDomainKind::SavannaGrassland
            | MaterialDomainKind::TropicalForest
    );

    let exposure = if exposed_domain {
        support.exposure * 0.58
    } else if dry_or_rock_boundary {
        support.exposure * 0.34 + support.sediment * 0.18
    } else {
        0.0
    };
    let wet = if wet_boundary {
        support.wetness * 0.48
    } else {
        0.0
    };
    let green = if green_boundary {
        support.soil_cover * 0.32
    } else {
        0.0
    };

    exposure.max(wet).max(green).clamp(0.0, 1.0)
}

fn upper_terrace_support(
    hard_domain: MaterialDomainKind,
    candidate_domain: MaterialDomainKind,
    smoothed: SmoothedColumn,
    hydrology: HydrologyColumn,
) -> f32 {
    let rocky_to_green = matches!(
        (hard_domain, candidate_domain),
        (
            MaterialDomainKind::RockyCoast,
            MaterialDomainKind::TemperateGreen
        ) | (
            MaterialDomainKind::TemperateGreen,
            MaterialDomainKind::RockyCoast
        ) | (
            MaterialDomainKind::RockyCoast,
            MaterialDomainKind::DryGrassland
        ) | (
            MaterialDomainKind::DryGrassland,
            MaterialDomainKind::RockyCoast
        )
    );
    if !rocky_to_green {
        return 0.0;
    }

    let above_sea = hydrology.terrain_height - SEA_LEVEL_Y as f32;
    let upland = smoothstep_range(1.4, 5.0, above_sea);
    let gentle = 1.0 - smoothstep_range(0.26, 0.58, smoothed.local_slope);
    let stable_surface = 1.0 - smoothstep_range(0.10, 0.34, smoothed.concavity.max(0.0));

    (upland * gentle * stable_surface).clamp(0.0, 1.0)
}

fn hydrology_support(hydrology: HydrologyColumn) -> f32 {
    let mode: f32 = match hydrology.mode {
        HydrologyMode::Dry => 0.0,
        HydrologyMode::Floodplain => 0.54,
        HydrologyMode::Wetland => 0.72,
        HydrologyMode::Channel => 0.86,
        HydrologyMode::Lake => 0.82,
    };
    let visible_water = if hydrology.water_surface_height.is_some() {
        0.92
    } else {
        0.0
    };
    let channel_floor = if hydrology.channel_floor_height.is_some() {
        0.54
    } else {
        0.0
    };
    let saturation = smoothstep_range(0.42, 0.82, hydrology.saturation);
    let gravel = smoothstep_range(0.14, 0.38, hydrology.gravel_bar_strength);

    mode.max(visible_water)
        .max(channel_floor)
        .max(saturation)
        .max(gravel)
        .clamp(0.0, 1.0)
}

fn hydrology_supports_domain_pair(
    hard_domain: MaterialDomainKind,
    candidate_domain: MaterialDomainKind,
) -> bool {
    matches!(
        hard_domain,
        MaterialDomainKind::MarineShelf
            | MaterialDomainKind::SandyCoast
            | MaterialDomainKind::RockyCoast
            | MaterialDomainKind::Wetland
            | MaterialDomainKind::TropicalForest
            | MaterialDomainKind::ColdSparse
    ) || matches!(
        candidate_domain,
        MaterialDomainKind::MarineShelf
            | MaterialDomainKind::SandyCoast
            | MaterialDomainKind::RockyCoast
            | MaterialDomainKind::Wetland
            | MaterialDomainKind::TropicalForest
            | MaterialDomainKind::ColdSparse
    )
}

fn coastal_support(
    hard_owner: RegionClassCell,
    hard_domain: MaterialDomainKind,
    candidate_owner: RegionClassCell,
    candidate_domain: MaterialDomainKind,
    smoothed: SmoothedColumn,
    hydrology: HydrologyColumn,
) -> f32 {
    if !is_coastal_domain(hard_domain)
        && !is_coastal_domain(candidate_domain)
        && !matches!(
            hard_owner.coastal_context,
            CoastalContext::Marine | CoastalContext::Coastal
        )
        && !matches!(
            candidate_owner.coastal_context,
            CoastalContext::Marine | CoastalContext::Coastal
        )
    {
        return 0.0;
    }

    let sea_level = smoothstep_range(
        6.0,
        0.0,
        (hydrology.terrain_height - SEA_LEVEL_Y as f32).abs(),
    );
    let water = if hydrology.water_surface_height.is_some() {
        0.86
    } else {
        0.0
    };
    let gravel = smoothstep_range(0.12, 0.34, hydrology.gravel_bar_strength) * 0.82;

    let _ = smoothed;

    sea_level.max(water).max(gravel).clamp(0.0, 1.0)
}

fn domain_compatibility(
    hard_domain: MaterialDomainKind,
    candidate_domain: MaterialDomainKind,
    hard_owner: RegionClassCell,
    candidate_owner: RegionClassCell,
) -> f32 {
    if hard_domain == candidate_domain {
        return 1.0;
    }

    let mut compatibility: f32 = match (hard_domain, candidate_domain) {
        (MaterialDomainKind::MarineShelf, MaterialDomainKind::SandyCoast)
        | (MaterialDomainKind::SandyCoast, MaterialDomainKind::MarineShelf) => 0.62,
        (MaterialDomainKind::MarineShelf, MaterialDomainKind::RockyCoast)
        | (MaterialDomainKind::RockyCoast, MaterialDomainKind::MarineShelf) => 0.48,
        (MaterialDomainKind::SandyCoast, MaterialDomainKind::RockyCoast)
        | (MaterialDomainKind::RockyCoast, MaterialDomainKind::SandyCoast) => 0.58,
        (MaterialDomainKind::Wetland, MaterialDomainKind::TemperateGreen)
        | (MaterialDomainKind::TemperateGreen, MaterialDomainKind::Wetland) => 0.70,
        (MaterialDomainKind::Wetland, MaterialDomainKind::ColdSparse)
        | (MaterialDomainKind::ColdSparse, MaterialDomainKind::Wetland) => 0.60,
        (MaterialDomainKind::TemperateGreen, MaterialDomainKind::DryGrassland)
        | (MaterialDomainKind::DryGrassland, MaterialDomainKind::TemperateGreen) => 0.86,
        (MaterialDomainKind::DryGrassland, MaterialDomainKind::SavannaGrassland)
        | (MaterialDomainKind::SavannaGrassland, MaterialDomainKind::DryGrassland) => 0.84,
        (MaterialDomainKind::TemperateGreen, MaterialDomainKind::SavannaGrassland)
        | (MaterialDomainKind::SavannaGrassland, MaterialDomainKind::TemperateGreen) => 0.76,
        (MaterialDomainKind::DryGrassland, MaterialDomainKind::DesertDry)
        | (MaterialDomainKind::DesertDry, MaterialDomainKind::DryGrassland) => 0.78,
        (MaterialDomainKind::SavannaGrassland, MaterialDomainKind::DesertDry)
        | (MaterialDomainKind::DesertDry, MaterialDomainKind::SavannaGrassland) => 0.70,
        (MaterialDomainKind::TropicalForest, MaterialDomainKind::SavannaGrassland)
        | (MaterialDomainKind::SavannaGrassland, MaterialDomainKind::TropicalForest) => 0.62,
        (MaterialDomainKind::TropicalForest, MaterialDomainKind::Wetland)
        | (MaterialDomainKind::Wetland, MaterialDomainKind::TropicalForest) => 0.66,
        (MaterialDomainKind::AlpineRock, MaterialDomainKind::ColdSparse)
        | (MaterialDomainKind::ColdSparse, MaterialDomainKind::AlpineRock) => 0.68,
        (MaterialDomainKind::AlpineRock, MaterialDomainKind::TemperateGreen)
        | (MaterialDomainKind::TemperateGreen, MaterialDomainKind::AlpineRock) => 0.52,
        (MaterialDomainKind::RockyCoast, MaterialDomainKind::AlpineRock)
        | (MaterialDomainKind::AlpineRock, MaterialDomainKind::RockyCoast) => 0.54,
        _ => 0.42,
    };

    if hard_owner.biome_family == candidate_owner.biome_family {
        compatibility += 0.10;
    }
    if hard_owner.terrain_form_family == candidate_owner.terrain_form_family {
        compatibility += 0.08;
    }
    if is_coastal_domain(hard_domain) != is_coastal_domain(candidate_domain) {
        compatibility *= 0.86;
    }
    if matches!(
        (
            hard_owner.hydrology_context,
            candidate_owner.hydrology_context
        ),
        (HydrologyContext::Dryland, HydrologyContext::WetLowland)
            | (HydrologyContext::WetLowland, HydrologyContext::Dryland)
            | (HydrologyContext::Dryland, HydrologyContext::LakeBasin)
            | (HydrologyContext::LakeBasin, HydrologyContext::Dryland)
    ) {
        compatibility *= 0.78;
    }

    compatibility.clamp(0.0, 1.0)
}

fn material_domain_transition_phase(
    candidate: DomainCandidate,
    support: MaterialDomainSupport,
    boundary_displacement: f32,
) -> f32 {
    let weight_edge = candidate.score - candidate.owner_weight * 0.62;
    let supported_push = support.total * 0.34;
    let compatibility_push = support.compatibility * 0.10;
    let barrier_pull = support.barrier * 0.24;
    let ripple_push = boundary_displacement * (0.12 + support.total * 0.07);

    (weight_edge + ripple_push + supported_push + compatibility_push - barrier_pull - 0.18)
        .clamp(-1.0, 1.0)
}

fn accepted_reason(support: MaterialDomainSupport) -> MaterialDomainTransitionReason {
    if support.hydrology >= support.terrain
        && support.hydrology >= support.coastal
        && support.hydrology >= support.generation
        && support.hydrology >= 0.40
    {
        MaterialDomainTransitionReason::HydrologySupportedBoundary
    } else if support.coastal >= support.terrain
        && support.coastal >= support.generation
        && support.coastal >= 0.40
    {
        MaterialDomainTransitionReason::CoastalSupportedBoundary
    } else if support.terrain.max(support.generation) >= 0.40 {
        MaterialDomainTransitionReason::TerrainSupportedBoundary
    } else {
        MaterialDomainTransitionReason::BoundaryDisplacement
    }
}

fn is_exposed_domain(domain: MaterialDomainKind) -> bool {
    matches!(
        domain,
        MaterialDomainKind::RockyCoast
            | MaterialDomainKind::DesertDry
            | MaterialDomainKind::ColdSparse
            | MaterialDomainKind::AlpineRock
    )
}

fn is_coastal_domain(domain: MaterialDomainKind) -> bool {
    matches!(
        domain,
        MaterialDomainKind::MarineShelf
            | MaterialDomainKind::SandyCoast
            | MaterialDomainKind::RockyCoast
    )
}

fn material_domain_boundary_displacement(
    world_x: i32,
    world_z: i32,
    hard_domain: MaterialDomainKind,
    candidate_domain: MaterialDomainKind,
) -> f32 {
    let salt = DOMAIN_BOUNDARY_SALT
        ^ ((hard_domain as u64).wrapping_mul(0xA24B_AED4_963E_E407))
        ^ ((candidate_domain as u64).wrapping_mul(0x9FB2_1C65_1E98_DF25));
    let meso = signed_value_noise_2d(
        world_x as f32 - 61.0,
        world_z as f32 + 29.0,
        181.0,
        salt.rotate_left(17),
    );
    let broad = signed_value_noise_2d(
        world_x as f32 + 107.0,
        world_z as f32 - 73.0,
        421.0,
        salt.rotate_left(31),
    );
    let fine = signed_value_noise_2d(
        world_x as f32 + 3.0,
        world_z as f32 - 5.0,
        9.0,
        (salt ^ DOMAIN_BOUNDARY_FINE_SALT).rotate_left(11),
    );
    let subchunk = signed_value_noise_2d(
        world_x as f32 - 17.0,
        world_z as f32 + 23.0,
        23.0,
        (salt ^ DOMAIN_BOUNDARY_FINE_SALT).rotate_left(43),
    );

    ((meso * 0.34 + broad * 0.22 + subchunk * 0.28 + fine * 0.16) * 0.55).clamp(-1.0, 1.0)
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
    hash ^= (x as i64 as u64).wrapping_mul(DOMAIN_BOUNDARY_HASH_K1);
    hash = hash.rotate_left(27);
    hash ^= (z as i64 as u64).wrapping_mul(DOMAIN_BOUNDARY_HASH_K2);
    hash = mix_u64(hash);

    ((hash >> 40) as u32 as f32) / ((1_u32 << 24) as f32)
}

fn mix_u64(mut value: u64) -> u64 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::atlas::{
        AtlasCoord, BiomeFamily, ClimateRegime, ElevationBand, MoistureBand, RegionClassInfluence,
        ReliefClass, TemperatureBand,
    };

    fn temperate_region(archetype: RegionArchetype) -> RegionClassCell {
        RegionClassCell {
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
        }
    }

    fn desert_region() -> RegionClassCell {
        RegionClassCell {
            temperature_band: TemperatureBand::Hot,
            moisture_band: MoistureBand::Arid,
            hydrology_context: HydrologyContext::Dryland,
            climate_regime: ClimateRegime::AridHot,
            biome_family: BiomeFamily::Desert,
            terrain_form_family: TerrainFormFamily::Plain,
            archetype: RegionArchetype::DesertPlain,
            ..temperate_region(RegionArchetype::DesertPlain)
        }
    }

    fn wetland_region() -> RegionClassCell {
        RegionClassCell {
            moisture_band: MoistureBand::Wet,
            hydrology_context: HydrologyContext::WetLowland,
            climate_regime: ClimateRegime::TemperateSeasonal,
            biome_family: BiomeFamily::Marsh,
            terrain_form_family: TerrainFormFamily::WetLowland,
            archetype: RegionArchetype::ColdWetLowland,
            ..temperate_region(RegionArchetype::ColdWetLowland)
        }
    }

    fn alpine_region() -> RegionClassCell {
        RegionClassCell {
            elevation_band: ElevationBand::Highland,
            relief_class: ReliefClass::Mountain,
            climate_regime: ClimateRegime::ColdAlpine,
            biome_family: BiomeFamily::AlpineMeadow,
            terrain_form_family: TerrainFormFamily::Mountain,
            archetype: RegionArchetype::AlpineMeadowMountain,
            ..temperate_region(RegionArchetype::AlpineMeadowMountain)
        }
    }

    fn dry_smoothed() -> SmoothedColumn {
        SmoothedColumn {
            height: 42.0,
            remaining_relief_budget: 6.0,
            local_slope: 0.18,
            concavity: 0.02,
            material_support: crate::world::RealizationMaterialSupport {
                wetness: 0.05,
                exposure: 0.10,
                sediment: 0.12,
                soil_cover: 0.58,
            },
        }
    }

    fn supported_smoothed() -> SmoothedColumn {
        SmoothedColumn {
            height: 42.0,
            remaining_relief_budget: 6.0,
            local_slope: 0.86,
            concavity: 0.34,
            material_support: crate::world::RealizationMaterialSupport {
                wetness: 0.38,
                exposure: 0.70,
                sediment: 0.42,
                soil_cover: 0.30,
            },
        }
    }

    fn dry_hydrology() -> HydrologyColumn {
        HydrologyColumn {
            terrain_height: 42.0,
            water_surface_height: None,
            channel_floor_height: None,
            saturation: 0.05,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Dry,
        }
    }

    fn green_supported_smoothed() -> SmoothedColumn {
        SmoothedColumn {
            height: 43.0,
            remaining_relief_budget: 5.0,
            local_slope: 0.12,
            concavity: 0.04,
            material_support: crate::world::RealizationMaterialSupport {
                wetness: 0.22,
                exposure: 0.06,
                sediment: 0.10,
                soil_cover: 0.86,
            },
        }
    }

    fn supported_hydrology() -> HydrologyColumn {
        HydrologyColumn {
            terrain_height: 40.8,
            water_surface_height: None,
            channel_floor_height: Some(39.4),
            saturation: 0.76,
            gravel_bar_strength: 0.28,
            mode: HydrologyMode::Channel,
        }
    }

    fn wet_smoothed() -> SmoothedColumn {
        SmoothedColumn {
            height: 40.0,
            remaining_relief_budget: 5.0,
            local_slope: 0.16,
            concavity: 0.18,
            material_support: crate::world::RealizationMaterialSupport {
                wetness: 0.84,
                exposure: 0.08,
                sediment: 0.26,
                soil_cover: 0.62,
            },
        }
    }

    fn wet_hydrology() -> HydrologyColumn {
        HydrologyColumn {
            terrain_height: 39.4,
            water_surface_height: Some(40.2),
            channel_floor_height: Some(38.8),
            saturation: 0.88,
            gravel_bar_strength: 0.10,
            mode: HydrologyMode::Wetland,
        }
    }

    fn mixed_influence(
        owner: RegionClassCell,
        candidate: RegionClassCell,
        candidate_weight: f32,
    ) -> RegionClassInfluenceSet {
        let candidate_weight = candidate_weight.clamp(0.0, 1.0);

        RegionClassInfluenceSet {
            dominant: RegionClassInfluence {
                coord: AtlasCoord::new(1, 0),
                class: candidate,
                weight: candidate_weight,
            },
            neighbors: vec![RegionClassInfluence {
                coord: AtlasCoord::new(0, 0),
                class: owner,
                weight: 1.0 - candidate_weight,
            }],
            transition_strength: candidate_weight,
            barrier_strength: 0.18,
        }
    }

    fn single_owner_influence(owner: RegionClassCell) -> RegionClassInfluenceSet {
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

    #[test]
    fn visible_anchor_does_not_restore_rectangular_hard_owner() {
        let owner = temperate_region(RegionArchetype::TemperatePlain);
        let candidate = desert_region();
        let influence = mixed_influence(owner, candidate, 0.62);

        let sample = sample_material_domain(MaterialDomainInput {
            hard_owner: owner,
            influence: &influence,
            smoothed: dry_smoothed(),
            hydrology: dry_hydrology(),
            world_x: 1024,
            world_z: -128,
        });

        assert_eq!(sample.visible_owner.archetype, RegionArchetype::DesertPlain);
        assert_ne!(
            sample.visible_domain,
            MaterialDomainKind::TemperateGreen,
            "visible material domain must not fall back to the rectangular hard-owner domain"
        );
    }

    #[test]
    fn hydrology_local_support_can_override_atlas_foreign_owner() {
        let owner = temperate_region(RegionArchetype::TemperatePlain);
        let candidate = desert_region();
        let influence = mixed_influence(owner, candidate, 0.58);

        let sample = sample_material_domain(MaterialDomainInput {
            hard_owner: owner,
            influence: &influence,
            smoothed: supported_smoothed(),
            hydrology: supported_hydrology(),
            world_x: 1032,
            world_z: -128,
        });

        assert_eq!(sample.visible_owner.archetype, RegionArchetype::DesertPlain);
        assert_eq!(sample.visible_domain, MaterialDomainKind::Wetland);
        assert!(!sample.accepted_foreign_owner);
        assert_eq!(
            sample.reason,
            MaterialDomainTransitionReason::LocalSupportInterior
        );
    }

    #[test]
    fn best_supported_foreign_domain_wins_over_heavier_but_unsupported_candidate() {
        let owner = temperate_region(RegionArchetype::TemperatePlain);
        let desert = desert_region();
        let wetland = wetland_region();
        let influence = RegionClassInfluenceSet {
            dominant: RegionClassInfluence {
                coord: AtlasCoord::new(0, 0),
                class: owner,
                weight: 0.45,
            },
            neighbors: vec![
                RegionClassInfluence {
                    coord: AtlasCoord::new(1, 0),
                    class: desert,
                    weight: 0.35,
                },
                RegionClassInfluence {
                    coord: AtlasCoord::new(0, 1),
                    class: wetland,
                    weight: 0.20,
                },
            ],
            transition_strength: 0.48,
            barrier_strength: 0.12,
        };

        let sample = sample_material_domain(MaterialDomainInput {
            hard_owner: owner,
            influence: &influence,
            smoothed: wet_smoothed(),
            hydrology: wet_hydrology(),
            world_x: 4096,
            world_z: -256,
        });

        assert_eq!(
            sample.visible_owner.archetype,
            RegionArchetype::ColdWetLowland
        );
        assert_eq!(sample.visible_domain, MaterialDomainKind::Wetland);
        assert!(sample.accepted_foreign_owner);
        assert_eq!(sample.candidate_domain, Some(MaterialDomainKind::Wetland));
    }

    #[test]
    fn boundary_phase_is_deterministic_and_adjacent_samples_are_not_salt_and_pepper() {
        let owner = temperate_region(RegionArchetype::TemperatePlain);
        let candidate = desert_region();
        let influence = mixed_influence(owner, candidate, 0.51);
        let smoothed = supported_smoothed();
        let hydrology = supported_hydrology();
        let mut previous = None;
        let mut switches = 0;
        let mut accepted = Vec::new();

        for world_x in 2048..2080 {
            let a = sample_material_domain(MaterialDomainInput {
                hard_owner: owner,
                influence: &influence,
                smoothed,
                hydrology,
                world_x,
                world_z: 512,
            });
            let b = sample_material_domain(MaterialDomainInput {
                hard_owner: owner,
                influence: &influence,
                smoothed,
                hydrology,
                world_x,
                world_z: 512,
            });

            assert_eq!(a.visible_domain, b.visible_domain);
            assert_eq!(a.transition_phase, b.transition_phase);
            assert_eq!(a.boundary_displacement, b.boundary_displacement);

            let current = a.accepted_foreign_owner;
            if let Some(previous) = previous
                && previous != current
            {
                switches += 1;
            }
            previous = Some(current);
            accepted.push(current);
        }

        let isolated = accepted
            .windows(3)
            .filter(|window| window[0] == window[2] && window[0] != window[1])
            .count();

        assert!(
            switches <= 4,
            "domain boundary flipped too often across adjacent columns: {switches}"
        );
        assert_eq!(
            isolated, 0,
            "boundary should not alternate in one-column speckles"
        );
    }

    #[test]
    fn strong_local_support_can_override_hard_domain_without_foreign_owner() {
        let owner = alpine_region();

        let sample = sample_material_domain(MaterialDomainInput {
            hard_owner: owner,
            influence: &single_owner_influence(owner),
            smoothed: green_supported_smoothed(),
            hydrology: dry_hydrology(),
            world_x: 1536,
            world_z: -448,
        });

        assert_eq!(sample.visible_owner.archetype, owner.archetype);
        assert_eq!(sample.visible_domain, MaterialDomainKind::TemperateGreen);
        assert_eq!(
            sample.reason,
            MaterialDomainTransitionReason::LocalSupportInterior
        );
        assert!(!sample.accepted_foreign_owner);
    }
}
