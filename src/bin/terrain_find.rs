use std::cmp::Ordering;
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fmt::Debug;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;

use new_world::world::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasCoord, CHUNK_EDGE_I32, MesoGuideSample,
    RegionArchetype, RegionCatalogStatus, RegionClassSample, WorldMeta, generate_atlas_fields,
    generate_atlas_structure, generate_meso_guides, meso_feature_defs, region_archetype_def,
    region_catalog_entries, resolve_region_classes, sample_meso_guides, sample_region_classes,
};

const DEFAULT_ORIGIN_CELL_X: i32 = 0;
const DEFAULT_ORIGIN_CELL_Z: i32 = 0;
const DEFAULT_SEARCH_RADIUS_CELLS: i32 = 16;
const DEFAULT_CHUNK_STEP: u32 = 1;
const DEFAULT_TOP: usize = 8;
const DEFAULT_PREVIEW_RANK: usize = 1;
const DEFAULT_PREVIEW_RADIUS: i32 = 4;
const DEFAULT_PREVIEW_WIDTH: u32 = 1600;
const DEFAULT_PREVIEW_HEIGHT: u32 = 900;
const DEFAULT_PREVIEW_QUARTER_TURNS: u8 = 0;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeMesoChannel {
    HillCluster,
    ShallowBasin,
    EscarpmentBand,
    UplandTerrace,
}

impl RuntimeMesoChannel {
    fn value(self, sample: MesoGuideSample) -> f32 {
        match self {
            Self::HillCluster => sample.hilliness,
            Self::ShallowBasin => sample.basin_weight,
            Self::EscarpmentBand => sample.escarpment_weight,
            Self::UplandTerrace => sample.terrace_weight,
        }
    }
}

#[derive(Debug, Clone)]
struct RequestedMeso {
    key: String,
    runtime_channel: Option<RuntimeMesoChannel>,
    summary: &'static str,
}

#[derive(Debug, Clone)]
struct RequestedMesoMatch {
    key: String,
    allowed_by_archetype: bool,
    runtime_value: Option<f32>,
}

