use rayon::prelude::*;
use std::collections::HashMap;

use super::biome::{GraphBiomeContext, GraphBiomeKind, GraphBiomeWaterRole};
use super::heightfield::{HeightfieldColumn, HeightfieldTerrainKind, HeightfieldTile};
use super::macro_field::{MacroFieldSample, MacroFieldTile};
use crate::world::generation::graph::VoronoiSiteId;
use crate::world::legacy::surface::SurfaceCondition;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfacePlanConfig {
    pub seed: u64,
    pub generator_version: u32,
    pub default_soil_depth_blocks: u8,
    pub shallow_soil_depth_blocks: u8,
    pub deep_soil_depth_blocks: u8,
    pub boundary_mix_radius_blocks: u8,
    pub boundary_mix_strength_percent: u8,
    pub river_water_threshold: f32,
    pub coast_threshold: f32,
    pub ridge_threshold: f32,
    pub palette: SurfaceBlockPalette,
}

impl SurfacePlanConfig {
    pub fn new(seed: u64, generator_version: u32) -> Self {
        Self {
            seed,
            generator_version,
            default_soil_depth_blocks: 3,
            shallow_soil_depth_blocks: 1,
            deep_soil_depth_blocks: 5,
            boundary_mix_radius_blocks: 1,
            boundary_mix_strength_percent: 28,
            river_water_threshold: 0.88,
            coast_threshold: 0.45,
            ridge_threshold: 0.55,
            palette: SurfaceBlockPalette::default(),
        }
    }
}

