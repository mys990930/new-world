use crate::world::{BiomeFamily, ChunkCoord};

use super::{
    SimEcologyEvent, SimEvent, SimPlantGrowthStage, SimPlantKind, SimRegion, SimSpatialScope,
    SimSpecies, SimTick, SimulationResult, SubSystemId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcologySimConfig {
    pub ticks_per_ecology_step: u32,
}

impl EcologySimConfig {
    pub fn for_fixed_rate(ticks_per_second: u32) -> Self {
        Self {
            ticks_per_ecology_step: ticks_per_second.max(1),
        }
    }
}

impl Default for EcologySimConfig {
    fn default() -> Self {
        Self::for_fixed_rate(20)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcologySimChunkInput {
    pub coord: ChunkCoord,
    pub biome: BiomeFamily,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcologySimBundleInput {
    pub world_seed: u64,
    pub chunks: Vec<EcologySimChunkInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcologySimInput {
    pub tick: SimTick,
    pub region: SimRegion,
    pub world_seed: u64,
    pub chunks: Vec<EcologySimChunkInput>,
}

pub struct EcologySim {
    config: EcologySimConfig,
}

impl EcologySim {
    pub fn new(config: EcologySimConfig) -> Self {
        Self { config }
    }

    pub fn step(&self, input: EcologySimInput) -> SimulationResult {
        let mut result = SimulationResult::empty(SubSystemId::Ecology, input.tick);
        let step_period = u64::from(self.config.ticks_per_ecology_step.max(1));

        if input.tick.index % step_period != 0 {
            return result;
        }

        let ecology_window = input.tick.index / step_period;
        let mut chunks = input.chunks;
        chunks.sort_by_key(|chunk| chunk.coord);

        for chunk in chunks {
            let event =
                ecology_event_for_chunk(input.world_seed, ecology_window, chunk.coord, chunk.biome);
            result.events.push(SimEvent::EcologyEventObserved {
                scope: SimSpatialScope::Chunk(chunk.coord),
                biome: chunk.biome,
                event,
            });
        }

        result
    }
}

fn ecology_event_for_chunk(
    world_seed: u64,
    ecology_window: u64,
    coord: ChunkCoord,
    biome: BiomeFamily,
) -> SimEcologyEvent {
    let roll = hash_u64(world_seed, ecology_window, coord, 0xEC01_0001);
    match roll % 5 {
        0 => SimEcologyEvent::AnimalSpawned {
            species: primary_herbivore_for_biome(biome),
        },
        1 => SimEcologyEvent::AnimalFight {
            attacker: predator_for_biome(biome),
            defender: primary_herbivore_for_biome(biome),
        },
        2 => SimEcologyEvent::CarcassCreated {
            species: carcass_species_for_biome(biome),
        },
        3 => SimEcologyEvent::PlantGrazed {
            plant: forage_for_biome(biome),
            by: primary_herbivore_for_biome(biome),
        },
        _ => SimEcologyEvent::PlantGrowthAdvanced {
            plant: forage_for_biome(biome),
            stage: growth_stage_for_window(ecology_window, coord),
        },
    }
}

fn primary_herbivore_for_biome(biome: BiomeFamily) -> SimSpecies {
    match biome {
        BiomeFamily::Savanna
        | BiomeFamily::Steppe
        | BiomeFamily::TemperateGrassland
        | BiomeFamily::AlpineMeadow => SimSpecies::LargeHerbivore,
        BiomeFamily::Desert
        | BiomeFamily::SemiDesert
        | BiomeFamily::DryShrubland
        | BiomeFamily::MediterraneanShrubland
        | BiomeFamily::Tundra
        | BiomeFamily::PolarBarrens
        | BiomeFamily::PolarIce
        | BiomeFamily::Oceanic => SimSpecies::SmallHerbivore,
        _ => SimSpecies::SmallHerbivore,
    }
}

fn predator_for_biome(biome: BiomeFamily) -> SimSpecies {
    match biome {
        BiomeFamily::Savanna
        | BiomeFamily::Steppe
        | BiomeFamily::TemperateGrassland
        | BiomeFamily::BorealForest
        | BiomeFamily::TemperateMixedForest
        | BiomeFamily::TemperateRainforest
        | BiomeFamily::TropicalRainforest
        | BiomeFamily::MonsoonForest
        | BiomeFamily::TropicalDryForest => SimSpecies::LargePredator,
        _ => SimSpecies::SmallPredator,
    }
}

fn carcass_species_for_biome(biome: BiomeFamily) -> SimSpecies {
    if matches!(
        biome,
        BiomeFamily::Savanna | BiomeFamily::Steppe | BiomeFamily::TemperateGrassland
    ) {
        SimSpecies::LargeHerbivore
    } else {
        SimSpecies::SmallHerbivore
    }
}

fn forage_for_biome(biome: BiomeFamily) -> SimPlantKind {
    match biome {
        BiomeFamily::TemperateBroadleafForest
        | BiomeFamily::TemperateMixedForest
        | BiomeFamily::TemperateRainforest
        | BiomeFamily::BorealForest
        | BiomeFamily::TropicalDryForest
        | BiomeFamily::TropicalRainforest
        | BiomeFamily::MonsoonForest
        | BiomeFamily::FloodedForest => SimPlantKind::Tree,
        BiomeFamily::Desert
        | BiomeFamily::SemiDesert
        | BiomeFamily::DryShrubland
        | BiomeFamily::MediterraneanShrubland
        | BiomeFamily::Mangrove => SimPlantKind::Shrub,
        _ => SimPlantKind::Grass,
    }
}

fn growth_stage_for_window(ecology_window: u64, coord: ChunkCoord) -> SimPlantGrowthStage {
    match hash_u64(0, ecology_window, coord, 0xEC01_0002) % 4 {
        0 => SimPlantGrowthStage::Seedling,
        1 => SimPlantGrowthStage::Growing,
        2 => SimPlantGrowthStage::Mature,
        _ => SimPlantGrowthStage::Dormant,
    }
}

fn hash_u64(world_seed: u64, ecology_window: u64, coord: ChunkCoord, salt: u64) -> u64 {
    let mut state = world_seed
        ^ ecology_window.rotate_left(17)
        ^ (coord.0 as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (coord.1 as i64 as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93)
        ^ (coord.2 as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ salt;
    splitmix64(&mut state)
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::world::{AtlasArea, AtlasCoord};

    fn input(chunks: Vec<EcologySimChunkInput>) -> EcologySimInput {
        let center = AtlasCoord::new(0, 0);
        EcologySimInput {
            tick: SimTick {
                index: 20,
                delta: Duration::from_millis(50),
            },
            region: SimRegion {
                center_atlas: center,
                atlas_area: AtlasArea::new(center, 1, 1).unwrap(),
            },
            world_seed: 7,
            chunks,
        }
    }

    #[test]
    fn ecology_step_is_deterministic_for_same_input() {
        let sim = EcologySim::new(EcologySimConfig::default());
        let chunks = vec![
            EcologySimChunkInput {
                coord: ChunkCoord(0, 0, 0),
                biome: BiomeFamily::TemperateGrassland,
            },
            EcologySimChunkInput {
                coord: ChunkCoord(1, 0, 0),
                biome: BiomeFamily::TemperateMixedForest,
            },
            EcologySimChunkInput {
                coord: ChunkCoord(0, 0, 1),
                biome: BiomeFamily::Marsh,
            },
        ];

        let left = sim.step(input(chunks.clone()));
        let right = sim.step(input(chunks));

        assert_eq!(left, right);
    }

    #[test]
    fn ecology_event_carries_chunk_scope_and_cell_biome() {
        let sim = EcologySim::new(EcologySimConfig::default());
        let coord = ChunkCoord(2, 0, -3);
        let biome = BiomeFamily::TemperateGrassland;

        let result = sim.step(input(vec![EcologySimChunkInput { coord, biome }]));

        assert!(result.events.iter().any(|event| {
            matches!(
                event,
                SimEvent::EcologyEventObserved {
                    scope: SimSpatialScope::Chunk(event_coord),
                    biome: event_biome,
                    ..
                } if *event_coord == coord && *event_biome == biome
            )
        }));
    }

    #[test]
    fn ecology_events_cover_textmode_categories_across_deterministic_chunk_window() {
        let sim = EcologySim::new(EcologySimConfig::default());
        let chunks = (-2..=2)
            .flat_map(|z| {
                (-2..=2).map(move |x| EcologySimChunkInput {
                    coord: ChunkCoord(x, 0, z),
                    biome: BiomeFamily::TemperateGrassland,
                })
            })
            .collect::<Vec<_>>();

        let result = sim.step(input(chunks));

        assert!(result.events.iter().any(|event| matches!(
            event,
            SimEvent::EcologyEventObserved {
                event: SimEcologyEvent::AnimalSpawned { .. },
                ..
            }
        )));
        assert!(result.events.iter().any(|event| matches!(
            event,
            SimEvent::EcologyEventObserved {
                event: SimEcologyEvent::AnimalFight { .. },
                ..
            }
        )));
        assert!(result.events.iter().any(|event| matches!(
            event,
            SimEvent::EcologyEventObserved {
                event: SimEcologyEvent::CarcassCreated { .. },
                ..
            }
        )));
        assert!(result.events.iter().any(|event| matches!(
            event,
            SimEvent::EcologyEventObserved {
                event: SimEcologyEvent::PlantGrazed { .. },
                ..
            }
        )));
        assert!(result.events.iter().any(|event| matches!(
            event,
            SimEvent::EcologyEventObserved {
                event: SimEcologyEvent::PlantGrowthAdvanced { .. },
                ..
            }
        )));
    }
}