#[derive(Debug, Clone)]
struct Candidate {
    chunk_x: i32,
    chunk_z: i32,
    atlas_x: i32,
    atlas_z: i32,
    score: f32,
    allowed_fraction: f32,
    runtime_peak: f32,
    interestingness: f32,
    region: RegionClassSample,
    meso: MesoGuideSample,
    requested_meso: Vec<RequestedMesoMatch>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(cli_error(usage()));
    }

    let list_archetypes = take_flag(&mut args, "--list-archetypes");
    let list_meso = take_flag(&mut args, "--list-meso");
    if list_archetypes {
        print_archetype_catalog();
    }
    if list_meso {
        print_meso_catalog();
    }
    if list_archetypes || list_meso {
        return Ok(());
    }

    let seed = parse_required::<u64>(&mut args, "seed")?;
    let mut origin_cell_x = DEFAULT_ORIGIN_CELL_X;
    let mut origin_cell_z = DEFAULT_ORIGIN_CELL_Z;
    let mut search_radius_cells = DEFAULT_SEARCH_RADIUS_CELLS;
    let mut chunk_step = DEFAULT_CHUNK_STEP;
    let mut top = DEFAULT_TOP;
    let mut preview_rank = DEFAULT_PREVIEW_RANK;
    let mut preview_radius = DEFAULT_PREVIEW_RADIUS;
    let mut preview_width = DEFAULT_PREVIEW_WIDTH;
    let mut preview_height = DEFAULT_PREVIEW_HEIGHT;
    let mut preview_quarter_turns = DEFAULT_PREVIEW_QUARTER_TURNS;
    let mut render_preview = false;
    let mut preview_output: Option<PathBuf> = None;
    let mut archetype_tokens = Vec::new();
    let mut meso_tokens = Vec::new();

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--origin-cell-x" => origin_cell_x = parse_required::<i32>(&mut args, "origin-cell-x")?,
            "--origin-cell-z" => origin_cell_z = parse_required::<i32>(&mut args, "origin-cell-z")?,
            "--search-radius-cells" => {
                search_radius_cells = parse_required::<i32>(&mut args, "search-radius-cells")?
            }
            "--chunk-step" => chunk_step = parse_required::<u32>(&mut args, "chunk-step")?,
            "--top" => top = parse_required::<usize>(&mut args, "top")?,
            "--archetype" => archetype_tokens.push(parse_required::<String>(&mut args, "archetype")?),
            "--meso" => meso_tokens.push(parse_required::<String>(&mut args, "meso")?),
            "--preview-rank" => preview_rank = parse_required::<usize>(&mut args, "preview-rank")?,
            "--preview-radius" => preview_radius = parse_required::<i32>(&mut args, "preview-radius")?,
            "--preview-width" => preview_width = parse_required::<u32>(&mut args, "preview-width")?,
            "--preview-height" => preview_height = parse_required::<u32>(&mut args, "preview-height")?,
            "--preview-quarter-turns" => {
                preview_quarter_turns = parse_required::<u8>(&mut args, "preview-quarter-turns")?
            }
            "--preview-output" => {
                preview_output = Some(PathBuf::from(parse_required::<String>(&mut args, "preview-output")?))
            }
            "--render-preview" => render_preview = true,
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    if search_radius_cells < 0 {
        return Err(cli_error("search-radius-cells must be non-negative"));
    }
    if chunk_step == 0 {
        return Err(cli_error("chunk-step must be >= 1"));
    }
    if top == 0 {
        return Err(cli_error("top must be >= 1"));
    }
    if preview_rank == 0 {
        return Err(cli_error("preview-rank must be >= 1"));
    }
    if preview_radius < 0 {
        return Err(cli_error("preview-radius must be non-negative"));
    }
    if preview_width == 0 || preview_height == 0 {
        return Err(cli_error("preview width and height must be >= 1"));
    }

    let requested_archetypes = resolve_requested_archetypes(&archetype_tokens)?;
    let requested_meso = resolve_requested_meso(&meso_tokens)?;

    let requested_search_area = AtlasArea::new(
        AtlasCoord::new(
            origin_cell_x - search_radius_cells,
            origin_cell_z - search_radius_cells,
        ),
        (search_radius_cells * 2 + 1) as u32,
        (search_radius_cells * 2 + 1) as u32,
    )?;
    // Region and meso samplers read neighboring cells on the positive axes, so
    // the generated lookup area needs one-cell padding around the requested search window.
    let generation_area = AtlasArea::new(
        AtlasCoord::new(
            requested_search_area.origin().x - 1,
            requested_search_area.origin().z - 1,
        ),
        requested_search_area.width() + 2,
        requested_search_area.height() + 2,
    )?;
    let meta = WorldMeta::new(seed);
    let fields = generate_atlas_fields(&meta, generation_area);
    let structure = generate_atlas_structure(&meta, generation_area);
    let regions = resolve_region_classes(&meta, generation_area, &fields, &structure);
    let meso = generate_meso_guides(&meta, generation_area, &fields, &structure);

    let min_chunk_x = requested_search_area.origin().x * ATLAS_CELL_SIZE_IN_CHUNKS as i32;
    let min_chunk_z = requested_search_area.origin().z * ATLAS_CELL_SIZE_IN_CHUNKS as i32;
    let max_chunk_x = (requested_search_area.origin().x + requested_search_area.width() as i32)
        * ATLAS_CELL_SIZE_IN_CHUNKS as i32
        - 1;
    let max_chunk_z = (requested_search_area.origin().z + requested_search_area.height() as i32)
        * ATLAS_CELL_SIZE_IN_CHUNKS as i32
        - 1;
    let retain_count = top.max(preview_rank);

    let mut scanned = 0_usize;
    let mut matched = 0_usize;
    let mut candidates = Vec::with_capacity(retain_count);

    for chunk_z in (min_chunk_z..=max_chunk_z).step_by(chunk_step as usize) {
        for chunk_x in (min_chunk_x..=max_chunk_x).step_by(chunk_step as usize) {
            scanned += 1;
            if let Some(candidate) = build_candidate(chunk_x, chunk_z, &regions, &meso, &requested_archetypes, &requested_meso) {
                matched += 1;
                push_top_candidate(&mut candidates, candidate, retain_count);
            }
        }
    }

    candidates.sort_by(compare_candidates);

    if candidates.is_empty() {
        return Err(cli_error(
            "no candidates matched the requested archetype / meso filters inside the search area",
        ));
    }

    println!("seed: {seed}");
    println!(
        "search atlas area: origin=({}, {}), size={}x{} cells",
        requested_search_area.origin().x,
        requested_search_area.origin().z,
        requested_search_area.width(),
        requested_search_area.height()
    );
    println!(
        "search chunk bounds: x={}..{}, z={}..{}, chunk_step={}",
        min_chunk_x, max_chunk_x, min_chunk_z, max_chunk_z, chunk_step
    );
    println!("scanned candidates: {scanned}");
    println!("matched candidates: {matched}");
    if requested_archetypes.is_empty() {
        println!("archetype filter: any launch archetype");
    } else {
        let archetypes = requested_archetypes
            .iter()
            .map(|archetype| archetype_key(*archetype))
            .collect::<Vec<_>>();
        println!("archetype filter: {}", archetypes.join(", "));
    }
    if requested_meso.is_empty() {
        println!("meso filter: none");
    } else {
        println!("meso filter:");
        for meso in &requested_meso {
            let mode = if meso.runtime_channel.is_some() {
                "runtime-backed"
            } else {
                "planned-only"
            };
            println!("  - {} [{}] {}", meso.key, mode, meso.summary);
        }
    }

    println!("top candidates:");
    for (index, candidate) in candidates.iter().take(top).enumerate() {
        println!(
            "{}. chunk=({}, {}) atlas=({}, {}) score={:.3}",
            index + 1,
            candidate.chunk_x,
            candidate.chunk_z,
            candidate.atlas_x,
            candidate.atlas_z,
            candidate.score
        );
        println!(
            "   archetype={} biome={} terrain={} climate={} relief={} hydrology={} coastal={}",
            archetype_key(candidate.region.archetype),
            enum_key(candidate.region.biome_family),
            enum_key(candidate.region.terrain_form_family),
            enum_key(candidate.region.climate_regime),
            enum_key(candidate.region.relief_class),
            enum_key(candidate.region.hydrology_context),
            enum_key(candidate.region.coastal_context)
        );
        println!(
            "   guide raw: hill={:.2} basin={:.2} escarpment={:.2} terrace={:.2} interestingness={:.2}",
            candidate.meso.hilliness,
            candidate.meso.basin_weight,
            candidate.meso.escarpment_weight,
            candidate.meso.terrace_weight,
            candidate.interestingness
        );
        if !candidate.requested_meso.is_empty() {
            println!(
                "   requested meso: {}",
                format_requested_meso_matches(&candidate.requested_meso)
            );
        }
        println!(
            "   preview: {}",
            preview_command_string(
                seed,
                candidate,
                preview_radius,
                preview_width,
                preview_height,
                preview_quarter_turns,
                preview_output.as_deref(),
            )
        );
    }

    let preview_candidate = candidates
        .get(preview_rank - 1)
        .ok_or_else(|| cli_error(format!("preview-rank {} exceeds {} results", preview_rank, candidates.len())))?;
    if render_preview {
        let output = preview_output
            .clone()
            .unwrap_or_else(|| default_preview_output(seed, preview_candidate, &requested_archetypes, &requested_meso));
        render_preview_for_candidate(
            seed,
            preview_candidate,
            preview_radius,
            preview_width,
            preview_height,
            preview_quarter_turns,
            &output,
        )?;
        println!("rendered preview: {}", output.display());
    }

    Ok(())
}

