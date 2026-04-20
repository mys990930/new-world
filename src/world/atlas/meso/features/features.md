# features

## Role

- hold one submodule and one design note per concrete meso feature type
- keep terrain and ecology intent close to the code stub for later implementation

## Contract

- every concrete feature has:
  - `mod.rs`
  - `<feature>.md`
- the Rust module always exposes a `MesoFeatureDef`
- launch features that are already wired into runtime guide generation may also keep their feature-specific build, rasterize, and chunk-apply shaping helpers in that same folder instead of centralizing every implementation detail in `atlas/meso.rs`
- the markdown file holds planning content:
  - stage label
  - placement family
  - hydrology coupling
  - summary
  - terrain and ecology planning notes

## Current Candidate Pool

### Launch
- `hill_cluster`
- `upland_terrace`
- `escarpment_band`
- `shallow_basin`
- `ravine`
- `coastal_cliff_band`
- `dune_field`
- `crater`

### Extended
- `rolling_hill_belt`
- `ridge_spur`
- `summit_group`
- `upland_knob_field`
- `mesa_island`
- `fault_scarp`
- `shoulder_shelf`
- `closed_basin`
- `wet_basin`
- `sinkhole_field`
- `broad_valley`
- `narrow_valley`
- `canyon_reach`
- `gorge_cut`
- `cirque_basin`
- `creek_corridor`
- `secondary_channel_belt`
- `alluvial_fan`
- `terraced_floodplain`
- `levee_strip`
- `oxbow_lowland`
- `delta_lobe`
- `distributary_fan`
- `rocky_headland_chain`
- `cove_breakup`
- `barrier_spit`
- `lagoon_rim`
- `tidal_flat_bench`
- `backshore_dune_field`
- `wave_cut_shelf`
- `linear_dune_belt`
- `badlands_patch`
- `yardang_band`
- `dry_gully_network`
- `pediment_steps`
- `mesa_cluster`
- `eroded_butte_field`
- `glacial_trough`
- `moraine_belt`
- `crevasse_belt`
- `permafrost_pingo_field`
- `frost_heave_plain`
- `snow_basin`
- `glacial_bench`
- `caldera`
- `lava_field`
- `fissure_ridge`
- `volcanic_terrace`
- `marsh_flat`
- `peaty_hollow`
- `spring_basin`
- `wet_meadow_bowl`

### Deferred
- `sea_stack_cluster`
- `fjord_wall_breakup`
- `icefall_breakup`
- `cinder_cone_cluster`
- `natural_arch`

- the full scaffolded pool is broader than the currently emitted runtime guide subset
- current guide generation still only emits the Wave 1A subset: `hill_cluster`, `shallow_basin`, `escarpment_band`, and `upland_terrace`
- `atlas/meso.rs` should stay focused on shared lottery, sampling, and raster dispatch, while feature-specific realization details for runtime-wired launch features should live under the matching `features/<name>/` folder