impl Default for SurfacePlanConfig {
    fn default() -> Self {
        Self::new(0, 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceHydrologyRole {
    Ocean,
    Lake,
    River,
    Wetland,
    Coast,
    DryBasin,
    Ridge,
    Land,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceBlockPalette {
    pub water: &'static str,
    pub ice: &'static str,
    pub default_top: &'static str,
    pub default_subsurface: &'static str,
    pub default_base: &'static str,
}

impl Default for SurfaceBlockPalette {
    fn default() -> Self {
        Self {
            water: "water",
            ice: "ice",
            default_top: "grass",
            default_subsurface: "dirt",
            default_base: "stone",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiomeSurfacePolicy {
    pub biome: GraphBiomeKind,
    pub family: &'static str,
    pub default_top: &'static str,
    pub dry_top: &'static str,
    pub wet_top: &'static str,
    pub frozen_top: &'static str,
    pub subsurface: &'static str,
    pub base: &'static str,
    pub exposed: &'static str,
    pub sediment: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceColumnInput {
    pub world_x: i32,
    pub world_z: i32,
    pub heightfield: HeightfieldColumn,
    pub biome: Option<GraphBiomeKind>,
    pub biome_context: Option<GraphBiomeContext>,
    pub runtime_surface: Option<SurfaceCondition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceColumnPlan {
    pub world_x: i32,
    pub world_z: i32,
    pub surface_y: i32,
    pub water_y: Option<i32>,
    pub hydrology_role: SurfaceHydrologyRole,
    pub top_block: &'static str,
    pub subsurface_block: &'static str,
    pub base_block: &'static str,
    pub underwater_top_block: &'static str,
    pub exposed_block: &'static str,
    pub sediment_block: &'static str,
    pub soil_depth_blocks: u8,
    pub vegetation_allowed: bool,
    pub cover_phase: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SurfaceMixSource {
    owner_site: Option<VoronoiSiteId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurfacePlanArea {
    pub width: u32,
    pub height: u32,
    pub sample_spacing_blocks: f32,
    pub columns: Vec<SurfaceColumnPlan>,
    pub stats: SurfacePlanAreaStats,
    pub config: SurfacePlanConfig,
}

impl SurfacePlanArea {
    pub fn column(&self, x: u32, z: u32) -> Option<&SurfaceColumnPlan> {
        if x >= self.width || z >= self.height {
            return None;
        }
        self.columns
            .get(z as usize * self.width as usize + x as usize)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SurfacePlanAreaStats {
    pub column_count: usize,
    pub ocean_column_count: usize,
    pub lake_column_count: usize,
    pub river_column_count: usize,
    pub wetland_column_count: usize,
    pub coast_column_count: usize,
    pub dry_basin_column_count: usize,
    pub ridge_column_count: usize,
    pub land_column_count: usize,
    pub water_column_count: usize,
    pub exposed_column_count: usize,
    pub vegetation_allowed_count: usize,
}

pub fn biome_surface_policy(biome: GraphBiomeKind) -> BiomeSurfacePolicy {
    match biome {
        GraphBiomeKind::ShallowOcean => policy(
            biome,
            "OceanicShelf",
            "sand",
            "sand",
            "silt",
            "ice",
            "silt",
            "stone",
            "stone",
            "silt",
        ),
        GraphBiomeKind::DeepOcean => policy(
            biome,
            "DeepOceanFloor",
            "silt",
            "silt",
            "silt",
            "ice",
            "clay",
            "stone",
            "stone",
            "silt",
        ),
        GraphBiomeKind::Mangrove => policy(
            biome,
            "MangroveMudflat",
            "mud",
            "mud",
            "mud",
            "ice",
            "peat",
            "dirt",
            "humus",
            "silt",
        ),
        GraphBiomeKind::EstuarineCoast => policy(
            biome,
            "EstuarineMudflat",
            "mud",
            "mud",
            "mud",
            "ice",
            "silt",
            "clay",
            "wet_sand",
            "silt",
        ),
        GraphBiomeKind::LagoonCoast => policy(
            biome,
            "LagoonMudflat",
            "mud",
            "sand",
            "wet_sand",
            "ice",
            "silt",
            "clay",
            "sand",
            "silt",
        ),
        GraphBiomeKind::RockyCoast => policy(
            biome,
            "CoastalCliff",
            "gravel",
            "gravel",
            "wet_gravel",
            "snow",
            "gravel",
            "stone",
            "rock",
            "wet_gravel",
        ),
        GraphBiomeKind::SandyCoast => policy(
            biome,
            "SandyBeach",
            "sand",
            "sand",
            "wet_sand",
            "snow",
            "sand",
            "stone",
            "stone",
            "silt",
        ),
        GraphBiomeKind::Lake => policy(
            biome,
            "InlandLakeShore",
            "silt",
            "silt",
            "mud",
            "ice",
            "clay",
            "stone",
            "gravel",
            "silt",
        ),
        GraphBiomeKind::Marsh => policy(
            biome,
            "ColdWetland",
            "mud",
            "mud",
            "peat",
            "snow",
            "clay",
            "dirt",
            "gravel",
            "silt",
        ),
        GraphBiomeKind::Swamp => policy(
            biome,
            "WarmSwamp",
            "peat",
            "peat",
            "mud",
            "ice",
            "clay",
            "dirt",
            "humus",
            "silt",
        ),
        GraphBiomeKind::FloodedForest => policy(
            biome,
            "FloodedForestWetland",
            "mud",
            "mud",
            "peat",
            "snow",
            "silt",
            "dirt",
            "humus",
            "silt",
        ),
        GraphBiomeKind::Desert => policy(
            biome,
            "DesertSurface",
            "sand",
            "sand",
            "wet_sand",
            "sand",
            "sand",
            "sandstone",
            "exposed_rock",
            "gravel",
        ),
        GraphBiomeKind::SemiDesert => policy(
            biome,
            "SemiDesertTransition",
            "red_sand",
            "red_sand",
            "wet_sand",
            "snow",
            "coarse_dirt",
            "sandstone",
            "exposed_rock",
            "gravel",
        ),
        GraphBiomeKind::Steppe => policy(
            biome,
            "SteppeGrassland",
            "dry_grass",
            "coarse_dirt",
            "mud",
            "snow",
            "dirt",
            "stone",
            "gravel",
            "gravel",
        ),
        GraphBiomeKind::DryShrubland => policy(
            biome,
            "DryShrublandRocky",
            "coarse_dirt",
            "coarse_dirt",
            "mud",
            "snow",
            "dirt",
            "stone",
            "exposed_rock",
            "gravel",
        ),
        GraphBiomeKind::MediterraneanShrubland => policy(
            biome,
            "MediterraneanShrubland",
            "dry_grass",
            "dry_grass",
            "grass",
            "snow",
            "coarse_dirt",
            "stone",
            "exposed_rock",
            "gravel",
        ),
        GraphBiomeKind::PolarIce => policy(
            biome, "Icefield", "ice", "ice", "ice", "ice", "snow", "stone", "rock", "moraine",
        ),
        GraphBiomeKind::PolarBarrens => policy(
            biome,
            "TundraExposure",
            "thin_soil",
            "thin_soil",
            "moss",
            "snow",
            "dirt",
            "stone",
            "gravel",
            "gravel",
        ),
        GraphBiomeKind::Tundra => policy(
            biome,
            "TundraExposure",
            "moss",
            "thin_soil",
            "moss",
            "snow",
            "dirt",
            "stone",
            "gravel",
            "gravel",
        ),
        GraphBiomeKind::SubalpineWoodland => policy(
            biome,
            "SubalpineWoodland",
            "moss",
            "thin_soil",
            "moss",
            "snow",
            "thin_soil",
            "stone",
            "gravel",
            "gravel",
        ),
        GraphBiomeKind::AlpineMeadow => policy(
            biome,
            "AlpineMeadow",
            "alpine_soil",
            "alpine_soil",
            "gravel",
            "snow",
            "moraine",
            "stone",
            "rock",
            "gravel",
        ),
        GraphBiomeKind::BorealForest => policy(
            biome,
            "BorealForest",
            "podzol",
            "podzol",
            "moss",
            "snow",
            "dirt",
            "stone",
            "gravel",
            "gravel",
        ),
        GraphBiomeKind::TropicalRainforest => policy(
            biome,
            "TropicalLowland",
            "jungle_grass",
            "laterite",
            "mud",
            "mud",
            "humus",
            "dirt",
            "laterite",
            "silt",
        ),
        GraphBiomeKind::MonsoonForest => policy(
            biome,
            "MonsoonForest",
            "jungle_grass",
            "laterite",
            "mud",
            "mud",
            "humus",
            "dirt",
            "laterite",
            "silt",
        ),
        GraphBiomeKind::TropicalDryForest => policy(
            biome,
            "TropicalDryForest",
            "jungle_grass",
            "laterite",
            "mud",
            "mud",
            "humus",
            "stone",
            "laterite",
            "gravel",
        ),
        GraphBiomeKind::Savanna => policy(
            biome,
            "SavannaGrassland",
            "dry_grass",
            "dry_grass",
            "grass",
            "dry_grass",
            "dirt",
            "stone",
            "exposed_rock",
            "gravel",
        ),
        GraphBiomeKind::TemperateRainforest => policy(
            biome,
            "TemperateRainforest",
            "grass",
            "grass",
            "mud",
            "snow",
            "humus",
            "stone",
            "exposed_rock",
            "silt",
        ),
        GraphBiomeKind::TemperateMixedForest => policy(
            biome,
            "TemperateMixedForest",
            "grass",
            "coarse_dirt",
            "mud",
            "snow",
            "dirt",
            "stone",
            "exposed_rock",
            "gravel",
        ),
        GraphBiomeKind::TemperateBroadleafForest => policy(
            biome,
            "TemperateBroadleafForest",
            "grass",
            "grass",
            "mud",
            "snow",
            "humus",
            "stone",
            "exposed_rock",
            "gravel",
        ),
        GraphBiomeKind::TemperateGrassland => policy(
            biome,
            "TemperateGrassland",
            "grass",
            "coarse_dirt",
            "mud",
            "snow",
            "dirt",
            "stone",
            "exposed_rock",
            "gravel",
        ),
    }
}

pub fn resolve_surface_column(
    input: SurfaceColumnInput,
    config: SurfacePlanConfig,
) -> SurfaceColumnPlan {
    let biome = input.biome.unwrap_or(GraphBiomeKind::TemperateGrassland);
    let policy = biome_surface_policy(biome);
    let role = hydrology_role(&input, config);
    let variant = material_variant(input.world_x, input.world_z, config, policy.family);
    let mut top = policy.default_top;
    let mut subsurface = policy.subsurface;
    let base = policy.base;
    let mut underwater_top = policy.sediment;
    let exposed = policy.exposed;
    let mut sediment = policy.sediment;
    let mut soil_depth = config.default_soil_depth_blocks;
    let mut vegetation_allowed = true;

    match role {
        SurfaceHydrologyRole::Ocean => {
            top = if input.heightfield.terrain_ruggedness > 0.48 || variant > 80 {
                "gravel"
            } else {
                policy.sediment
            };
            underwater_top = top;
            sediment = top;
            subsurface = if variant > 70 { "clay" } else { "silt" };
            soil_depth = config.shallow_soil_depth_blocks;
            vegetation_allowed = false;
        }
        SurfaceHydrologyRole::Lake => {
            top = if input.heightfield.river_gravel_hint > 0.55 || variant > 86 {
                "gravel"
            } else if variant < 28 {
                "mud"
            } else {
                "silt"
            };
            underwater_top = top;
            sediment = top;
            subsurface = "clay";
            soil_depth = config.shallow_soil_depth_blocks;
            vegetation_allowed = false;
        }
        SurfaceHydrologyRole::River => {
            let gravelly = input.heightfield.river_gravel_hint > 0.45
                || input.heightfield.river_flow_hint > 0.58
                || variant > 64;
            top = if gravelly {
                "wet_gravel"
            } else if variant < 24 {
                "mud"
            } else {
                "silt"
            };
            underwater_top = top;
            sediment = if gravelly { "gravel" } else { "silt" };
            subsurface = sediment;
            soil_depth = config.shallow_soil_depth_blocks;
            vegetation_allowed = false;
        }
        SurfaceHydrologyRole::Wetland => {
            top = if variant < 38 {
                policy.wet_top
            } else if variant < 72 {
                "mud"
            } else {
                "peat"
            };
            underwater_top = top;
            sediment = if variant > 76 { "clay" } else { "silt" };
            subsurface = sediment;
            soil_depth = config.deep_soil_depth_blocks;
            vegetation_allowed = !input.heightfield.has_water_column();
        }
        SurfaceHydrologyRole::Coast => {
            top = if matches!(
                biome,
                GraphBiomeKind::Mangrove
                    | GraphBiomeKind::EstuarineCoast
                    | GraphBiomeKind::LagoonCoast
            ) {
                if should_apply_continuous_coast_mudflat(&input, config) {
                    if variant > 62 { "silt" } else { "mud" }
                } else if let Some(mouth_sediment) =
                    local_river_mouth_sediment(input.heightfield, config, variant)
                {
                    mouth_sediment
                } else if variant > 54 {
                    "wet_sand"
                } else {
                    "sand"
                }
            } else if input.heightfield.water_y.is_some() || variant > 54 {
                "wet_sand"
            } else {
                "sand"
            };
            underwater_top = top;
            sediment = "silt";
            subsurface = policy.subsurface;
            soil_depth = config.shallow_soil_depth_blocks;
            vegetation_allowed = false;
        }
        SurfaceHydrologyRole::DryBasin => {
            top = if variant < 34 {
                "clay"
            } else if variant < 68 {
                "silt"
            } else {
                policy.dry_top
            };
            underwater_top = top;
            sediment = if variant < 50 { "clay" } else { "silt" };
            subsurface = sediment;
            vegetation_allowed = false;
        }
        SurfaceHydrologyRole::Ridge => {
            if let Some(context) = input.biome_context {
                if context.hydration > 0.66 && variant > 50 {
                    top = policy.wet_top;
                } else if context.hydration < 0.34 && variant > 45 {
                    top = policy.dry_top;
                }
            }
        }
        SurfaceHydrologyRole::Land => {
            if let Some(bank_sediment) =
                local_dry_river_bank_sediment(input.heightfield, policy, config, variant)
            {
                top = bank_sediment;
                sediment = bank_sediment;
            } else if let Some(context) = input.biome_context {
                if context.hydration > 0.66 && variant > 50 {
                    top = policy.wet_top;
                } else if context.hydration < 0.34 && variant > 45 {
                    top = policy.dry_top;
                }
            }
        }
    }

    if let Some(surface) = input.runtime_surface {
        if surface.snow_depth > 0.35 {
            top = policy.frozen_top;
        } else if surface.wetness > 0.65
            && !matches!(
                role,
                SurfaceHydrologyRole::Ocean
                    | SurfaceHydrologyRole::Lake
                    | SurfaceHydrologyRole::River
            )
        {
            top = policy.wet_top;
        }
    }

    SurfaceColumnPlan {
        world_x: input.world_x,
        world_z: input.world_z,
        surface_y: input.heightfield.surface_y,
        water_y: input.heightfield.water_y,
        hydrology_role: role,
        top_block: top,
        subsurface_block: subsurface,
        base_block: base,
        underwater_top_block: underwater_top,
        exposed_block: exposed,
        sediment_block: sediment,
        soil_depth_blocks: soil_depth,
        vegetation_allowed,
        cover_phase: variant % 4,
    }
}

pub fn generate_surface_plan_area(
    heightfield: &HeightfieldTile,
    macro_field: Option<&MacroFieldTile>,
    config: SurfacePlanConfig,
) -> SurfacePlanArea {
    let mix_sources = macro_field.map(|tile| {
        (0..heightfield.columns.len())
            .map(|index| SurfaceMixSource {
                owner_site: tile
                    .samples
                    .get(index)
                    .and_then(|sample| sample.nearest_site),
            })
            .collect::<Vec<_>>()
    });
    let mut columns = heightfield
        .columns
        .par_iter()
        .enumerate()
        .map(|(index, column)| {
            let sample = macro_field.and_then(|tile| tile.samples.get(index));
            resolve_surface_column(
                SurfaceColumnInput {
                    world_x: column.position.x.round() as i32,
                    world_z: column.position.z.round() as i32,
                    heightfield: *column,
                    biome: sample.and_then(|sample| sample.biome),
                    biome_context: sample.and_then(|sample| sample.biome_context),
                    runtime_surface: None,
                },
                config,
            )
        })
        .collect::<Vec<_>>();
    apply_noisy_boundary_mixing(
        &mut columns,
        heightfield.width as usize,
        heightfield.height as usize,
        config,
        mix_sources.as_deref(),
    );
    normalize_non_water_owner_surface_materials(&mut columns, macro_field, config);
    let stats = surface_plan_stats(&columns);

    SurfacePlanArea {
        width: heightfield.width,
        height: heightfield.height,
        sample_spacing_blocks: heightfield.sample_spacing_blocks,
        columns,
        stats,
        config,
    }
}

fn normalize_non_water_owner_surface_materials(
    columns: &mut [SurfaceColumnPlan],
    macro_field: Option<&MacroFieldTile>,
    config: SurfacePlanConfig,
) {
    let Some(tile) = macro_field.filter(|tile| tile.samples.len() == columns.len()) else {
        return;
    };

    let mut top_by_site = HashMap::<VoronoiSiteId, &'static str>::new();
    for (sample, column) in tile.samples.iter().zip(columns.iter()) {
        if !is_non_water_owner_surface_column(*column) {
            continue;
        }
        let (Some(site), Some(top)) = (
            sample.nearest_site,
            canonical_non_water_owner_top(*sample, config),
        ) else {
            continue;
        };
        top_by_site.entry(site).or_insert(top);
    }

    for (sample, column) in tile.samples.iter().zip(columns.iter_mut()) {
        if !is_non_water_owner_surface_column(*column) {
            continue;
        }
        let Some(site) = sample.nearest_site else {
            continue;
        };
        let Some(top) = top_by_site.get(&site).copied() else {
            continue;
        };
        column.top_block = top;
        column.underwater_top_block = top;
    }
}

fn canonical_non_water_owner_top(
    sample: MacroFieldSample,
    _config: SurfacePlanConfig,
) -> Option<&'static str> {
    let biome = sample.biome.unwrap_or(GraphBiomeKind::TemperateGrassland);
    let policy = biome_surface_policy(biome);
    let context = sample.biome_context?;

    match context.water_role {
        GraphBiomeWaterRole::ShallowOcean
        | GraphBiomeWaterRole::DeepOcean
        | GraphBiomeWaterRole::Lake => Some(policy.default_top),
        GraphBiomeWaterRole::Coast => Some("sand"),
        GraphBiomeWaterRole::Wetland => Some(policy.wet_top),
        GraphBiomeWaterRole::DryBasin => Some(policy.dry_top),
        GraphBiomeWaterRole::Land => {
            if is_coast_biome(Some(biome)) {
                Some("sand")
            } else if context.hydration > 0.66 {
                Some(policy.wet_top)
            } else if context.hydration < 0.34 {
                Some(policy.dry_top)
            } else {
                Some(policy.default_top)
            }
        }
    }
}

fn is_non_water_owner_surface_column(column: SurfaceColumnPlan) -> bool {
    column.water_y.is_none()
}

fn apply_noisy_boundary_mixing(
    columns: &mut [SurfaceColumnPlan],
    width: usize,
    height: usize,
    config: SurfacePlanConfig,
    mix_sources: Option<&[SurfaceMixSource]>,
) {
    if columns.is_empty()
        || width == 0
        || height == 0
        || config.boundary_mix_radius_blocks == 0
        || config.boundary_mix_strength_percent == 0
    {
        return;
    }

    let original = columns.to_vec();
    let mix_sources = mix_sources.filter(|sources| sources.len() == original.len());
    let radius = usize::from(config.boundary_mix_radius_blocks);
    for z in 0..height {
        for x in 0..width {
            let index = z * width + x;
            let current = original[index];
            let current_source = mix_sources.and_then(|sources| sources.get(index).copied());
            let Some((neighbor, distance)) = nearest_mix_candidate(
                &original,
                mix_sources,
                width,
                height,
                x,
                z,
                radius,
                current,
                current_source,
                config,
            ) else {
                continue;
            };

            let strength = boundary_mix_strength(distance, radius, config);
            let roll =
                boundary_mix_roll(current.world_x, current.world_z, neighbor.top_block, config);
            if roll >= strength {
                continue;
            }

            copy_visual_material(&mut columns[index], neighbor);
        }
    }
    restore_cross_owner_boundary_swaps(columns, &original, width, height, mix_sources);
}

fn nearest_mix_candidate(
    columns: &[SurfaceColumnPlan],
    mix_sources: Option<&[SurfaceMixSource]>,
    width: usize,
    height: usize,
    x: usize,
    z: usize,
    radius: usize,
    current: SurfaceColumnPlan,
    current_source: Option<SurfaceMixSource>,
    config: SurfacePlanConfig,
) -> Option<(SurfaceColumnPlan, usize)> {
    let mut best: Option<(SurfaceColumnPlan, usize, u64)> = None;
    let min_x = x.saturating_sub(radius);
    let max_x = (x + radius).min(width.saturating_sub(1));
    let min_z = z.saturating_sub(radius);
    let max_z = (z + radius).min(height.saturating_sub(1));

    for nz in min_z..=max_z {
        for nx in min_x..=max_x {
            if nx == x && nz == z {
                continue;
            }
            let dx = nx.abs_diff(x);
            let dz = nz.abs_diff(z);
            let distance = dx + dz;
            if distance == 0 || distance > radius {
                continue;
            }
            let neighbor_index = nz * width + nx;
            let neighbor = columns[neighbor_index];
            let neighbor_source =
                mix_sources.and_then(|sources| sources.get(neighbor_index).copied());
            if !boundary_mix_direction_allows(current, neighbor, config) {
                continue;
            }
            if !mix_candidate_allowed_with_sources(
                current,
                neighbor,
                current_source,
                neighbor_source,
            ) {
                continue;
            }
            let tie = boundary_candidate_tie_breaker(current, neighbor, config);
            match best {
                Some((_, best_distance, best_tie))
                    if distance > best_distance
                        || (distance == best_distance && tie >= best_tie) => {}
                _ => best = Some((neighbor, distance, tie)),
            }
        }
    }

    best.map(|(neighbor, distance, _)| (neighbor, distance))
}

fn mix_candidate_allowed_with_sources(
    current: SurfaceColumnPlan,
    neighbor: SurfaceColumnPlan,
    current_source: Option<SurfaceMixSource>,
    neighbor_source: Option<SurfaceMixSource>,
) -> bool {
    if current.top_block == neighbor.top_block {
        return false;
    }
    if let (Some(current_site), Some(neighbor_site)) = (
        current_source.and_then(|source| source.owner_site),
        neighbor_source.and_then(|source| source.owner_site),
    ) {
        if current_site != neighbor_site {
            return false;
        }
    }
    if current.water_y.is_some() != neighbor.water_y.is_some() {
        return false;
    }
    if protected_water_role(current.hydrology_role) || protected_water_role(neighbor.hydrology_role)
    {
        return current.hydrology_role == neighbor.hydrology_role;
    }
    true
}

fn boundary_mix_direction_allows(
    current: SurfaceColumnPlan,
    neighbor: SurfaceColumnPlan,
    config: SurfacePlanConfig,
) -> bool {
    boundary_mix_endpoint_key(current, config) < boundary_mix_endpoint_key(neighbor, config)
}

fn boundary_mix_endpoint_key(column: SurfaceColumnPlan, config: SurfacePlanConfig) -> u64 {
    splitmix64(
        config.seed
            ^ ((config.generator_version as u64) << 32)
            ^ (column.world_x as i64 as u64).wrapping_mul(0xD1B5_4A32_D192_ED03)
            ^ (column.world_z as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB)
            ^ stable_str_hash(column.top_block)
            ^ stable_str_hash(column.subsurface_block).rotate_left(17)
            ^ 0xC6BC_2796_92B5_CC83,
    )
}

fn protected_water_role(role: SurfaceHydrologyRole) -> bool {
    matches!(
        role,
        SurfaceHydrologyRole::Ocean | SurfaceHydrologyRole::Lake | SurfaceHydrologyRole::River
    )
}

fn boundary_mix_strength(distance: usize, radius: usize, config: SurfacePlanConfig) -> u8 {
    let radius = radius.max(1);
    let falloff = radius.saturating_sub(distance.saturating_sub(1)).max(1);
    let strength = u16::from(config.boundary_mix_strength_percent) * falloff as u16 / radius as u16;
    strength.clamp(1, 100) as u8
}

fn boundary_mix_roll(
    world_x: i32,
    world_z: i32,
    source_top_block: &'static str,
    config: SurfacePlanConfig,
) -> u8 {
    let mut value = config.seed
        ^ ((config.generator_version as u64) << 32)
        ^ (world_x as i64 as u64).wrapping_mul(0xD6E8_FD93_56A6_1C53)
        ^ (world_z as i64 as u64).wrapping_mul(0xA076_1D64_78BD_642F)
        ^ stable_str_hash(source_top_block)
        ^ 0xB0B0_51A7_E5AA_1234;
    value = splitmix64(value);
    (value % 100) as u8
}

fn boundary_candidate_tie_breaker(
    current: SurfaceColumnPlan,
    neighbor: SurfaceColumnPlan,
    config: SurfacePlanConfig,
) -> u64 {
    splitmix64(
        config.seed
            ^ ((config.generator_version as u64) << 32)
            ^ (current.world_x as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB)
            ^ (current.world_z as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ stable_str_hash(neighbor.top_block),
    )
}

fn copy_visual_material(target: &mut SurfaceColumnPlan, source: SurfaceColumnPlan) {
    target.top_block = source.top_block;
    target.subsurface_block = source.subsurface_block;
    target.underwater_top_block = source.underwater_top_block;
    target.exposed_block = source.exposed_block;
    target.sediment_block = source.sediment_block;
    target.soil_depth_blocks = target.soil_depth_blocks.min(source.soil_depth_blocks);
    target.vegetation_allowed &= source.vegetation_allowed;
    target.cover_phase = source.cover_phase;
}

fn restore_cross_owner_boundary_swaps(
    columns: &mut [SurfaceColumnPlan],
    original: &[SurfaceColumnPlan],
    width: usize,
    height: usize,
    mix_sources: Option<&[SurfaceMixSource]>,
) {
    let Some(mix_sources) = mix_sources.filter(|sources| sources.len() == original.len()) else {
        return;
    };

    for z in 0..height {
        for x in 0..width {
            let index = z * width + x;
            for neighbor_index in [
                (x + 1 < width).then_some(index + 1),
                (z + 1 < height).then_some(index + width),
            ]
            .into_iter()
            .flatten()
            {
                if !is_cross_owner_boundary_swap(
                    columns,
                    original,
                    mix_sources,
                    index,
                    neighbor_index,
                ) {
                    continue;
                }
                restore_visual_material(&mut columns[index], original[index]);
                restore_visual_material(&mut columns[neighbor_index], original[neighbor_index]);
            }
        }
    }
}

fn is_cross_owner_boundary_swap(
    columns: &[SurfaceColumnPlan],
    original: &[SurfaceColumnPlan],
    mix_sources: &[SurfaceMixSource],
    left_index: usize,
    right_index: usize,
) -> bool {
    let (Some(left_site), Some(right_site)) = (
        mix_sources[left_index].owner_site,
        mix_sources[right_index].owner_site,
    ) else {
        return false;
    };
    if left_site == right_site {
        return false;
    }

    let left_base = original[left_index];
    let right_base = original[right_index];
    if left_base.top_block == right_base.top_block {
        return false;
    }

    let left_final = columns[left_index];
    let right_final = columns[right_index];
    left_final.top_block == right_base.top_block && right_final.top_block == left_base.top_block
}

fn restore_visual_material(target: &mut SurfaceColumnPlan, source: SurfaceColumnPlan) {
    target.top_block = source.top_block;
    target.subsurface_block = source.subsurface_block;
    target.underwater_top_block = source.underwater_top_block;
    target.exposed_block = source.exposed_block;
    target.sediment_block = source.sediment_block;
    target.soil_depth_blocks = source.soil_depth_blocks;
    target.vegetation_allowed = source.vegetation_allowed;
    target.cover_phase = source.cover_phase;
}

fn hydrology_role(input: &SurfaceColumnInput, config: SurfacePlanConfig) -> SurfaceHydrologyRole {
    let heightfield = input.heightfield;
    if heightfield.terrain_kind == HeightfieldTerrainKind::Ocean || heightfield.ocean_mask > 0.5 {
        SurfaceHydrologyRole::Ocean
    } else if input.biome_context.is_some_and(|context| {
        matches!(
            context.water_role,
            GraphBiomeWaterRole::ShallowOcean | GraphBiomeWaterRole::DeepOcean
        )
    }) {
        SurfaceHydrologyRole::Ocean
    } else if heightfield.terrain_kind == HeightfieldTerrainKind::Lake
        || heightfield.lake_mask > 0.5
        || input
            .biome_context
            .is_some_and(|context| context.water_role == GraphBiomeWaterRole::Lake)
        || input.biome == Some(GraphBiomeKind::Lake)
    {
        SurfaceHydrologyRole::Lake
    } else if heightfield.terrain_kind == HeightfieldTerrainKind::River
        || (heightfield.river_valley_strength >= config.river_water_threshold
            && heightfield.river_flow_hint > 0.0)
    {
        SurfaceHydrologyRole::River
    } else if is_wetland(input) {
        SurfaceHydrologyRole::Wetland
    } else if heightfield.terrain_kind == HeightfieldTerrainKind::Coast
        || input
            .biome_context
            .is_some_and(|context| context.water_role == GraphBiomeWaterRole::Coast)
        || is_coast_biome(input.biome)
    {
        SurfaceHydrologyRole::Coast
    } else if heightfield.terrain_kind == HeightfieldTerrainKind::DryBasin
        || heightfield.dry_basin_mask > 0.5
        || input
            .biome_context
            .is_some_and(|context| context.water_role == GraphBiomeWaterRole::DryBasin)
    {
        SurfaceHydrologyRole::DryBasin
    } else if heightfield.terrain_kind == HeightfieldTerrainKind::Ridge
        || heightfield.ridge_influence >= config.ridge_threshold
    {
        SurfaceHydrologyRole::Ridge
    } else {
        SurfaceHydrologyRole::Land
    }
}

fn is_wetland(input: &SurfaceColumnInput) -> bool {
    matches!(
        input.biome,
        Some(GraphBiomeKind::Marsh | GraphBiomeKind::Swamp | GraphBiomeKind::FloodedForest)
    ) || input
        .biome_context
        .is_some_and(|context| context.water_role == GraphBiomeWaterRole::Wetland)
}

fn is_coast_biome(biome: Option<GraphBiomeKind>) -> bool {
    matches!(
        biome,
        Some(
            GraphBiomeKind::Mangrove
                | GraphBiomeKind::EstuarineCoast
                | GraphBiomeKind::LagoonCoast
                | GraphBiomeKind::RockyCoast
                | GraphBiomeKind::SandyCoast
        )
    )
}

fn should_apply_continuous_coast_mudflat(
    input: &SurfaceColumnInput,
    config: SurfacePlanConfig,
) -> bool {
    input.heightfield.water_y.is_some()
        || input.heightfield.terrain_kind == HeightfieldTerrainKind::Coast
        || input.heightfield.coast_mask >= config.coast_threshold * 1.6
}

fn local_river_mouth_sediment(
    heightfield: HeightfieldColumn,
    config: SurfacePlanConfig,
    variant: u8,
) -> Option<&'static str> {
    let selected_mouth_hint = heightfield.river_flow_hint > 0.12
        && heightfield.river_bed_depth_blocks > 0.0
        && heightfield.river_valley_strength >= config.river_water_threshold * 0.68;
    if !selected_mouth_hint {
        return None;
    }

    if variant < 12 {
        Some("mud")
    } else if variant > 84 {
        Some("silt")
    } else {
        None
    }
}

fn local_dry_river_bank_sediment(
    heightfield: HeightfieldColumn,
    policy: BiomeSurfacePolicy,
    config: SurfacePlanConfig,
    variant: u8,
) -> Option<&'static str> {
    let selected_bank_hint = heightfield.river_flow_hint > 0.12
        && heightfield.river_valley_strength >= config.river_water_threshold * 0.72
        && (heightfield.river_bed_depth_blocks > 0.0
            || heightfield.river_bank_roughness_hint > 0.48
            || heightfield.river_gravel_hint > 0.32
            || heightfield.river_cutbank_hint > 0.38);
    if !selected_bank_hint {
        return None;
    }

    if variant > 86 {
        Some("gravel")
    } else if variant < 10 {
        Some(policy.sediment)
    } else {
        None
    }
}

fn material_variant(
    world_x: i32,
    world_z: i32,
    config: SurfacePlanConfig,
    family: &'static str,
) -> u8 {
    let mut value = config.seed
        ^ ((config.generator_version as u64) << 32)
        ^ (world_x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (world_z as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ stable_str_hash(family);
    value = splitmix64(value);
    (value % 100) as u8
}

fn stable_str_hash(value: &str) -> u64 {
    value.bytes().fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01B3)
    })
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn policy(
    biome: GraphBiomeKind,
    family: &'static str,
    default_top: &'static str,
    dry_top: &'static str,
    wet_top: &'static str,
    frozen_top: &'static str,
    subsurface: &'static str,
    base: &'static str,
    exposed: &'static str,
    sediment: &'static str,
) -> BiomeSurfacePolicy {
    BiomeSurfacePolicy {
        biome,
        family,
        default_top,
        dry_top,
        wet_top,
        frozen_top,
        subsurface,
        base,
        exposed,
        sediment,
    }
}

fn surface_plan_stats(columns: &[SurfaceColumnPlan]) -> SurfacePlanAreaStats {
    let mut stats = SurfacePlanAreaStats {
        column_count: columns.len(),
        ..SurfacePlanAreaStats::default()
    };
    for column in columns {
        match column.hydrology_role {
            SurfaceHydrologyRole::Ocean => stats.ocean_column_count += 1,
            SurfaceHydrologyRole::Lake => stats.lake_column_count += 1,
            SurfaceHydrologyRole::River => stats.river_column_count += 1,
            SurfaceHydrologyRole::Wetland => stats.wetland_column_count += 1,
            SurfaceHydrologyRole::Coast => stats.coast_column_count += 1,
            SurfaceHydrologyRole::DryBasin => stats.dry_basin_column_count += 1,
            SurfaceHydrologyRole::Ridge => stats.ridge_column_count += 1,
            SurfaceHydrologyRole::Land => stats.land_column_count += 1,
        }
        if column.water_y.is_some() {
            stats.water_column_count += 1;
        }
        if column.top_block == column.exposed_block {
            stats.exposed_column_count += 1;
        }
        if column.vegetation_allowed {
            stats.vegetation_allowed_count += 1;
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::graph::WorldPlanePoint;
    use crate::world::generation::macro_field::{
        MacroFieldRasterContext, MacroFieldSample, MacroFieldTileConfig, MacroFieldTileStats,
    };
    use crate::world::generation::{
        BoundaryConfig, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS,
        GraphRegionArea, HeightfieldConfig, HeightfieldPerlinConfig, HydrologyConfig,
        MacroMapConfig, NoisyBoundaryCurve, VoronoiGraphConfig, VoronoiGraphPatchRequest,
        VoronoiSiteId, apply_headwater_source_hydration_to_biomes, build_river_plan,
        generate_heightfield_tile, generate_macro_field_tile, generate_macro_map,
        generate_noisy_boundaries, generate_voronoi_graph_patch, graph_region_for_world_block,
        solve_hydrology,
    };
    use crate::world::{CHUNK_EDGE_I32, WorldMeta};
    use std::collections::{BTreeMap, BTreeSet};

    const ALL_BIOMES: &[GraphBiomeKind] = &[
        GraphBiomeKind::ShallowOcean,
        GraphBiomeKind::DeepOcean,
        GraphBiomeKind::Mangrove,
        GraphBiomeKind::EstuarineCoast,
        GraphBiomeKind::LagoonCoast,
        GraphBiomeKind::RockyCoast,
        GraphBiomeKind::SandyCoast,
        GraphBiomeKind::Lake,
        GraphBiomeKind::Marsh,
        GraphBiomeKind::Swamp,
        GraphBiomeKind::FloodedForest,
        GraphBiomeKind::Desert,
        GraphBiomeKind::SemiDesert,
        GraphBiomeKind::Steppe,
        GraphBiomeKind::DryShrubland,
        GraphBiomeKind::MediterraneanShrubland,
        GraphBiomeKind::PolarIce,
        GraphBiomeKind::PolarBarrens,
        GraphBiomeKind::Tundra,
        GraphBiomeKind::SubalpineWoodland,
        GraphBiomeKind::AlpineMeadow,
        GraphBiomeKind::BorealForest,
        GraphBiomeKind::TropicalRainforest,
        GraphBiomeKind::MonsoonForest,
        GraphBiomeKind::TropicalDryForest,
        GraphBiomeKind::Savanna,
        GraphBiomeKind::TemperateRainforest,
        GraphBiomeKind::TemperateMixedForest,
        GraphBiomeKind::TemperateBroadleafForest,
        GraphBiomeKind::TemperateGrassland,
    ];

    const KNOWN_BLOCK_KEYS: &[&str] = &[
        "grass",
        "jungle_grass",
        "dirt",
        "stone",
        "sand",
        "gravel",
        "mud",
        "snow",
        "water",
        "coarse_dirt",
        "clay",
        "ice",
        "peat",
        "silt",
        "humus",
        "moss",
        "podzol",
        "thin_soil",
        "red_sand",
        "dry_grass",
        "wet_sand",
        "wet_gravel",
        "exposed_rock",
        "rock",
        "sandstone",
        "alpine_soil",
        "moraine",
        "laterite",
    ];

    #[test]
    #[ignore = "diagnostic audit for the seed 42 surface-plan preview footprint"]
    fn diagnose_seed42_surface_plan_contract_data() {
        let seed = 42;
        let center_chunk_x = std::env::var("NW_SURFACE_AUDIT_CENTER_X")
            .ok()
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(-70);
        let center_chunk_z = std::env::var("NW_SURFACE_AUDIT_CENTER_Z")
            .ok()
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(-32);
        let chunk_radius = std::env::var("NW_SURFACE_AUDIT_RADIUS")
            .ok()
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(8);
        let meta = WorldMeta::new(seed);
        let min_chunk_x = center_chunk_x - chunk_radius;
        let _max_chunk_x = center_chunk_x + chunk_radius;
        let min_chunk_z = center_chunk_z - chunk_radius;
        let _max_chunk_z = center_chunk_z + chunk_radius;
        let min_world_x = min_chunk_x * CHUNK_EDGE_I32;
        let min_world_z = min_chunk_z * CHUNK_EDGE_I32;
        let chunk_count = chunk_radius * 2 + 1;
        let columns_x = (chunk_count * CHUNK_EDGE_I32) as u32;
        let columns_z = columns_x;
        let center_world_x = min_world_x + columns_x as i32 / 2;
        let center_world_z = min_world_z + columns_z as i32 / 2;
        let max_world_x_exclusive = min_world_x + columns_x as i32;
        let max_world_z_exclusive = min_world_z + columns_z as i32;
        let graph_area = GraphRegionArea::new(
            graph_region_for_world_block(
                min_world_x,
                min_world_z,
                DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            ),
            graph_region_for_world_block(
                max_world_x_exclusive,
                max_world_z_exclusive,
                DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            ),
        )
        .expect("valid audit graph area");
        let center_region = graph_region_for_world_block(
            center_world_x,
            center_world_z,
            DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
        );
        let padding_regions = required_audit_padding_regions(center_region, graph_area);
        let patch = generate_voronoi_graph_patch(VoronoiGraphPatchRequest::new(
            VoronoiGraphConfig {
                seed: meta.seed,
                generator_version: meta.generator_version,
                region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
                site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
                padding_regions,
            },
            center_world_x,
            center_world_z,
        ));
        let mut macro_map = generate_macro_map(
            &patch,
            MacroMapConfig {
                land_bias: MacroMapConfig::new(meta.seed, meta.generator_version).land_bias,
                ..MacroMapConfig::new(meta.seed, meta.generator_version)
            },
        );
        let hydrology = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
        apply_headwater_source_hydration_to_biomes(&patch, &mut macro_map, &hydrology);
        let boundary = generate_noisy_boundaries(
            &patch,
            &macro_map,
            BoundaryConfig::new(meta.seed, meta.generator_version),
        );
        let river_plan = build_river_plan(&patch, &macro_map, &hydrology, Default::default());
        let macro_tile = generate_macro_field_tile(
            &patch,
            &macro_map,
            &river_plan,
            &boundary,
            MacroFieldTileConfig::new(
                min_world_x as f32 + 0.5,
                min_world_z as f32 + 0.5,
                columns_x,
                columns_z,
                1.0,
            ),
        );
        let raw_context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let heightfield = generate_heightfield_tile(
            &macro_tile,
            HeightfieldConfig {
                perlin: HeightfieldPerlinConfig::preview_enabled(meta.seed, meta.generator_version),
                ..HeightfieldConfig::default()
            },
        );
        let surface_config = SurfacePlanConfig::default();
        let base_surface_config = SurfacePlanConfig {
            boundary_mix_radius_blocks: 0,
            ..surface_config
        };
        let base_surface_plan =
            generate_surface_plan_area(&heightfield, Some(&macro_tile), base_surface_config);
        let surface_plan =
            generate_surface_plan_area(&heightfield, Some(&macro_tile), surface_config);
        let surface_mix_sources = macro_tile
            .samples
            .iter()
            .map(|sample| SurfaceMixSource {
                owner_site: sample.nearest_site,
            })
            .collect::<Vec<_>>();

        let mut anomaly_count = 0usize;
        let mut representatives = Vec::new();
        let mut top_counts = BTreeMap::<&'static str, usize>::new();
        let mut role_counts = BTreeMap::<String, usize>::new();
        let mut biome_counts = BTreeMap::<String, usize>::new();
        let mut site_biomes = BTreeMap::<String, BTreeSet<&'static str>>::new();
        let mut representative_rows = BTreeMap::<String, String>::new();
        let mut missing_owner_biome_count = 0usize;
        let mut missing_owner_biome_representatives = Vec::new();
        let mut owner_biome_mismatch_count = 0usize;
        let mut owner_biome_mismatch_representatives = Vec::new();
        let mut raw_owner_switch_count = 0usize;
        let mut unsupported_raw_owner_switch_count = 0usize;
        let mut raw_owner_switch_representatives = Vec::new();
        let mut local_mix_count = 0usize;
        let mut local_mix_pair_counts = BTreeMap::<String, usize>::new();
        let mut unsupported_local_mix_count = 0usize;
        let mut unsupported_local_mix_representatives = Vec::new();
        let mut owner_boundary_pair_counts = BTreeMap::<String, usize>::new();
        let mut owner_boundary_swap_count = 0usize;
        let mut owner_boundary_swap_representatives = Vec::new();

        for (index, (((sample, height), base_plan), plan)) in macro_tile
            .samples
            .iter()
            .zip(&heightfield.columns)
            .zip(&base_surface_plan.columns)
            .zip(&surface_plan.columns)
            .enumerate()
        {
            *top_counts.entry(plan.top_block).or_default() += 1;
            *role_counts
                .entry(format!("{:?}", plan.hydrology_role))
                .or_default() += 1;
            let biome = sample.biome.unwrap_or(GraphBiomeKind::TemperateGrassland);
            *biome_counts.entry(format!("{biome:?}")).or_default() += 1;
            let key = format!(
                "{:?}/{:?}/{}",
                sample.biome, base_plan.hydrology_role, base_plan.top_block
            );
            representative_rows.entry(key).or_insert_with(|| {
                format!(
                    "index={index} world=({}, {}) site={:?} biome={:?} water_role={:?} terrain={:?} role={:?} top={} policy_default={} surface_y={} water_y={:?}",
                    base_plan.world_x,
                    base_plan.world_z,
                    sample.nearest_site,
                    sample.biome,
                    sample.biome_context.map(|context| context.water_role),
                    height.terrain_kind,
                    base_plan.hydrology_role,
                    base_plan.top_block,
                    biome_surface_policy(biome).default_top,
                    base_plan.surface_y,
                    base_plan.water_y,
                )
            });
            if let Some(site) = sample.nearest_site {
                site_biomes
                    .entry(format!("{site:?}"))
                    .or_default()
                    .insert(base_plan.top_block);
                if sample.biome.is_none() || sample.biome_context.is_none() {
                    missing_owner_biome_count += 1;
                    if missing_owner_biome_representatives.len() < 12 {
                        missing_owner_biome_representatives.push(format!(
                            "#{index} world=({}, {}) site={site:?} biome={:?} water_role={:?} terrain={:?} role={:?} top={}",
                            base_plan.world_x,
                            base_plan.world_z,
                            sample.biome,
                            sample.biome_context.map(|context| context.water_role),
                            height.terrain_kind,
                            base_plan.hydrology_role,
                            base_plan.top_block,
                        ));
                    }
                }
                if let Some(expected) = macro_map.biome(site) {
                    if sample.biome != Some(expected.biome)
                        || sample.biome_context != Some(expected.context)
                    {
                        owner_biome_mismatch_count += 1;
                        if owner_biome_mismatch_representatives.len() < 12 {
                            owner_biome_mismatch_representatives.push(format!(
                                "#{index} world=({}, {}) owner={site:?} sample_biome={:?} expected_biome={:?} sample_water_role={:?} expected_water_role={:?} terrain={:?} role={:?} top={}",
                                base_plan.world_x,
                                base_plan.world_z,
                                sample.biome,
                                expected.biome,
                                sample.biome_context.map(|context| context.water_role),
                                Some(expected.context.water_role),
                                height.terrain_kind,
                                base_plan.hydrology_role,
                                base_plan.top_block,
                            ));
                        }
                    }
                }
            }
            let raw_site = raw_context
                .nearest_site(sample.position)
                .map(|site| site.id);
            if raw_site != sample.nearest_site {
                raw_owner_switch_count += 1;
                let allowance = raw_owner_switch_allowance(
                    &boundary.curves,
                    raw_site,
                    sample.nearest_site,
                    sample.position,
                    macro_tile.config.sample_spacing_blocks,
                );
                if !allowance.is_some_and(|(_, _, within)| within) {
                    unsupported_raw_owner_switch_count += 1;
                    if raw_owner_switch_representatives.len() < 24 {
                        raw_owner_switch_representatives.push(format!(
                            "#{index} world=({}, {}) raw={:?} owner={:?} biome={:?} water_role={:?} terrain={:?} role={:?} top={} boundary_allowance={:?}",
                            plan.world_x,
                            plan.world_z,
                            raw_site,
                            sample.nearest_site,
                            sample.biome,
                            sample.biome_context.map(|context| context.water_role),
                            height.terrain_kind,
                            plan.hydrology_role,
                            plan.top_block,
                            allowance,
                        ));
                    }
                }
            }
            if plan.top_block != base_plan.top_block {
                local_mix_count += 1;
                *local_mix_pair_counts
                    .entry(format!("{}->{}", base_plan.top_block, plan.top_block))
                    .or_default() += 1;

                if !is_supported_local_mix(
                    &base_surface_plan.columns,
                    base_surface_plan.width as usize,
                    base_surface_plan.height as usize,
                    index,
                    *base_plan,
                    plan.top_block,
                    Some(&surface_mix_sources),
                    surface_config,
                ) {
                    unsupported_local_mix_count += 1;
                    if unsupported_local_mix_representatives.len() < 24 {
                        unsupported_local_mix_representatives.push(format!(
                            "#{index} world=({}, {}) site={:?} biome={:?} water_role={:?} base_role={:?} base_top={} final_role={:?} final_top={}",
                            plan.world_x,
                            plan.world_z,
                            sample.nearest_site,
                            sample.biome,
                            sample.biome_context.map(|context| context.water_role),
                            base_plan.hydrology_role,
                            base_plan.top_block,
                            plan.hydrology_role,
                            plan.top_block,
                        ));
                    }
                }
            }
            let allowed = documented_allowed_tops(
                SurfaceColumnInput {
                    world_x: base_plan.world_x,
                    world_z: base_plan.world_z,
                    heightfield: *height,
                    biome: sample.biome,
                    biome_context: sample.biome_context,
                    runtime_surface: None,
                },
                base_surface_config,
            );
            if !allowed.contains(base_plan.top_block) {
                anomaly_count += 1;
                if representatives.len() < 24 {
                    representatives.push(format!(
                        "#{index} world=({}, {}) site={:?} biome={:?} water_role={:?} terrain={:?} role={:?} top={} allowed={:?} surface_y={} water_y={:?} river(core={:.3},valley={:.3},flow={:.3},bed={:.3},gravel={:.3}) masks(ocean={:.3},lake={:.3},coast={:.3},dry={:.3})",
                        base_plan.world_x,
                        base_plan.world_z,
                        sample.nearest_site,
                        sample.biome,
                        sample.biome_context.map(|context| context.water_role),
                        height.terrain_kind,
                        base_plan.hydrology_role,
                        base_plan.top_block,
                        allowed,
                        base_plan.surface_y,
                        base_plan.water_y,
                        height.river_core_strength,
                        height.river_valley_strength,
                        height.river_flow_hint,
                        height.river_bed_depth_blocks,
                        height.river_gravel_hint,
                        sample.ocean_mask,
                        sample.lake_mask,
                        sample.coast_mask,
                        sample.dry_basin_mask,
                    ));
                }
            }
        }

        let width = macro_tile.config.width as usize;
        let height = macro_tile.config.height as usize;
        for z in 0..height {
            for x in 0..width {
                let index = z * width + x;
                for (label, neighbor_index) in [
                    ("E", (x + 1 < width).then_some(index + 1)),
                    ("S", (z + 1 < height).then_some(index + width)),
                ] {
                    let Some(neighbor_index) = neighbor_index else {
                        continue;
                    };
                    let left_sample = macro_tile.samples[index];
                    let right_sample = macro_tile.samples[neighbor_index];
                    if left_sample.nearest_site == right_sample.nearest_site {
                        continue;
                    }
                    let left_base = base_surface_plan.columns[index];
                    let right_base = base_surface_plan.columns[neighbor_index];
                    let left_final = surface_plan.columns[index];
                    let right_final = surface_plan.columns[neighbor_index];
                    *owner_boundary_pair_counts
                        .entry(format!(
                            "{}:{}->{}/{}->{}",
                            label,
                            left_base.top_block,
                            left_final.top_block,
                            right_base.top_block,
                            right_final.top_block
                        ))
                        .or_default() += 1;
                    if left_base.top_block != right_base.top_block
                        && left_final.top_block == right_base.top_block
                        && right_final.top_block == left_base.top_block
                    {
                        owner_boundary_swap_count += 1;
                        if owner_boundary_swap_representatives.len() < 24 {
                            owner_boundary_swap_representatives.push(format!(
                                "{label} left#{} world=({}, {}) site={:?} biome={:?} base={} final={} | right#{} world=({}, {}) site={:?} biome={:?} base={} final={}",
                                index,
                                left_final.world_x,
                                left_final.world_z,
                                left_sample.nearest_site,
                                left_sample.biome,
                                left_base.top_block,
                                left_final.top_block,
                                neighbor_index,
                                right_final.world_x,
                                right_final.world_z,
                                right_sample.nearest_site,
                                right_sample.biome,
                                right_base.top_block,
                                right_final.top_block,
                            ));
                        }
                    }
                }
            }
        }

        eprintln!(
            "surface contract audit seed={seed} center_chunk=({center_chunk_x},{center_chunk_z}) radius={chunk_radius} columns={} footprint=x:{}..{},z:{}..{} perlin=true",
            surface_plan.columns.len(),
            min_world_x,
            max_world_x_exclusive,
            min_world_z,
            max_world_z_exclusive
        );
        eprintln!("top_counts={top_counts:?}");
        eprintln!("role_counts={role_counts:?}");
        eprintln!("biome_counts={biome_counts:?}");
        eprintln!("site_material_family_count={}", site_biomes.len());
        for (key, row) in representative_rows.iter().take(24) {
            eprintln!("representative {key}: {row}");
        }
        eprintln!("missing_owner_biome_count={missing_owner_biome_count}");
        for representative in &missing_owner_biome_representatives {
            eprintln!("missing_owner_biome {representative}");
        }
        eprintln!("owner_biome_mismatch_count={owner_biome_mismatch_count}");
        for representative in &owner_biome_mismatch_representatives {
            eprintln!("owner_biome_mismatch {representative}");
        }
        eprintln!("raw_owner_switch_count={raw_owner_switch_count}");
        eprintln!("unsupported_raw_owner_switch_count={unsupported_raw_owner_switch_count}");
        for representative in &raw_owner_switch_representatives {
            eprintln!("unsupported_raw_owner_switch {representative}");
        }
        eprintln!("local_mix_count={local_mix_count}");
        eprintln!("local_mix_pair_counts={local_mix_pair_counts:?}");
        eprintln!("unsupported_local_mix_count={unsupported_local_mix_count}");
        for representative in &unsupported_local_mix_representatives {
            eprintln!("unsupported_local_mix {representative}");
        }
        eprintln!("owner_boundary_pair_counts={owner_boundary_pair_counts:?}");
        eprintln!("owner_boundary_swap_count={owner_boundary_swap_count}");
        for representative in &owner_boundary_swap_representatives {
            eprintln!("owner_boundary_swap {representative}");
        }
        let final_owner_top_audit =
            audit_final_non_water_tops_by_owner(&macro_tile.samples, &surface_plan.columns);
        eprintln!(
            "final_non_water_owner_top_audit checked_columns={} owner_count={} conflicting_owner_count={}",
            final_owner_top_audit.checked_column_count,
            final_owner_top_audit.owner_count,
            final_owner_top_audit.conflicting_owner_count
        );
        for representative in &final_owner_top_audit.owner_count_representatives {
            eprintln!("final_non_water_owner_top_count {representative}");
        }
        for representative in &final_owner_top_audit.conflict_representatives {
            eprintln!("final_non_water_owner_top_conflict {representative}");
        }
        eprintln!("anomaly_count={anomaly_count}");
        for representative in &representatives {
            eprintln!("anomaly {representative}");
        }

        assert_eq!(
            missing_owner_biome_count, 0,
            "owner samples must preserve biome/context handoff"
        );
        assert_eq!(
            owner_biome_mismatch_count, 0,
            "biome/context handoff must follow the canonical noisy boundary owner site"
        );
        assert_eq!(
            unsupported_raw_owner_switch_count, 0,
            "raw owner may change only inside the canonical noisy boundary owner band for that raw/owner site pair"
        );
        assert_eq!(
            unsupported_local_mix_count, 0,
            "final surface material may differ from the noisy-owner base resolve only through the bounded local material mix pass"
        );
        assert_eq!(
            owner_boundary_swap_count, 0,
            "adjacent noisy-owner cells must not mutually exchange top materials across their shared boundary"
        );
        assert_eq!(
            final_owner_top_audit.conflicting_owner_count, 0,
            "final non-water top materials must stay singular within each noisy-boundary owner"
        );
        assert_eq!(
            anomaly_count, 0,
            "unexpected top material outside documented owner-biome plus hydrology policy"
        );
    }

    #[test]
    #[ignore = "focused diagnostic audit for seed 42 chunk (-73,-40) material boundaries"]
    fn diagnose_seed42_chunk_neg73_neg40_surface_material_boundary() {
        let seed = 42;
        let chunk_x = -73;
        let chunk_z = -40;
        let meta = WorldMeta::new(seed);
        let guard_blocks = 1;
        let min_world_x = chunk_x * CHUNK_EDGE_I32 - guard_blocks;
        let min_world_z = chunk_z * CHUNK_EDGE_I32 - guard_blocks;
        let columns_x = (CHUNK_EDGE_I32 + guard_blocks * 2) as u32;
        let columns_z = columns_x;
        let center_world_x = min_world_x + columns_x as i32 / 2;
        let center_world_z = min_world_z + columns_z as i32 / 2;
        let max_world_x_exclusive = min_world_x + columns_x as i32;
        let max_world_z_exclusive = min_world_z + columns_z as i32;
        let graph_area = GraphRegionArea::new(
            graph_region_for_world_block(
                min_world_x,
                min_world_z,
                DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            ),
            graph_region_for_world_block(
                max_world_x_exclusive,
                max_world_z_exclusive,
                DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            ),
        )
        .expect("valid focused audit graph area");
        let center_region = graph_region_for_world_block(
            center_world_x,
            center_world_z,
            DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
        );
        let padding_regions = required_audit_padding_regions(center_region, graph_area);
        let patch = generate_voronoi_graph_patch(VoronoiGraphPatchRequest::new(
            VoronoiGraphConfig {
                seed: meta.seed,
                generator_version: meta.generator_version,
                region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
                site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
                padding_regions,
            },
            center_world_x,
            center_world_z,
        ));
        let mut macro_map = generate_macro_map(
            &patch,
            MacroMapConfig {
                land_bias: MacroMapConfig::new(meta.seed, meta.generator_version).land_bias,
                ..MacroMapConfig::new(meta.seed, meta.generator_version)
            },
        );
        let hydrology = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
        apply_headwater_source_hydration_to_biomes(&patch, &mut macro_map, &hydrology);
        let boundary = generate_noisy_boundaries(
            &patch,
            &macro_map,
            BoundaryConfig::new(meta.seed, meta.generator_version),
        );
        let river_plan = build_river_plan(&patch, &macro_map, &hydrology, Default::default());
        let macro_tile = generate_macro_field_tile(
            &patch,
            &macro_map,
            &river_plan,
            &boundary,
            MacroFieldTileConfig::new(
                min_world_x as f32 + 0.5,
                min_world_z as f32 + 0.5,
                columns_x,
                columns_z,
                1.0,
            ),
        );
        let raw_context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let heightfield = generate_heightfield_tile(
            &macro_tile,
            HeightfieldConfig {
                perlin: HeightfieldPerlinConfig::preview_enabled(meta.seed, meta.generator_version),
                ..HeightfieldConfig::default()
            },
        );
        let surface_config = SurfacePlanConfig::default();
        let base_surface_config = SurfacePlanConfig {
            boundary_mix_radius_blocks: 0,
            ..surface_config
        };
        let base_surface_plan =
            generate_surface_plan_area(&heightfield, Some(&macro_tile), base_surface_config);
        let surface_plan =
            generate_surface_plan_area(&heightfield, Some(&macro_tile), surface_config);
        let surface_mix_sources = macro_tile
            .samples
            .iter()
            .map(|sample| SurfaceMixSource {
                owner_site: sample.nearest_site,
            })
            .collect::<Vec<_>>();

        let mut raw_owner_switch_count = 0usize;
        let mut unsupported_raw_owner_switch_count = 0usize;
        let mut local_mix_count = 0usize;
        let mut unsupported_local_mix_count = 0usize;
        let mut base_top_counts = BTreeMap::<&'static str, usize>::new();
        let mut final_top_counts = BTreeMap::<&'static str, usize>::new();
        let mut local_mix_rows = Vec::new();
        let mut owner_switch_rows = Vec::new();
        let mut owner_boundary_swap_count = 0usize;
        let mut owner_boundary_swap_rows = Vec::new();

        for z in guard_blocks..guard_blocks + CHUNK_EDGE_I32 {
            for x in guard_blocks..guard_blocks + CHUNK_EDGE_I32 {
                let index = z as usize * columns_x as usize + x as usize;
                let sample = macro_tile.samples[index];
                let height = heightfield.columns[index];
                let base_plan = base_surface_plan.columns[index];
                let plan = surface_plan.columns[index];
                *base_top_counts.entry(base_plan.top_block).or_default() += 1;
                *final_top_counts.entry(plan.top_block).or_default() += 1;

                let raw_site = raw_context
                    .nearest_site(sample.position)
                    .map(|site| site.id);
                if raw_site != sample.nearest_site {
                    raw_owner_switch_count += 1;
                    let allowance = raw_owner_switch_allowance(
                        &boundary.curves,
                        raw_site,
                        sample.nearest_site,
                        sample.position,
                        macro_tile.config.sample_spacing_blocks,
                    );
                    if !allowance.is_some_and(|(_, _, within)| within) {
                        unsupported_raw_owner_switch_count += 1;
                    }
                    if owner_switch_rows.len() < 24 {
                        owner_switch_rows.push(format!(
                            "world=({}, {}) local=({}, {}) raw={:?} noisy_owner={:?} biome={:?} base_top={} final_top={} allowance={:?}",
                            plan.world_x,
                            plan.world_z,
                            x - guard_blocks,
                            z - guard_blocks,
                            raw_site,
                            sample.nearest_site,
                            sample.biome,
                            base_plan.top_block,
                            plan.top_block,
                            allowance,
                        ));
                    }
                }

                if plan.top_block != base_plan.top_block {
                    local_mix_count += 1;
                    let source = supported_local_mix_source(
                        &base_surface_plan.columns,
                        base_surface_plan.width as usize,
                        base_surface_plan.height as usize,
                        index,
                        base_plan,
                        plan.top_block,
                        Some(&surface_mix_sources),
                        surface_config,
                    );
                    if source.is_none() {
                        unsupported_local_mix_count += 1;
                    }
                    if local_mix_rows.len() < 24 {
                        let source_text = source.map_or_else(
                            || "source=None".to_string(),
                            |(source_index, source_plan)| {
                                let source_sample = macro_tile.samples[source_index];
                                format!(
                                    "source=world=({}, {}) site={:?} biome={:?} top={}",
                                    source_plan.world_x,
                                    source_plan.world_z,
                                    source_sample.nearest_site,
                                    source_sample.biome,
                                    source_plan.top_block,
                                )
                            },
                        );
                        local_mix_rows.push(format!(
                            "world=({}, {}) local=({}, {}) site={:?} biome={:?} role={:?} base_top={} final_top={} {} river(core={:.3},valley={:.3},flow={:.3})",
                            plan.world_x,
                            plan.world_z,
                            x - guard_blocks,
                            z - guard_blocks,
                            sample.nearest_site,
                            sample.biome,
                            plan.hydrology_role,
                            base_plan.top_block,
                            plan.top_block,
                            source_text,
                            height.river_core_strength,
                            height.river_valley_strength,
                            height.river_flow_hint,
                        ));
                    }
                }
            }
        }

        for z in guard_blocks..guard_blocks + CHUNK_EDGE_I32 {
            for x in guard_blocks..guard_blocks + CHUNK_EDGE_I32 {
                let index = z as usize * columns_x as usize + x as usize;
                for (label, neighbor_index) in [
                    (
                        "E",
                        (x + 1 < guard_blocks + CHUNK_EDGE_I32).then_some(index + 1),
                    ),
                    (
                        "S",
                        (z + 1 < guard_blocks + CHUNK_EDGE_I32)
                            .then_some(index + columns_x as usize),
                    ),
                ] {
                    let Some(neighbor_index) = neighbor_index else {
                        continue;
                    };
                    if !is_cross_owner_boundary_swap(
                        &surface_plan.columns,
                        &base_surface_plan.columns,
                        &surface_mix_sources,
                        index,
                        neighbor_index,
                    ) {
                        continue;
                    }

                    owner_boundary_swap_count += 1;
                    if owner_boundary_swap_rows.len() < 24 {
                        let left_sample = macro_tile.samples[index];
                        let right_sample = macro_tile.samples[neighbor_index];
                        let left_base = base_surface_plan.columns[index];
                        let right_base = base_surface_plan.columns[neighbor_index];
                        let left_final = surface_plan.columns[index];
                        let right_final = surface_plan.columns[neighbor_index];
                        owner_boundary_swap_rows.push(format!(
                            "{label} left=world=({}, {}) local=({}, {}) site={:?} biome={:?} base={} final={} | right=world=({}, {}) site={:?} biome={:?} base={} final={}",
                            left_final.world_x,
                            left_final.world_z,
                            x - guard_blocks,
                            z - guard_blocks,
                            left_sample.nearest_site,
                            left_sample.biome,
                            left_base.top_block,
                            left_final.top_block,
                            right_final.world_x,
                            right_final.world_z,
                            right_sample.nearest_site,
                            right_sample.biome,
                            right_base.top_block,
                            right_final.top_block,
                        ));
                    }
                }
            }
        }

        eprintln!(
            "focused surface audit seed={seed} chunk=({chunk_x},{chunk_z}) columns={} guarded_footprint=x:{}..{},z:{}..{}",
            CHUNK_EDGE_I32 * CHUNK_EDGE_I32,
            min_world_x,
            max_world_x_exclusive,
            min_world_z,
            max_world_z_exclusive
        );
        eprintln!("base_top_counts={base_top_counts:?}");
        eprintln!("final_top_counts={final_top_counts:?}");
        eprintln!("raw_owner_switch_count={raw_owner_switch_count}");
        eprintln!("unsupported_raw_owner_switch_count={unsupported_raw_owner_switch_count}");
        for row in &owner_switch_rows {
            eprintln!("owner_switch {row}");
        }
        eprintln!("local_mix_count={local_mix_count}");
        eprintln!("unsupported_local_mix_count={unsupported_local_mix_count}");
        for row in &local_mix_rows {
            eprintln!("local_mix {row}");
        }
        eprintln!("owner_boundary_swap_count={owner_boundary_swap_count}");
        for row in &owner_boundary_swap_rows {
            eprintln!("owner_boundary_swap {row}");
        }
        let final_owner_top_audit =
            audit_final_non_water_tops_by_owner(&macro_tile.samples, &surface_plan.columns);
        eprintln!(
            "final_non_water_owner_top_audit checked_columns={} owner_count={} conflicting_owner_count={}",
            final_owner_top_audit.checked_column_count,
            final_owner_top_audit.owner_count,
            final_owner_top_audit.conflicting_owner_count
        );
        for row in &final_owner_top_audit.owner_count_representatives {
            eprintln!("final_non_water_owner_top_count {row}");
        }
        for row in &final_owner_top_audit.conflict_representatives {
            eprintln!("final_non_water_owner_top_conflict {row}");
        }

        assert_eq!(unsupported_raw_owner_switch_count, 0);
        assert_eq!(unsupported_local_mix_count, 0);
        assert_eq!(owner_boundary_swap_count, 0);
        assert_eq!(
            final_owner_top_audit.conflicting_owner_count, 0,
            "final non-water top materials must stay singular within each noisy-boundary owner"
        );
    }

    #[derive(Debug, Default)]
    struct FinalNonWaterOwnerTopAudit {
        checked_column_count: usize,
        owner_count: usize,
        conflicting_owner_count: usize,
        owner_count_representatives: Vec<String>,
        conflict_representatives: Vec<String>,
    }

    fn audit_final_non_water_tops_by_owner(
        samples: &[MacroFieldSample],
        columns: &[SurfaceColumnPlan],
    ) -> FinalNonWaterOwnerTopAudit {
        let mut counts_by_site = BTreeMap::<String, BTreeMap<&'static str, usize>>::new();
        let mut rows_by_site = BTreeMap::<String, Vec<String>>::new();
        let mut checked_column_count = 0usize;

        for (index, (sample, column)) in samples.iter().zip(columns).enumerate() {
            if !is_non_water_owner_surface_column(*column) {
                continue;
            }
            let Some(site) = sample.nearest_site else {
                continue;
            };

            checked_column_count += 1;
            let site_key = format!("{site:?}");
            *counts_by_site
                .entry(site_key.clone())
                .or_default()
                .entry(column.top_block)
                .or_default() += 1;
            rows_by_site.entry(site_key).or_default().push(format!(
                "#{index} world=({}, {}) site={:?} role={:?} water_y={:?} biome={:?} final_top={}",
                column.world_x,
                column.world_z,
                sample.nearest_site,
                column.hydrology_role,
                column.water_y,
                sample.biome,
                column.top_block
            ));
        }

        let owner_count_representatives = counts_by_site
            .iter()
            .take(32)
            .map(|(site, counts)| format!("site={site} final_counts={counts:?}"))
            .collect();
        let mut conflict_representatives = Vec::new();
        for (site, counts) in &counts_by_site {
            if counts.len() <= 1 {
                continue;
            }
            if conflict_representatives.len() < 24 {
                let rows = rows_by_site
                    .get(site)
                    .map(|rows| rows.iter().take(6).cloned().collect::<Vec<_>>().join(" | "))
                    .unwrap_or_default();
                conflict_representatives.push(format!(
                    "site={site} final_counts={counts:?} examples={rows}"
                ));
            }
        }

        FinalNonWaterOwnerTopAudit {
            checked_column_count,
            owner_count: counts_by_site.len(),
            conflicting_owner_count: counts_by_site
                .values()
                .filter(|counts| counts.len() > 1)
                .count(),
            owner_count_representatives,
            conflict_representatives,
        }
    }

    fn is_supported_local_mix(
        base_columns: &[SurfaceColumnPlan],
        width: usize,
        height: usize,
        index: usize,
        current: SurfaceColumnPlan,
        final_top_block: &'static str,
        mix_sources: Option<&[SurfaceMixSource]>,
        config: SurfacePlanConfig,
    ) -> bool {
        let radius = usize::from(config.boundary_mix_radius_blocks);
        if radius == 0 || width == 0 || height == 0 {
            return false;
        }
        let x = index % width;
        let z = index / width;
        let mix_sources = mix_sources.filter(|sources| sources.len() == base_columns.len());
        let current_source = mix_sources.and_then(|sources| sources.get(index).copied());
        let min_x = x.saturating_sub(radius);
        let max_x = (x + radius).min(width.saturating_sub(1));
        let min_z = z.saturating_sub(radius);
        let max_z = (z + radius).min(height.saturating_sub(1));

        for nz in min_z..=max_z {
            for nx in min_x..=max_x {
                if nx == x && nz == z {
                    continue;
                }
                let dx = nx.abs_diff(x);
                let dz = nz.abs_diff(z);
                let distance = dx + dz;
                if distance == 0 || distance > radius {
                    continue;
                }
                let neighbor_index = nz * width + nx;
                let neighbor = base_columns[neighbor_index];
                let neighbor_source =
                    mix_sources.and_then(|sources| sources.get(neighbor_index).copied());
                if neighbor.top_block == final_top_block
                    && boundary_mix_direction_allows(current, neighbor, config)
                    && mix_candidate_allowed_with_sources(
                        current,
                        neighbor,
                        current_source,
                        neighbor_source,
                    )
                {
                    return true;
                }
            }
        }

        false
    }

    fn supported_local_mix_source(
        base_columns: &[SurfaceColumnPlan],
        width: usize,
        height: usize,
        index: usize,
        current: SurfaceColumnPlan,
        final_top_block: &'static str,
        mix_sources: Option<&[SurfaceMixSource]>,
        config: SurfacePlanConfig,
    ) -> Option<(usize, SurfaceColumnPlan)> {
        let radius = usize::from(config.boundary_mix_radius_blocks);
        if radius == 0 || width == 0 || height == 0 {
            return None;
        }
        let x = index % width;
        let z = index / width;
        let mix_sources = mix_sources.filter(|sources| sources.len() == base_columns.len());
        let current_source = mix_sources.and_then(|sources| sources.get(index).copied());
        let min_x = x.saturating_sub(radius);
        let max_x = (x + radius).min(width.saturating_sub(1));
        let min_z = z.saturating_sub(radius);
        let max_z = (z + radius).min(height.saturating_sub(1));

        for nz in min_z..=max_z {
            for nx in min_x..=max_x {
                if nx == x && nz == z {
                    continue;
                }
                let dx = nx.abs_diff(x);
                let dz = nz.abs_diff(z);
                let distance = dx + dz;
                if distance == 0 || distance > radius {
                    continue;
                }
                let neighbor_index = nz * width + nx;
                let neighbor = base_columns[neighbor_index];
                let neighbor_source =
                    mix_sources.and_then(|sources| sources.get(neighbor_index).copied());
                if neighbor.top_block == final_top_block
                    && boundary_mix_direction_allows(current, neighbor, config)
                    && mix_candidate_allowed_with_sources(
                        current,
                        neighbor,
                        current_source,
                        neighbor_source,
                    )
                {
                    return Some((neighbor_index, neighbor));
                }
            }
        }

        None
    }

    fn raw_owner_switch_allowance(
        curves: &[NoisyBoundaryCurve],
        raw_site: Option<VoronoiSiteId>,
        owner_site: Option<VoronoiSiteId>,
        position: WorldPlanePoint,
        sample_spacing_blocks: f32,
    ) -> Option<(f32, f32, bool)> {
        let raw_site = raw_site?;
        let owner_site = owner_site?;
        if raw_site == owner_site {
            return Some((0.0, 0.0, true));
        }

        curves
            .iter()
            .filter(|curve| {
                curve.anchors.sites.contains(&raw_site) && curve.anchors.sites.contains(&owner_site)
            })
            .map(|curve| {
                let distance = test_polyline_distance(position, &curve.points);
                let radius = curve.amplitude + sample_spacing_blocks * 0.5;
                (distance, radius, distance <= radius + 0.001)
            })
            .min_by(|left, right| left.0.total_cmp(&right.0))
    }

    fn test_polyline_distance(position: WorldPlanePoint, points: &[WorldPlanePoint]) -> f32 {
        match points {
            [] => f32::INFINITY,
            [point] => test_squared_distance(position, *point).sqrt(),
            _ => points
                .windows(2)
                .map(|segment| {
                    test_point_segment_distance_squared(position, segment[0], segment[1])
                })
                .fold(f32::INFINITY, f32::min)
                .sqrt(),
        }
    }

    fn test_point_segment_distance_squared(
        point: WorldPlanePoint,
        start: WorldPlanePoint,
        end: WorldPlanePoint,
    ) -> f32 {
        let dx = end.x - start.x;
        let dz = end.z - start.z;
        let len2 = dx * dx + dz * dz;
        let t = if len2 <= f32::EPSILON {
            0.0
        } else {
            (((point.x - start.x) * dx + (point.z - start.z) * dz) / len2).clamp(0.0, 1.0)
        };
        let projected = WorldPlanePoint::new(start.x + dx * t, start.z + dz * t);
        test_squared_distance(point, projected)
    }

    fn test_squared_distance(left: WorldPlanePoint, right: WorldPlanePoint) -> f32 {
        let dx = left.x - right.x;
        let dz = left.z - right.z;
        dx * dx + dz * dz
    }

    fn documented_allowed_tops(
        input: SurfaceColumnInput,
        config: SurfacePlanConfig,
    ) -> BTreeSet<&'static str> {
        let biome = input.biome.unwrap_or(GraphBiomeKind::TemperateGrassland);
        let policy = biome_surface_policy(biome);
        let role = hydrology_role(&input, config);
        let variant = material_variant(input.world_x, input.world_z, config, policy.family);
        let mut allowed = BTreeSet::new();
        match role {
            SurfaceHydrologyRole::Ocean => {
                allowed.insert(policy.sediment);
                allowed.insert("gravel");
            }
            SurfaceHydrologyRole::Lake => {
                allowed.extend(["gravel", "mud", "silt"]);
            }
            SurfaceHydrologyRole::River => {
                allowed.extend(["wet_gravel", "mud", "silt"]);
            }
            SurfaceHydrologyRole::Wetland => {
                allowed.insert(policy.wet_top);
                allowed.extend(["mud", "peat"]);
            }
            SurfaceHydrologyRole::Coast => {
                allowed.extend(["sand", "wet_sand"]);
                if matches!(
                    biome,
                    GraphBiomeKind::Mangrove
                        | GraphBiomeKind::EstuarineCoast
                        | GraphBiomeKind::LagoonCoast
                ) && should_apply_continuous_coast_mudflat(&input, config)
                {
                    allowed.extend(["mud", "silt"]);
                }
                if local_river_mouth_sediment(input.heightfield, config, variant).is_some() {
                    allowed.extend(["mud", "silt"]);
                }
            }
            SurfaceHydrologyRole::DryBasin => {
                allowed.extend(["clay", "silt"]);
                allowed.insert(policy.dry_top);
            }
            SurfaceHydrologyRole::Ridge | SurfaceHydrologyRole::Land => {
                allowed.insert(policy.default_top);
                if let Some(context) = input.biome_context {
                    if context.hydration > 0.66 {
                        allowed.insert(policy.wet_top);
                    }
                    if context.hydration < 0.34 {
                        allowed.insert(policy.dry_top);
                    }
                }
                if local_dry_river_bank_sediment(input.heightfield, policy, config, variant)
                    .is_some()
                {
                    allowed.insert("gravel");
                    allowed.insert(policy.sediment);
                }
            }
        }
        allowed
    }

    fn required_audit_padding_regions(
        center: crate::world::generation::GraphRegionCoord,
        area: GraphRegionArea,
    ) -> u32 {
        let dx = (center.x - area.min.x)
            .abs()
            .max((area.max.x - center.x).abs());
        let dz = (center.z - area.min.z)
            .abs()
            .max((area.max.z - center.z).abs());
        u32::try_from(dx.max(dz).saturating_add(1)).expect("audit padding fits u32")
    }

    #[test]
    fn every_biome_policy_uses_known_block_keys() {
        for biome in ALL_BIOMES {
            let policy = biome_surface_policy(*biome);
            assert_eq!(policy.biome, *biome);
            assert_known(policy.default_top);
            assert_known(policy.dry_top);
            assert_known(policy.wet_top);
            assert_known(policy.frozen_top);
            assert_known(policy.subsurface);
            assert_known(policy.base);
            assert_known(policy.exposed);
            assert_known(policy.sediment);
        }
    }

    #[test]
    fn surface_resolve_preserves_surface_and_water_height() {
        let mut heightfield = column(10.0, 20.0, HeightfieldTerrainKind::River);
        heightfield.surface_y = 42;
        heightfield.water_y = Some(45);
        let plan = resolve_surface_column(
            input(heightfield, Some(GraphBiomeKind::TemperateGrassland), None),
            SurfacePlanConfig::new(12, 3),
        );

        assert_eq!(plan.surface_y, 42);
        assert_eq!(plan.water_y, Some(45));
    }

    #[test]
    fn ocean_lake_and_river_roles_resolve_to_distinct_materials() {
        let ocean = resolve_surface_column(
            input(
                column(0.0, 0.0, HeightfieldTerrainKind::Ocean),
                Some(GraphBiomeKind::ShallowOcean),
                None,
            ),
            SurfacePlanConfig::default(),
        );
        let lake = resolve_surface_column(
            input(
                column(1.0, 0.0, HeightfieldTerrainKind::Lake),
                Some(GraphBiomeKind::Lake),
                None,
            ),
            SurfacePlanConfig::default(),
        );
        let river = resolve_surface_column(
            input(
                column(2.0, 0.0, HeightfieldTerrainKind::River),
                Some(GraphBiomeKind::TemperateGrassland),
                None,
            ),
            SurfacePlanConfig::default(),
        );

        assert_eq!(ocean.hydrology_role, SurfaceHydrologyRole::Ocean);
        assert_eq!(lake.hydrology_role, SurfaceHydrologyRole::Lake);
        assert_eq!(river.hydrology_role, SurfaceHydrologyRole::River);
        assert_ne!(ocean.hydrology_role, lake.hydrology_role);
        assert_ne!(lake.hydrology_role, river.hydrology_role);
    }

    #[test]
    fn dry_land_with_moderate_river_valley_hint_keeps_biome_top() {
        let config = SurfacePlanConfig {
            boundary_mix_radius_blocks: 0,
            ..SurfacePlanConfig::default()
        };
        let mut heightfield = column(42.0, -17.0, HeightfieldTerrainKind::Land);
        heightfield.river_valley_strength = config.river_water_threshold * 0.60;
        heightfield.river_flow_hint = 0.72;

        let plan = resolve_surface_column(
            input(heightfield, Some(GraphBiomeKind::TemperateGrassland), None),
            config,
        );

        assert_eq!(plan.hydrology_role, SurfaceHydrologyRole::Land);
        assert_eq!(plan.top_block, "grass");
        assert_eq!(plan.sediment_block, "gravel");
    }

    #[test]
    fn coast_mask_alone_does_not_turn_land_into_sand() {
        let config = SurfacePlanConfig {
            boundary_mix_radius_blocks: 0,
            ..SurfacePlanConfig::default()
        };
        let mut heightfield = column(11.0, -9.0, HeightfieldTerrainKind::Land);
        heightfield.coast_mask = 1.0;

        let plan = resolve_surface_column(
            input(heightfield, Some(GraphBiomeKind::TemperateGrassland), None),
            config,
        );

        assert_eq!(plan.hydrology_role, SurfaceHydrologyRole::Land);
        assert_eq!(
            plan.top_block, "grass",
            "coast distance is diagnostic unless upstream biome/context says this column is coast"
        );
    }

    #[test]
    fn active_river_core_still_resolves_river_sediment() {
        let mut heightfield = column(8.0, 3.0, HeightfieldTerrainKind::Land);
        heightfield.river_valley_strength = SurfacePlanConfig::default().river_water_threshold;
        heightfield.river_flow_hint = 0.72;
        heightfield.river_bed_depth_blocks = 4.0;

        let plan = resolve_surface_column(
            input(heightfield, Some(GraphBiomeKind::TemperateGrassland), None),
            SurfacePlanConfig::default(),
        );

        assert_eq!(plan.hydrology_role, SurfaceHydrologyRole::River);
        assert!(matches!(plan.top_block, "wet_gravel" | "mud" | "silt"));
        assert!(matches!(plan.sediment_block, "gravel" | "silt"));
        assert!(!plan.vegetation_allowed);
    }

    #[test]
    fn dry_estuary_mouth_sediment_is_local_not_solid_patch() {
        let config = SurfacePlanConfig {
            boundary_mix_radius_blocks: 0,
            ..SurfacePlanConfig::new(41, 5)
        };
        let mut sediment_count = 0;
        let mut ordinary_count = 0;

        for x in 0..128 {
            let mut heightfield = column(x as f32, 5.0, HeightfieldTerrainKind::Land);
            heightfield.coast_mask = config.coast_threshold;
            heightfield.river_valley_strength = config.river_water_threshold * 0.74;
            heightfield.river_flow_hint = 0.44;
            heightfield.river_bed_depth_blocks = 3.0;
            let plan = resolve_surface_column(
                input(heightfield, Some(GraphBiomeKind::EstuarineCoast), None),
                config,
            );

            assert_eq!(plan.hydrology_role, SurfaceHydrologyRole::Coast);
            if matches!(plan.top_block, "mud" | "silt") {
                sediment_count += 1;
            } else if matches!(plan.top_block, "sand" | "wet_sand") {
                ordinary_count += 1;
            }
        }

        assert!(
            sediment_count > 0,
            "selected mouth hints should still produce some local sediment"
        );
        assert!(
            ordinary_count > 0,
            "dry mouth shoulders should not become a solid mud/silt patch"
        );
    }

    #[test]
    fn deterministic_variation_returns_same_result_for_same_input() {
        let heightfield = column(37.0, -91.0, HeightfieldTerrainKind::Coast);
        let input = input(heightfield, Some(GraphBiomeKind::SandyCoast), None);
        let config = SurfacePlanConfig::new(99, 7);

        assert_eq!(
            resolve_surface_column(input, config),
            resolve_surface_column(input, config)
        );
    }

    #[test]
    fn generate_surface_plan_area_preserves_row_major_column_count_and_order() {
        let heightfield = HeightfieldTile {
            width: 2,
            height: 2,
            sample_spacing_blocks: 1.0,
            columns: vec![
                column(0.0, 0.0, HeightfieldTerrainKind::Land),
                column(1.0, 0.0, HeightfieldTerrainKind::Land),
                column(0.0, 1.0, HeightfieldTerrainKind::Land),
                column(1.0, 1.0, HeightfieldTerrainKind::Land),
            ],
            stats: Default::default(),
            config: Default::default(),
        };
        let macro_field = MacroFieldTile {
            config: MacroFieldTileConfig::new(0.0, 0.0, 2, 2, 1.0),
            samples: vec![
                sample(0.0, 0.0, GraphBiomeKind::Desert),
                sample(1.0, 0.0, GraphBiomeKind::Lake),
                sample(0.0, 1.0, GraphBiomeKind::TemperateGrassland),
                sample(1.0, 1.0, GraphBiomeKind::RockyCoast),
            ],
            stats: MacroFieldTileStats::default(),
        };

        let area = generate_surface_plan_area(
            &heightfield,
            Some(&macro_field),
            SurfacePlanConfig {
                boundary_mix_radius_blocks: 0,
                ..SurfacePlanConfig::default()
            },
        );

        assert_eq!(area.columns.len(), 4);
        assert_eq!(area.stats.column_count, 4);
        assert_eq!(area.columns[0].world_x, 0);
        assert_eq!(area.columns[0].world_z, 0);
        assert_eq!(area.columns[1].world_x, 1);
        assert_eq!(area.columns[1].world_z, 0);
        assert_eq!(area.columns[2].world_x, 0);
        assert_eq!(area.columns[2].world_z, 1);
        assert_eq!(area.columns[3].world_x, 1);
        assert_eq!(area.columns[3].world_z, 1);
        assert_eq!(area.columns[0].top_block, "sand");
        assert_eq!(area.columns[1].hydrology_role, SurfaceHydrologyRole::Lake);
    }

    #[test]
    fn generate_surface_plan_area_normalizes_non_water_top_within_owner_site() {
        let heightfield = heightfield_tile(
            2,
            1,
            vec![
                column(-2322.0, -1280.0, HeightfieldTerrainKind::Coast),
                column(-2321.0, -1280.0, HeightfieldTerrainKind::Coast),
            ],
        );
        let mut samples = vec![
            sample(-2322.0, -1280.0, GraphBiomeKind::SandyCoast),
            sample(-2321.0, -1280.0, GraphBiomeKind::SandyCoast),
        ];
        for sample in &mut samples {
            sample.nearest_site = Some(VoronoiSiteId(107374182413));
            sample.biome_context = sample.biome_context.map(|mut context| {
                context.water_role = GraphBiomeWaterRole::Coast;
                context
            });
        }
        let macro_field = MacroFieldTile {
            config: MacroFieldTileConfig::new(-2322.0, -1280.0, 2, 1, 1.0),
            samples,
            stats: MacroFieldTileStats::default(),
        };

        let area = generate_surface_plan_area(
            &heightfield,
            Some(&macro_field),
            SurfacePlanConfig::default(),
        );
        let audit = audit_final_non_water_tops_by_owner(&macro_field.samples, &area.columns);

        assert_eq!(area.columns[0].top_block, "sand");
        assert_eq!(area.columns[1].top_block, "sand");
        assert_eq!(audit.conflicting_owner_count, 0);
    }

    #[test]
    fn generate_surface_plan_area_keeps_noisy_owner_material_boundaries() {
        let heightfield = heightfield_tile(
            2,
            1,
            vec![
                column(-2322.0, -1280.0, HeightfieldTerrainKind::Land),
                column(-2321.0, -1280.0, HeightfieldTerrainKind::Land),
            ],
        );
        let mut coast_owner = sample(-2322.0, -1280.0, GraphBiomeKind::SandyCoast);
        coast_owner.nearest_site = Some(VoronoiSiteId(107374182413));
        coast_owner.biome_context = coast_owner.biome_context.map(|mut context| {
            context.water_role = GraphBiomeWaterRole::Coast;
            context
        });
        let mut forest_owner = sample(-2321.0, -1280.0, GraphBiomeKind::TemperateMixedForest);
        forest_owner.nearest_site = Some(VoronoiSiteId(98784247821));
        let samples = vec![coast_owner, forest_owner];
        let macro_field = MacroFieldTile {
            config: MacroFieldTileConfig::new(-2322.0, -1280.0, 2, 1, 1.0),
            samples,
            stats: MacroFieldTileStats::default(),
        };

        let area = generate_surface_plan_area(
            &heightfield,
            Some(&macro_field),
            SurfacePlanConfig::default(),
        );
        let audit = audit_final_non_water_tops_by_owner(&macro_field.samples, &area.columns);

        assert_eq!(area.columns[0].top_block, "sand");
        assert_eq!(area.columns[1].top_block, "grass");
        assert_eq!(audit.checked_column_count, 2);
        assert_eq!(audit.owner_count, 2);
        assert_eq!(audit.conflicting_owner_count, 0);
    }

    #[test]
    fn rocky_coast_and_rugged_scalars_do_not_force_gravel() {
        let mut heightfield = column(1.0, 1.0, HeightfieldTerrainKind::Coast);
        heightfield.terrain_ruggedness = 0.95;
        heightfield.ridge_influence = 0.85;

        let plan = resolve_surface_column(
            input(heightfield, Some(GraphBiomeKind::RockyCoast), None),
            SurfacePlanConfig::new(23, 4),
        );

        assert_eq!(plan.hydrology_role, SurfaceHydrologyRole::Coast);
        assert!(matches!(plan.top_block, "sand" | "wet_sand"));
        assert!(
            !matches!(
                plan.top_block,
                "gravel" | "wet_gravel" | "rock" | "exposed_rock"
            ),
            "coast/rugged/RockyCoast inputs should not force a rocky material"
        );
    }

    #[test]
    fn rugged_land_and_ridge_inputs_keep_biome_top() {
        let mut land = column(3.0, 7.0, HeightfieldTerrainKind::Land);
        land.terrain_ruggedness = 0.95;
        land.river_bank_roughness_hint = 0.95;

        let land_plan = resolve_surface_column(
            input(land, Some(GraphBiomeKind::TemperateGrassland), None),
            SurfacePlanConfig::new(23, 4),
        );

        assert_eq!(land_plan.hydrology_role, SurfaceHydrologyRole::Land);
        assert_eq!(land_plan.top_block, "grass");
        assert!(land_plan.vegetation_allowed);

        let mut ridge = column(4.0, 7.0, HeightfieldTerrainKind::Ridge);
        ridge.terrain_ruggedness = 0.95;
        ridge.ridge_influence = 0.95;

        let ridge_plan = resolve_surface_column(
            input(ridge, Some(GraphBiomeKind::TemperateGrassland), None),
            SurfacePlanConfig::new(23, 4),
        );

        assert_eq!(ridge_plan.hydrology_role, SurfaceHydrologyRole::Ridge);
        assert_eq!(ridge_plan.top_block, "grass");
        assert!(ridge_plan.vegetation_allowed);
    }

    #[test]
    fn active_water_and_river_cores_keep_sediment_materials() {
        let mut columns = Vec::new();
        for z in 0..3 {
            for x in 0..3 {
                let terrain_kind = if x == 1 && z == 1 {
                    HeightfieldTerrainKind::River
                } else {
                    HeightfieldTerrainKind::Ocean
                };
                let mut heightfield = column(x as f32, z as f32, terrain_kind);
                heightfield.surface_y = if x == 1 && z == 1 { -2 } else { -5 };
                heightfield.water_y = Some(0);
                heightfield.river_valley_strength = 1.0;
                heightfield.river_flow_hint = 0.8;
                heightfield.river_bed_depth_blocks = 2.0;
                columns.push(heightfield);
            }
        }
        let heightfield = heightfield_tile(3, 3, columns);
        let macro_field = macro_field_tile(3, 3, GraphBiomeKind::TemperateGrassland);

        let area = generate_surface_plan_area(
            &heightfield,
            Some(&macro_field),
            SurfacePlanConfig {
                boundary_mix_radius_blocks: 0,
                ..SurfacePlanConfig::new(23, 4)
            },
        );
        let center = area.column(1, 1).unwrap();
        let ocean = area.column(0, 0).unwrap();

        assert_eq!(center.hydrology_role, SurfaceHydrologyRole::River);
        assert!(matches!(center.top_block, "wet_gravel" | "mud" | "silt"));
        assert!(matches!(center.sediment_block, "gravel" | "silt"));
        assert_eq!(ocean.hydrology_role, SurfaceHydrologyRole::Ocean);
        assert!(matches!(ocean.top_block, "silt" | "gravel"));
    }

    #[test]
    fn default_noisy_boundary_mixing_is_one_block_sparse_support() {
        let config = SurfacePlanConfig::default();

        assert_eq!(config.boundary_mix_radius_blocks, 1);
        assert_eq!(config.boundary_mix_strength_percent, 28);
    }

    #[test]
    fn noisy_boundary_mixing_steps_neighbor_materials_without_changing_height_or_role() {
        let config = SurfacePlanConfig {
            boundary_mix_radius_blocks: 1,
            boundary_mix_strength_percent: 100,
            ..SurfacePlanConfig::new(7, 2)
        };
        let mut columns = vec![
            test_plan(0, 0, "sand", SurfaceHydrologyRole::Land),
            test_plan(1, 0, "grass", SurfaceHydrologyRole::Land),
            test_plan(2, 0, "grass", SurfaceHydrologyRole::Land),
        ];

        apply_noisy_boundary_mixing(&mut columns, 3, 1, config, None);

        assert_eq!(columns[0].surface_y, 4);
        assert_eq!(columns[0].water_y, None);
        assert_eq!(columns[0].hydrology_role, SurfaceHydrologyRole::Land);
        assert_eq!(columns[1].surface_y, 4);
        assert_eq!(columns[1].hydrology_role, SurfaceHydrologyRole::Land);
        assert!(
            columns[0].top_block == "grass" || columns[1].top_block == "sand",
            "at least one boundary-adjacent column should receive the neighbor material"
        );
        assert_eq!(columns[2].top_block, "grass");
    }

    #[test]
    fn noisy_boundary_mixing_does_not_mutually_swap_adjacent_materials() {
        let config = SurfacePlanConfig {
            boundary_mix_radius_blocks: 1,
            boundary_mix_strength_percent: 100,
            ..SurfacePlanConfig::new(42, 1)
        };
        let mut columns = vec![
            test_plan(0, 0, "sand", SurfaceHydrologyRole::Land),
            test_plan(1, 0, "wet_sand", SurfaceHydrologyRole::Land),
        ];

        apply_noisy_boundary_mixing(&mut columns, 2, 1, config, None);

        assert_ne!(
            (columns[0].top_block, columns[1].top_block),
            ("wet_sand", "sand"),
            "local breakup may step one side of a boundary, but adjacent columns must not exchange top materials"
        );
    }

    #[test]
    fn noisy_boundary_mixing_does_not_jump_diagonal_corners() {
        let config = SurfacePlanConfig {
            boundary_mix_radius_blocks: 1,
            boundary_mix_strength_percent: 100,
            ..SurfacePlanConfig::new(7, 2)
        };
        let mut columns = vec![
            test_plan(0, 0, "grass", SurfaceHydrologyRole::Land),
            test_plan(1, 0, "grass", SurfaceHydrologyRole::Land),
            test_plan(2, 0, "grass", SurfaceHydrologyRole::Land),
            test_plan(0, 1, "grass", SurfaceHydrologyRole::Land),
            test_plan(1, 1, "grass", SurfaceHydrologyRole::Land),
            test_plan(2, 1, "grass", SurfaceHydrologyRole::Land),
            test_plan(0, 2, "sand", SurfaceHydrologyRole::Land),
            test_plan(1, 2, "grass", SurfaceHydrologyRole::Land),
            test_plan(2, 2, "grass", SurfaceHydrologyRole::Land),
        ];

        apply_noisy_boundary_mixing(&mut columns, 3, 3, config, None);

        assert_eq!(
            columns[4].top_block, "grass",
            "diagonal-only contact must not create a square-corner material intrusion"
        );
    }

    #[test]
    fn noisy_boundary_mixing_does_not_cross_macro_owner_sites() {
        let config = SurfacePlanConfig {
            boundary_mix_radius_blocks: 1,
            boundary_mix_strength_percent: 100,
            ..SurfacePlanConfig::new(7, 2)
        };
        let mut columns = vec![
            test_plan(0, 0, "sand", SurfaceHydrologyRole::Land),
            test_plan(1, 0, "grass", SurfaceHydrologyRole::Land),
        ];
        let mix_sources = vec![
            SurfaceMixSource {
                owner_site: Some(VoronoiSiteId(1)),
            },
            SurfaceMixSource {
                owner_site: Some(VoronoiSiteId(2)),
            },
        ];

        apply_noisy_boundary_mixing(&mut columns, 2, 1, config, Some(&mix_sources));

        assert_eq!(
            columns[0].top_block, "sand",
            "local material breakup must not copy material across the macro owner boundary"
        );
        assert_eq!(
            columns[1].top_block, "grass",
            "local material breakup must not copy material across the macro owner boundary"
        );
    }

    #[test]
    fn final_owner_top_audit_ignores_active_water_and_flags_non_water_conflict() {
        let mut land_a = sample(0.0, 0.0, GraphBiomeKind::TemperateGrassland);
        let mut land_b = sample(1.0, 0.0, GraphBiomeKind::TemperateGrassland);
        let mut ocean = sample(2.0, 0.0, GraphBiomeKind::ShallowOcean);
        let mut lake = sample(3.0, 0.0, GraphBiomeKind::Lake);
        let mut river = sample(4.0, 0.0, GraphBiomeKind::TemperateGrassland);
        for sample in [&mut land_a, &mut land_b, &mut ocean, &mut lake, &mut river] {
            sample.nearest_site = Some(VoronoiSiteId(7));
        }
        let mut columns = vec![
            test_plan(0, 0, "grass", SurfaceHydrologyRole::Land),
            test_plan(1, 0, "sand", SurfaceHydrologyRole::Land),
            test_plan(2, 0, "silt", SurfaceHydrologyRole::Ocean),
            test_plan(3, 0, "mud", SurfaceHydrologyRole::Lake),
            test_plan(4, 0, "wet_gravel", SurfaceHydrologyRole::River),
        ];
        for column in &mut columns[2..] {
            column.water_y = Some(column.surface_y + 1);
        }

        let audit =
            audit_final_non_water_tops_by_owner(&[land_a, land_b, ocean, lake, river], &columns);

        assert_eq!(audit.checked_column_count, 2);
        assert_eq!(audit.owner_count, 1);
        assert_eq!(audit.conflicting_owner_count, 1);
        assert!(
            audit.conflict_representatives[0].contains("grass")
                && audit.conflict_representatives[0].contains("sand")
        );
    }

    #[test]
    fn final_owner_top_audit_counts_dry_river_role_as_non_water_surface() {
        let dry_river = test_plan(0, 0, "wet_gravel", SurfaceHydrologyRole::River);
        let mut active_river = dry_river;
        active_river.water_y = Some(active_river.surface_y + 1);

        assert!(is_non_water_owner_surface_column(dry_river));
        assert!(!is_non_water_owner_surface_column(active_river));
    }

    #[test]
    fn noisy_boundary_mixing_restores_mutual_cross_owner_top_swaps() {
        let mut columns = vec![
            test_plan(0, 0, "sand", SurfaceHydrologyRole::Coast),
            test_plan(1, 0, "wet_sand", SurfaceHydrologyRole::Coast),
        ];
        let original = vec![
            test_plan(0, 0, "wet_sand", SurfaceHydrologyRole::Coast),
            test_plan(1, 0, "sand", SurfaceHydrologyRole::Coast),
        ];
        let mix_sources = vec![
            SurfaceMixSource {
                owner_site: Some(VoronoiSiteId(1)),
            },
            SurfaceMixSource {
                owner_site: Some(VoronoiSiteId(2)),
            },
        ];

        restore_cross_owner_boundary_swaps(&mut columns, &original, 2, 1, Some(&mix_sources));

        assert_eq!(columns[0].top_block, "wet_sand");
        assert_eq!(columns[1].top_block, "sand");
    }

    #[test]
    fn noisy_boundary_mixing_does_not_cross_active_water_boundaries() {
        let config = SurfacePlanConfig {
            boundary_mix_radius_blocks: 1,
            boundary_mix_strength_percent: 100,
            ..SurfacePlanConfig::new(7, 2)
        };
        let mut columns = vec![
            test_plan(0, 0, "sand", SurfaceHydrologyRole::Ocean),
            test_plan(1, 0, "grass", SurfaceHydrologyRole::Land),
        ];
        columns[0].water_y = Some(5);

        apply_noisy_boundary_mixing(&mut columns, 2, 1, config, None);

        assert_eq!(columns[0].top_block, "sand");
        assert_eq!(columns[0].water_y, Some(5));
        assert_eq!(columns[1].top_block, "grass");
        assert_eq!(columns[1].water_y, None);
    }

    fn assert_known(block: &'static str) {
        assert!(
            KNOWN_BLOCK_KEYS.contains(&block),
            "unknown block key in surface policy: {block}"
        );
    }

    fn input(
        heightfield: HeightfieldColumn,
        biome: Option<GraphBiomeKind>,
        biome_context: Option<GraphBiomeContext>,
    ) -> SurfaceColumnInput {
        SurfaceColumnInput {
            world_x: heightfield.position.x.round() as i32,
            world_z: heightfield.position.z.round() as i32,
            heightfield,
            biome,
            biome_context,
            runtime_surface: None,
        }
    }

    fn column(x: f32, z: f32, terrain_kind: HeightfieldTerrainKind) -> HeightfieldColumn {
        let water_y = matches!(
            terrain_kind,
            HeightfieldTerrainKind::Ocean
                | HeightfieldTerrainKind::Lake
                | HeightfieldTerrainKind::River
        )
        .then_some(1);
        HeightfieldColumn {
            position: WorldPlanePoint::new(x, z),
            raw_surface_height_blocks: 0.0,
            contour_guided_surface_height_blocks: 0.0,
            constrained_surface_height_blocks: 0.0,
            surface_height_blocks: 0.0,
            surface_y: 0,
            water_level_blocks: water_y.map(|y| y as f32),
            water_y,
            river_water_height_blocks: (terrain_kind == HeightfieldTerrainKind::River)
                .then_some(1.0),
            terrain_kind,
            macro_elevation: 0.0,
            combined_macro_height: 0.0,
            ocean_mask: (terrain_kind == HeightfieldTerrainKind::Ocean) as u8 as f32,
            lake_mask: (terrain_kind == HeightfieldTerrainKind::Lake) as u8 as f32,
            dry_basin_mask: (terrain_kind == HeightfieldTerrainKind::DryBasin) as u8 as f32,
            coast_mask: (terrain_kind == HeightfieldTerrainKind::Coast) as u8 as f32,
            ridge_influence: (terrain_kind == HeightfieldTerrainKind::Ridge) as u8 as f32,
            terrain_ruggedness: 0.0,
            river_core_strength: if terrain_kind == HeightfieldTerrainKind::River {
                1.0
            } else {
                0.0
            },
            river_shoulder_strength: if terrain_kind == HeightfieldTerrainKind::River {
                1.0
            } else {
                0.0
            },
            river_valley_strength: if terrain_kind == HeightfieldTerrainKind::River {
                1.0
            } else {
                0.0
            },
            river_distance_blocks: if terrain_kind == HeightfieldTerrainKind::River {
                0.0
            } else {
                f32::INFINITY
            },
            river_flow_hint: if terrain_kind == HeightfieldTerrainKind::River {
                0.5
            } else {
                0.0
            },
            river_bed_depth_blocks: 0.0,
            river_bank_roughness_hint: 0.0,
            river_gravel_hint: 0.0,
            river_cutbank_hint: 0.0,
            meso_delta_blocks: 0.0,
            micro_relief_blocks: 0.0,
        }
    }

    fn sample(x: f32, z: f32, biome: GraphBiomeKind) -> MacroFieldSample {
        MacroFieldSample {
            position: WorldPlanePoint::new(x, z),
            nearest_site: None,
            surface_kind: None,
            biome_context: Some(GraphBiomeContext {
                temperature: 0.5,
                hydration: 0.5,
                elevation: 0.0,
                continentality: 0.0,
                coastness: 0.0,
                mountainness: 0.0,
                ruggedness: 0.0,
                water_role: if biome == GraphBiomeKind::Lake {
                    GraphBiomeWaterRole::Lake
                } else {
                    GraphBiomeWaterRole::Land
                },
            }),
            biome: Some(biome),
            macro_elevation: 0.0,
            ocean_mask: 0.0,
            coast_mask: 0.0,
            lake_mask: (biome == GraphBiomeKind::Lake) as u8 as f32,
            dry_basin_mask: 0.0,
            ridge_influence: 0.0,
            river_core_strength: 0.0,
            river_shoulder_strength: 0.0,
            river_valley_strength: 0.0,
            river_distance_blocks: f32::INFINITY,
            river_flow_hint: 0.0,
            river_longitudinal_blocks: 0.0,
            river_bed_depth_hint: 0.0,
            river_bank_roughness_hint: 0.0,
            river_gravel_hint: 0.0,
            river_cutbank_hint: 0.0,
            combined_macro_height: 0.0,
        }
    }

    fn heightfield_tile(
        width: u32,
        height: u32,
        columns: Vec<HeightfieldColumn>,
    ) -> HeightfieldTile {
        HeightfieldTile {
            width,
            height,
            sample_spacing_blocks: 1.0,
            columns,
            stats: Default::default(),
            config: Default::default(),
        }
    }

    fn macro_field_tile(width: u32, height: u32, biome: GraphBiomeKind) -> MacroFieldTile {
        let mut samples = Vec::new();
        for z in 0..height {
            for x in 0..width {
                samples.push(sample(x as f32, z as f32, biome));
            }
        }
        MacroFieldTile {
            config: MacroFieldTileConfig::new(0.0, 0.0, width, height, 1.0),
            samples,
            stats: MacroFieldTileStats::default(),
        }
    }

    fn test_plan(
        world_x: i32,
        world_z: i32,
        top_block: &'static str,
        hydrology_role: SurfaceHydrologyRole,
    ) -> SurfaceColumnPlan {
        SurfaceColumnPlan {
            world_x,
            world_z,
            surface_y: 4,
            water_y: None,
            hydrology_role,
            top_block,
            subsurface_block: "dirt",
            base_block: "stone",
            underwater_top_block: top_block,
            exposed_block: "exposed_rock",
            sediment_block: "gravel",
            soil_depth_blocks: 3,
            vegetation_allowed: true,
            cover_phase: 0,
        }
    }
}