fn build_candidate(
    chunk_x: i32,
    chunk_z: i32,
    regions: &new_world::world::RegionClassMap,
    meso_guides: &new_world::world::MesoGuideMap,
    requested_archetypes: &[RegionArchetype],
    requested_meso: &[RequestedMeso],
) -> Option<Candidate> {
    let world_x = chunk_center_world(chunk_x);
    let world_z = chunk_center_world(chunk_z);
    let region = sample_region_classes(regions, world_x, world_z);
    if !requested_archetypes.is_empty() && !requested_archetypes.contains(&region.archetype) {
        return None;
    }

    let meso = sample_meso_guides(meso_guides, world_x, world_z);
    let archetype_def = region_archetype_def(region.archetype)?;
    let allowed_keys = archetype_def
        .allowed_meso_keys
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let interestingness = overall_meso_interestingness(meso);
    let mut requested_matches = Vec::with_capacity(requested_meso.len());
    let mut allowed_count = 0_usize;
    let mut runtime_count = 0_usize;
    let mut runtime_total = 0.0_f32;
    let mut runtime_peak = 0.0_f32;

    for requested in requested_meso {
        let allowed = allowed_keys.contains(requested.key.as_str());
        if allowed {
            allowed_count += 1;
        }

        let runtime_value = requested.runtime_channel.map(|channel| channel.value(meso));
        if let Some(value) = runtime_value {
            runtime_count += 1;
            runtime_total += value;
            runtime_peak = runtime_peak.max(value);
        }

        requested_matches.push(RequestedMesoMatch {
            key: requested.key.clone(),
            allowed_by_archetype: allowed,
            runtime_value,
        });
    }

    if !requested_meso.is_empty() && allowed_count == 0 {
        return None;
    }

    let allowed_fraction = if requested_meso.is_empty() {
        0.0
    } else {
        allowed_count as f32 / requested_meso.len() as f32
    };
    let runtime_average = if runtime_count == 0 {
        0.0
    } else {
        runtime_total / runtime_count as f32
    };
    let relief_bonus = relief_bonus(region);
    let score = interestingness
        + relief_bonus
        + allowed_fraction * 3.0
        + runtime_average * 2.5
        + runtime_peak * 1.5
        + if requested_archetypes.is_empty() { 0.0 } else { 2.0 };
    let atlas_span_blocks = atlas_cell_span_blocks();

    Some(Candidate {
        chunk_x,
        chunk_z,
        atlas_x: world_x.div_euclid(atlas_span_blocks),
        atlas_z: world_z.div_euclid(atlas_span_blocks),
        score,
        allowed_fraction,
        runtime_peak,
        interestingness,
        region,
        meso,
        requested_meso: requested_matches,
    })
}

