# ecology table

## Role

- Define biome-specific vegetation and animal candidate pools for ecology simulation.
- Act as the source of truth for early spawn, grazing, growth, territory, and rare boss-grade ecology events.
- Keep species labels stable enough for textmode, ECS entity spawning, renderer assets, and future loot/AI tables to converge on.

## Boundaries

- This table is policy data, not persistent entity storage.
- `ecology.rs` may implement a smaller first slice, but its biome choices should come from this table.
- Final spawn rates, population caps, seasonal windows, and AI behavior are separate tuning layers.
- `apex_predator` means rare boss-grade ecological threat. Most biomes may have none.

## Columns

- `flowers`: visible flowering plants or bloom candidates.
- `grasses`: grasses, ground cover, reeds, mosses, and herbaceous forage.
- `trees`: tree or tall woody vegetation candidates.
- `small_herbivore`: small prey and grazing animals.
- `large_herbivore`: larger grazers or browsers.
- `small_carnivore`: small hunters and scavengers.
- `predator`: regular danger-tier predators.
- `apex_predator`: rare boss-grade or territory-defining threat.

## Biome Ecology Candidates

| Biome | flowers | grasses | trees | small_herbivore | large_herbivore | small_carnivore | predator | apex_predator |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| ShallowOcean | sea_lavender | eelgrass, kelp | none | small_fish, crab | reef_turtle | gull | reef_shark | ancient_reef_shark |
| DeepOcean | none | deep_kelp | none | small_fish, squid | tuna | eel | deep_shark | leviathan |
| Lake | water_lily | reed, cattail | willow | frog, small_fish | water_deer | otter | lake_serpent | ancient_lake_serpent |
| Marsh | marsh_marigold | reed, cattail, sedge | willow | frog, muskrat | water_deer | otter | marsh_wolf | mire_stalker |
| Swamp | swamp_iris | fern, duckweed | cypress, willow | frog, muskrat | boar | fox | swamp_bear | bog_warden |
| FloodedForest | orchid | fern, reed | cypress, alder | frog, hare | boar, deer | fox | black_bear | floodwood_guardian |
| Mangrove | mangrove_flower | seagrass, salt_reed | mangrove | crab, wading_bird | tapir | fishing_cat | crocodile | elder_crocodile |
| EstuarineCoast | sea_aster | salt_grass, reed | willow | crab, wading_bird | water_deer | fox | crocodile | tide_maw |
| LagoonCoast | beach_morning_glory | dune_grass, seagrass | palm | crab, shorebird | reef_turtle | gull | reef_shark | lagoon_leviathan |
| RockyCoast | thrift | lichen, coastal_grass | wind_pine | hare, gull | goat | fox | cliff_wolf | storm_drake |
| SandyCoast | beach_pea | dune_grass | palm, pine | crab, shorebird | turtle | gull | jackal | dune_wyrm |
| Desert | cactus_flower | desert_grass | acacia | jerboa, hare | camel | fennec_fox | desert_wolf | sand_wyrm |
| SemiDesert | sage_flower | scrub_grass | acacia, juniper | hare | antelope | fox | jackal | dust_mane |
| Steppe | aster, poppy | feather_grass, steppe_grass | sparse_birch | marmot, hare | deer, horse | fox | wolf | steppe_alpha |
| DryShrubland | sage_bloom | scrub_grass | juniper | hare | goat | fox | cougar | thornback |
| MediterraneanShrubland | lavender, rosemary_flower | dry_grass | olive, pine | hare | goat, deer | fox | lynx | old_lynx |
| PolarIce | none | ice_moss | none | snow_hare | none | arctic_fox | polar_bear | frost_colossus |
| PolarBarrens | saxifrage | lichen, moss | none | snow_hare | musk_ox | arctic_fox | polar_bear | frost_colossus |
| Tundra | cotton_grass_flower | tundra_grass, moss | dwarf_birch | lemming, snow_hare | caribou | arctic_fox | wolf | white_bear |
| SubalpineWoodland | alpine_aster | meadow_grass | fir, spruce | hare | deer, goat | fox | wolf, bear | ancient_bear |
| AlpineMeadow | edelweiss, alpine_aster | meadow_grass | dwarf_pine | marmot, hare | goat | fox | eagle | mountain_spirit |
| BorealForest | fireweed | moss, fern | spruce, pine, birch | hare | deer, moose | fox | wolf, bear | ancient_bear |
| TropicalRainforest | orchid, hibiscus | fern, jungle_grass | kapok, mahogany | monkey, small_bird | tapir | ocelot | jaguar | emerald_jaguar |
| MonsoonForest | lotus, orchid | fern, tall_grass | teak, bamboo | monkey, hare | deer, boar | civet | tiger | storm_tiger |
| TropicalDryForest | flame_lily | dry_grass | teak, acacia | hare, monkey | deer, boar | civet | leopard | old_leopard |
| Savanna | acacia_flower | tall_grass | acacia, baobab | meerkat, hare | antelope, buffalo | jackal | lion | elder_lion |
| TemperateRainforest | trillium | fern, moss | cedar, hemlock, maple | hare | deer, elk | fox | wolf, bear | mossback_bear |
| TemperateMixedForest | bluebell, clover | fern, meadow_grass | oak, maple, pine | hare | deer, boar | fox | wolf, bear | old_forest_bear |
| TemperateBroadleafForest | wildflower, clover | fern, meadow_grass | oak, birch, maple | hare | deer, boar | fox | wolf | ancient_stag |
| TemperateGrassland | clover, poppy | meadow_grass, ryegrass | sparse_oak | hare | deer, bison | fox | wolf | grassland_alpha |

## Implementation Notes

- Current `SimSpecies` and `SimPlantKind` expose only part of this table.
- `ecology.rs` should grow toward table-driven lookup before persistent ECS animal entities are added.
- Textmode may display candidate labels directly, but final gameplay should map them to stable entity archetypes.
- Spawn rate, pack size, boss rarity, seasonality, and time-of-day behavior should be separate columns or data files when the first real entity system exists.
