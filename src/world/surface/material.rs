#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialPolicyId {
    TemperateGrassland,
    TemperatePlateau,
    TropicalLowland,
    DesertSurface,
    ColdWetland,
    AlpineExposed,
    CoastalCliff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterialPolicyDef {
    pub id: MaterialPolicyId,
    pub summary: &'static str,
    pub top_cover_hint: &'static str,
    pub subsoil_hint: &'static str,
    pub sediment_override_hint: &'static str,
}

pub const MATERIAL_POLICIES: &[MaterialPolicyDef] = &[
    MaterialPolicyDef {
        id: MaterialPolicyId::TemperateGrassland,
        summary: "Default grass-over-dirt policy for temperate plains and gentle hills.",
        top_cover_hint: "grass",
        subsoil_hint: "dirt",
        sediment_override_hint: "river and lake materials override through hydrology.",
    },
    MaterialPolicyDef {
        id: MaterialPolicyId::TemperatePlateau,
        summary: "Grass and dirt policy with higher rock exposure on edges and stronger slopes.",
        top_cover_hint: "grass_or_sparse_grass",
        subsoil_hint: "dirt_with_rock_exposure",
        sediment_override_hint: "terrace shoulders may expose more stone later.",
    },
    MaterialPolicyDef {
        id: MaterialPolicyId::TropicalLowland,
        summary: "Dense warm-soil surface policy for rainforest and tropical wet lowlands.",
        top_cover_hint: "lush_grass_or_litter",
        subsoil_hint: "rich_dirt",
        sediment_override_hint: "wet pockets can transition to mud through hydrology.",
    },
    MaterialPolicyDef {
        id: MaterialPolicyId::DesertSurface,
        summary: "Sand-dominant surface with sparse moist exceptions.",
        top_cover_hint: "sand",
        subsoil_hint: "packed_sand",
        sediment_override_hint: "runoff corridors can reveal gravel or mud locally.",
    },
    MaterialPolicyDef {
        id: MaterialPolicyId::ColdWetland,
        summary: "Wet soil and marshy surface policy with strong freeze-thaw response.",
        top_cover_hint: "wet_grass_or_mud",
        subsoil_hint: "soft_dirt",
        sediment_override_hint: "winter can freeze exposed mud or shallow water edges.",
    },
    MaterialPolicyDef {
        id: MaterialPolicyId::AlpineExposed,
        summary: "Rock-forward surface with snow persistence and sparse cover.",
        top_cover_hint: "snow_or_exposed_rock",
        subsoil_hint: "thin_soil",
        sediment_override_hint: "melt channels can introduce gravel and wet rock later.",
    },
    MaterialPolicyDef {
        id: MaterialPolicyId::CoastalCliff,
        summary: "Rock and sparse-soil coast policy with shoreline sediment overrides.",
        top_cover_hint: "sparse_grass_or_rock",
        subsoil_hint: "thin_dirt_over_stone",
        sediment_override_hint: "shoreline and pocket beaches override through coast policy.",
    },
];

pub fn default_material_policies() -> &'static [MaterialPolicyDef] {
    MATERIAL_POLICIES
}