fn push_top_candidate(candidates: &mut Vec<Candidate>, candidate: Candidate, retain_count: usize) {
    candidates.push(candidate);
    candidates.sort_by(compare_candidates);
    if candidates.len() > retain_count {
        candidates.truncate(retain_count);
    }
}

fn compare_candidates(left: &Candidate, right: &Candidate) -> Ordering {
    right
        .score
        .partial_cmp(&left.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| {
            right
                .runtime_peak
                .partial_cmp(&left.runtime_peak)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| {
            right
                .allowed_fraction
                .partial_cmp(&left.allowed_fraction)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| {
            right
                .interestingness
                .partial_cmp(&left.interestingness)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| left.chunk_z.cmp(&right.chunk_z))
        .then_with(|| left.chunk_x.cmp(&right.chunk_x))
}

fn resolve_requested_archetypes(values: &[String]) -> Result<Vec<RegionArchetype>, Box<dyn Error>> {
    let mut seen = HashSet::new();
    let mut resolved = Vec::new();

    for value in values {
        let key = canonical_key(value);
        let entry = region_catalog_entries()
            .iter()
            .find(|entry| archetype_key(entry.archetype) == key)
            .ok_or_else(|| {
                cli_error(format!(
                    "unknown archetype `{value}`. use --list-archetypes to inspect available keys"
                ))
            })?;
        if !matches!(entry.status, RegionCatalogStatus::LaunchCandidate) {
            return Err(cli_error(format!(
                "archetype `{}` is not a launch archetype. terrain_find currently searches the public launch-fallback region map only",
                archetype_key(entry.archetype)
            )));
        }

        if seen.insert(archetype_key(entry.archetype)) {
            resolved.push(entry.archetype);
        }
    }

    Ok(resolved)
}

fn resolve_requested_meso(values: &[String]) -> Result<Vec<RequestedMeso>, Box<dyn Error>> {
    let mut seen = HashSet::new();
    let mut resolved = Vec::new();

    for value in values {
        let key = canonical_key(value);
        let def = meso_feature_defs()
            .iter()
            .find(|def| canonical_key(def.key) == key)
            .ok_or_else(|| {
                cli_error(format!(
                    "unknown meso key `{value}`. use --list-meso to inspect available keys"
                ))
            })?;

        if seen.insert(def.key) {
            resolved.push(RequestedMeso {
                key: def.key.to_string(),
                runtime_channel: runtime_backing(def.key),
                summary: def.summary,
            });
        }
    }

    Ok(resolved)
}

fn render_preview_for_candidate(
    seed: u64,
    candidate: &Candidate,
    preview_radius: i32,
    preview_width: u32,
    preview_height: u32,
    preview_quarter_turns: u8,
    output: &Path,
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let status = Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--bin")
        .arg("chunk_preview")
        .arg("--")
        .arg(seed.to_string())
        .arg("--stage")
        .arg("prototype")
        .arg("--center-x")
        .arg(candidate.chunk_x.to_string())
        .arg("--center-z")
        .arg(candidate.chunk_z.to_string())
        .arg("--radius")
        .arg(preview_radius.to_string())
        .arg("--width")
        .arg(preview_width.to_string())
        .arg("--height")
        .arg(preview_height.to_string())
        .arg("--quarter-turns")
        .arg(preview_quarter_turns.to_string())
        .arg("--output")
        .arg(output)
        .status()?;

    if !status.success() {
        return Err(cli_error(
            "chunk_preview command failed while rendering the selected candidate",
        ));
    }

    Ok(())
}

fn preview_command_string(
    seed: u64,
    candidate: &Candidate,
    preview_radius: i32,
    preview_width: u32,
    preview_height: u32,
    preview_quarter_turns: u8,
    preview_output: Option<&Path>,
) -> String {
    let mut parts = vec![
        "cargo run --bin chunk_preview --".to_string(),
        seed.to_string(),
        "--stage".to_string(),
        "prototype".to_string(),
        "--center-x".to_string(),
        candidate.chunk_x.to_string(),
        "--center-z".to_string(),
        candidate.chunk_z.to_string(),
        "--radius".to_string(),
        preview_radius.to_string(),
        "--width".to_string(),
        preview_width.to_string(),
        "--height".to_string(),
        preview_height.to_string(),
        "--quarter-turns".to_string(),
        preview_quarter_turns.to_string(),
    ];
    if let Some(output) = preview_output {
        parts.push("--output".to_string());
        parts.push(shell_word(output));
    }
    parts.join(" ")
}

fn default_preview_output(
    seed: u64,
    candidate: &Candidate,
    requested_archetypes: &[RegionArchetype],
    requested_meso: &[RequestedMeso],
) -> PathBuf {
    let archetype_label = requested_archetypes
        .first()
        .map(|archetype| archetype_key(*archetype))
        .unwrap_or_else(|| "any".to_string());
    let meso_label = if requested_meso.is_empty() {
        "plain".to_string()
    } else {
        requested_meso
            .iter()
            .map(|meso| meso.key.clone())
            .collect::<Vec<_>>()
            .join("_")
    };

    PathBuf::from(format!(
        "target/terrain-find/seed_{seed}_{archetype_label}_{meso_label}_cx{}_cz{}.png",
        candidate.chunk_x, candidate.chunk_z
    ))
}

fn overall_meso_interestingness(sample: MesoGuideSample) -> f32 {
    sample.hilliness * 0.85
        + sample.basin_weight * 0.75
        + sample.escarpment_weight * 1.00
        + sample.terrace_weight * 0.90
}

fn relief_bonus(region: RegionClassSample) -> f32 {
    match enum_key(region.relief_class).as_str() {
        "plain" => 0.10,
        "rolling" => 0.18,
        "hill" => 0.32,
        "mountain" => 0.50,
        _ => 0.0,
    }
}

fn print_archetype_catalog() {
    println!("archetypes:");
    for entry in region_catalog_entries() {
        let key = archetype_key(entry.archetype);
        let status = match entry.status {
            RegionCatalogStatus::LaunchCandidate => "launch",
            RegionCatalogStatus::ExtendedCandidate => "extended",
            RegionCatalogStatus::Deferred => "deferred",
        };
        let summary = region_archetype_def(entry.archetype)
            .map(|def| def.summary)
            .unwrap_or("missing archetype summary");
        println!("  - {:<28} {:<8} {}", key, status, summary);
    }
    println!("note: terrain_find exact search currently supports launch archetypes only.");
}

fn print_meso_catalog() {
    println!("meso features:");
    for def in meso_feature_defs() {
        let mode = if runtime_backing(def.key).is_some() {
            "runtime-backed"
        } else {
            "planned-only"
        };
        println!("  - {:<24} {:<14} {}", def.key, mode, def.summary);
    }
    println!("note: current runtime-backed guide search keys are hill_cluster, shallow_basin, escarpment_band, upland_terrace.");
}

fn format_requested_meso_matches(matches: &[RequestedMesoMatch]) -> String {
    matches
        .iter()
        .map(|meso| {
            let runtime = match meso.runtime_value {
                Some(value) => format!("{value:.2}"),
                None => "n/a".to_string(),
            };
            format!(
                "{}[allowed={}, guide={}]",
                meso.key,
                yes_no(meso.allowed_by_archetype),
                runtime
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    if let Some(index) = args.iter().position(|arg| arg == flag) {
        args.remove(index);
        true
    } else {
        false
    }
}

fn runtime_backing(key: &str) -> Option<RuntimeMesoChannel> {
    match key {
        "hill_cluster" => Some(RuntimeMesoChannel::HillCluster),
        "shallow_basin" => Some(RuntimeMesoChannel::ShallowBasin),
        "escarpment_band" => Some(RuntimeMesoChannel::EscarpmentBand),
        "upland_terrace" => Some(RuntimeMesoChannel::UplandTerrace),
        _ => None,
    }
}

fn archetype_key(archetype: RegionArchetype) -> String {
    canonical_key(&format!("{archetype:?}"))
}

fn enum_key<T>(value: T) -> String
where
    T: Debug,
{
    canonical_key(&format!("{value:?}"))
}

fn canonical_key(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let mut out = String::new();

    for (index, ch) in chars.iter().copied().enumerate() {
        if matches!(ch, '-' | '_' | ' ' | '/') {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            continue;
        }

        if ch.is_ascii_uppercase() {
            if index > 0 {
                let previous = chars[index - 1];
                let next = chars.get(index + 1).copied();
                if !out.ends_with('_')
                    && ((previous.is_ascii_lowercase() || previous.is_ascii_digit())
                        || (previous.is_ascii_uppercase()
                            && next.map(|c| c.is_ascii_lowercase()).unwrap_or(false)))
                {
                    out.push('_');
                }
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch.to_ascii_lowercase());
        }
    }

    out.trim_matches('_').to_string()
}

fn chunk_center_world(chunk: i32) -> i32 {
    chunk * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2
}

fn atlas_cell_span_blocks() -> i32 {
    ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn shell_word(path: &Path) -> String {
    let value = path.display().to_string();
    if value.contains(' ') {
        format!("\"{value}\"")
    } else {
        value
    }
}

fn parse_required<T>(args: &mut Vec<String>, label: &str) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
{
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| cli_error(format!("missing {label}\n\n{}", usage())))?;
    args.remove(0);
    value
        .parse::<T>()
        .map_err(|error| cli_error(format!("invalid {label}: {error}")))
}

fn usage() -> &'static str {
    "usage:
  cargo run --bin terrain_find -- --list-archetypes
  cargo run --bin terrain_find -- --list-meso
  cargo run --bin terrain_find -- <seed> [--origin-cell-x <i32>] [--origin-cell-z <i32>] [--search-radius-cells <i32>] [--chunk-step <u32>] [--top <usize>] [--archetype <key>]... [--meso <key>]... [--preview-rank <usize>] [--preview-radius <i32>] [--preview-width <u32>] [--preview-height <u32>] [--preview-quarter-turns <u8>] [--preview-output <path>] [--render-preview]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_key_accepts_multiple_input_styles() {
        assert_eq!(canonical_key("TemperateHills"), "temperate_hills");
        assert_eq!(canonical_key("temperate-hills"), "temperate_hills");
        assert_eq!(canonical_key("temperate_hills"), "temperate_hills");
    }

    #[test]
    fn runtime_backing_marks_wave_one_keys() {
        assert_eq!(runtime_backing("hill_cluster"), Some(RuntimeMesoChannel::HillCluster));
        assert_eq!(runtime_backing("shallow_basin"), Some(RuntimeMesoChannel::ShallowBasin));
        assert_eq!(runtime_backing("ravine"), None);
    }

    #[test]
    fn non_launch_archetype_is_rejected() {
        let error = resolve_requested_archetypes(&[String::from("boreal_hills")]).unwrap_err();
        assert!(format!("{error}").contains("launch archetype"));
    }
}
