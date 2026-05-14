use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use new_world::world::CHUNK_EDGE_I32;

const DEFAULT_CENTER_CHUNK_X: i32 = 0;
const DEFAULT_CENTER_CHUNK_Z: i32 = 0;
const DEFAULT_RADIUS: i32 = 8;
const DEFAULT_OVERVIEW_WIDTH: u32 = 3840;
const DEFAULT_OVERVIEW_HEIGHT: u32 = 2160;
const DEFAULT_ZOOM_WIDTH: u32 = 1280;
const DEFAULT_ZOOM_HEIGHT: u32 = 720;
const DEFAULT_HEIGHTFIELD_WIDTH: u32 = 1280;
const DEFAULT_HEIGHTFIELD_HEIGHT: u32 = 720;
const DEFAULT_CONTOUR_STEP_BLOCKS: i32 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SuiteConfig {
    seed: u64,
    center_chunk_x: i32,
    center_chunk_z: i32,
    radius: i32,
    output: PathBuf,
    overview_width: u32,
    overview_height: u32,
    zoom_width: u32,
    zoom_height: u32,
    heightfield_width: u32,
    heightfield_height: u32,
    contour_step_blocks: i32,
}

impl SuiteConfig {
    fn validate(self) -> Result<Self, Box<dyn Error>> {
        if self.radius < 0 {
            return Err(cli_error("radius must be non-negative"));
        }
        if self.overview_width == 0
            || self.overview_height == 0
            || self.zoom_width == 0
            || self.zoom_height == 0
            || self.heightfield_width == 0
            || self.heightfield_height == 0
        {
            return Err(cli_error("preview dimensions must be positive"));
        }
        if self.contour_step_blocks <= 0 {
            return Err(cli_error("contour-step must be positive"));
        }
        Ok(self)
    }

    fn center_world_x(&self) -> i32 {
        self.center_chunk_x * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2
    }

    fn center_world_z(&self) -> i32 {
        self.center_chunk_z * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreviewStep {
    ordinal: u8,
    label: &'static str,
    binary: &'static str,
    output_file: &'static str,
    args: Vec<String>,
}

impl PreviewStep {
    fn output_path(&self, root: &Path) -> PathBuf {
        root.join(self.output_file)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?.validate()?;
    fs::create_dir_all(&config.output)?;

    let steps = preview_steps(&config);
    let total_start = Instant::now();
    println!("generation preview suite");
    println!("seed: {}", config.seed);
    println!(
        "center chunk: ({}, {}), center world: ({}, {}), radius: {}",
        config.center_chunk_x,
        config.center_chunk_z,
        config.center_world_x(),
        config.center_world_z(),
        config.radius
    );
    println!("output: {}", config.output.display());

    for step in &steps {
        run_preview_step(step, &config.output)?;
    }

    println!(
        "suite complete in {:.3}s",
        total_start.elapsed().as_secs_f64()
    );
    for step in &steps {
        println!(
            "{:02}: {}",
            step.ordinal,
            step.output_path(&config.output).display()
        );
    }

    Ok(())
}

fn preview_steps(config: &SuiteConfig) -> Vec<PreviewStep> {
    let seed = config.seed.to_string();
    let world_x = config.center_world_x().to_string();
    let world_z = config.center_world_z().to_string();
    let chunk_x = config.center_chunk_x.to_string();
    let chunk_z = config.center_chunk_z.to_string();
    let radius = config.radius.to_string();
    let macro_field_radius = config.radius.max(1).to_string();
    let overview_width = config.overview_width.to_string();
    let overview_height = config.overview_height.to_string();
    let zoom_width = config.zoom_width.to_string();
    let zoom_height = config.zoom_height.to_string();
    let heightfield_width = config.heightfield_width.to_string();
    let heightfield_height = config.heightfield_height.to_string();
    let contour_step = config.contour_step_blocks.to_string();

    vec![
        PreviewStep {
            ordinal: 1,
            label: "graph continentality",
            binary: "graph_voronoi_preview",
            output_file: "01_graph_cont.png",
            args: vec![
                seed.clone(),
                world_x.clone(),
                world_z.clone(),
                "--mode".into(),
                "continentality".into(),
                "--width".into(),
                overview_width.clone(),
                "--height".into(),
                overview_height.clone(),
            ],
        },
        PreviewStep {
            ordinal: 2,
            label: "graph elevation",
            binary: "graph_voronoi_preview",
            output_file: "02_graph_elev.png",
            args: vec![
                seed.clone(),
                world_x.clone(),
                world_z.clone(),
                "--mode".into(),
                "elevation".into(),
                "--width".into(),
                overview_width.clone(),
                "--height".into(),
                overview_height.clone(),
            ],
        },
        PreviewStep {
            ordinal: 3,
            label: "macro map",
            binary: "macro_map_preview",
            output_file: "03_macro_map.png",
            args: vec![
                seed.clone(),
                world_x.clone(),
                world_z.clone(),
                "--width".into(),
                overview_width.clone(),
                "--height".into(),
                overview_height.clone(),
            ],
        },
        PreviewStep {
            ordinal: 4,
            label: "biome map",
            binary: "biome_map_preview",
            output_file: "04_biome_map.png",
            args: vec![
                seed.clone(),
                world_x.clone(),
                world_z.clone(),
                "--width".into(),
                overview_width.clone(),
                "--height".into(),
                overview_height.clone(),
            ],
        },
        PreviewStep {
            ordinal: 5,
            label: "macro field combined contour overview",
            binary: "macro_field_preview",
            output_file: "05_macro_combined.png",
            args: vec![
                seed.clone(),
                world_x.clone(),
                world_z.clone(),
                "--channel".into(),
                "combined".into(),
                "--contours".into(),
                "--contour-step".into(),
                contour_step.clone(),
                "--width".into(),
                overview_width,
                "--height".into(),
                overview_height,
            ],
        },
        PreviewStep {
            ordinal: 6,
            label: "macro field combined zoom",
            binary: "macro_field_preview",
            output_file: "06_macro_zoom.png",
            args: vec![
                seed.clone(),
                world_x,
                world_z,
                "--channel".into(),
                "combined".into(),
                "--contours".into(),
                "--contour-step".into(),
                contour_step,
                "--chunk-radius".into(),
                macro_field_radius,
                "--width".into(),
                zoom_width.clone(),
                "--height".into(),
                zoom_height.clone(),
            ],
        },
        PreviewStep {
            ordinal: 7,
            label: "pixelize",
            binary: "pixelize_preview",
            output_file: "07_pixelize.png",
            args: vec![
                seed.clone(),
                chunk_x.clone(),
                chunk_z.clone(),
                radius.clone(),
                "--width".into(),
                zoom_width,
                "--height".into(),
                zoom_height,
            ],
        },
        PreviewStep {
            ordinal: 8,
            label: "heightfield",
            binary: "heightfield_preview",
            output_file: "08_heightfield.png",
            args: vec![
                seed,
                chunk_x,
                chunk_z,
                "--chunk-radius".into(),
                radius,
                "--width".into(),
                heightfield_width,
                "--height".into(),
                heightfield_height,
            ],
        },
    ]
}

fn run_preview_step(step: &PreviewStep, output_root: &Path) -> Result<(), Box<dyn Error>> {
    let output = step.output_path(output_root);
    let mut args = step.args.clone();
    args.push("--output".into());
    args.push(output.to_string_lossy().into_owned());

    let command = resolve_child_command(step.binary, &args);
    println!(
        "[{:02}] {} -> {}",
        step.ordinal,
        step.label,
        output.display()
    );
    println!("     {}", command.display());

    let started = Instant::now();
    let status = command.spawn()?.wait()?;
    let elapsed = started.elapsed().as_secs_f64();
    if !status.success() {
        return Err(cli_error(format!(
            "preview step {:02} `{}` failed with status {status}",
            step.ordinal, step.binary
        )));
    }
    println!("     done in {elapsed:.3}s");
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChildCommand {
    program: PathBuf,
    args: Vec<OsString>,
}

impl ChildCommand {
    fn spawn(&self) -> io::Result<std::process::Child> {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        command.spawn()
    }

    fn display(&self) -> String {
        let mut parts = Vec::with_capacity(self.args.len() + 1);
        parts.push(self.program.display().to_string());
        parts.extend(
            self.args
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned()),
        );
        parts.join(" ")
    }
}

fn resolve_child_command(binary: &str, args: &[String]) -> ChildCommand {
    if let Some(path) = sibling_binary_path(binary).filter(|path| path.exists()) {
        return ChildCommand {
            program: path,
            args: args.iter().map(OsString::from).collect(),
        };
    }

    let mut cargo_args = vec![
        OsString::from("run"),
        OsString::from("--bin"),
        OsString::from(binary),
        OsString::from("--"),
    ];
    cargo_args.extend(args.iter().map(OsString::from));

    ChildCommand {
        program: PathBuf::from("cargo"),
        args: cargo_args,
    }
}

fn sibling_binary_path(binary: &str) -> Option<PathBuf> {
    let mut path = env::current_exe().ok()?;
    path.pop();
    path.push(format!("{binary}{}", env::consts::EXE_SUFFIX));
    Some(path)
}

fn parse_args() -> Result<SuiteConfig, Box<dyn Error>> {
    parse_args_from(env::args().skip(1))
}

fn parse_args_from<I, S>(args: I) -> Result<SuiteConfig, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(cli_error(usage()));
    }

    let seed = parse_required::<u64>(&mut args, "seed")?;
    let mut center_chunk_x = DEFAULT_CENTER_CHUNK_X;
    let mut center_chunk_z = DEFAULT_CENTER_CHUNK_Z;
    let mut radius = DEFAULT_RADIUS;
    let mut output: Option<PathBuf> = None;
    let mut overview_width = DEFAULT_OVERVIEW_WIDTH;
    let mut overview_height = DEFAULT_OVERVIEW_HEIGHT;
    let mut zoom_width = DEFAULT_ZOOM_WIDTH;
    let mut zoom_height = DEFAULT_ZOOM_HEIGHT;
    let mut heightfield_width = DEFAULT_HEIGHTFIELD_WIDTH;
    let mut heightfield_height = DEFAULT_HEIGHTFIELD_HEIGHT;
    let mut contour_step_blocks = DEFAULT_CONTOUR_STEP_BLOCKS;

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--center-chunk-x" | "--cx" => {
                center_chunk_x = parse_required::<i32>(&mut args, "center-chunk-x")?
            }
            "--center-chunk-z" | "--cz" => {
                center_chunk_z = parse_required::<i32>(&mut args, "center-chunk-z")?
            }
            "--radius" | "--r" => radius = parse_required::<i32>(&mut args, "radius")?,
            "--output" => {
                output = Some(PathBuf::from(parse_required::<String>(
                    &mut args, "output",
                )?))
            }
            "--overview-width" => {
                overview_width = parse_required::<u32>(&mut args, "overview-width")?
            }
            "--overview-height" => {
                overview_height = parse_required::<u32>(&mut args, "overview-height")?
            }
            "--zoom-width" => zoom_width = parse_required::<u32>(&mut args, "zoom-width")?,
            "--zoom-height" => zoom_height = parse_required::<u32>(&mut args, "zoom-height")?,
            "--heightfield-width" => {
                heightfield_width = parse_required::<u32>(&mut args, "heightfield-width")?
            }
            "--heightfield-height" => {
                heightfield_height = parse_required::<u32>(&mut args, "heightfield-height")?
            }
            "--contour-step" => {
                contour_step_blocks = parse_required::<i32>(&mut args, "contour-step")?
            }
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    let output = output.unwrap_or_else(|| {
        PathBuf::from(format!(
            "target/generation-preview-suite/s{seed}_cx{center_chunk_x}_cz{center_chunk_z}_r{radius}"
        ))
    });

    Ok(SuiteConfig {
        seed,
        center_chunk_x,
        center_chunk_z,
        radius,
        output,
        overview_width,
        overview_height,
        zoom_width,
        zoom_height,
        heightfield_width,
        heightfield_height,
        contour_step_blocks,
    })
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
    "usage: cargo run --bin generation_preview_suite -- <seed> [--center-chunk-x <i32>] [--center-chunk-z <i32>] [--radius <non-negative i32>] [--output <path>] [--overview-width <u32>] [--overview-height <u32>] [--zoom-width <u32>] [--zoom-height <u32>] [--heightfield-width <u32>] [--heightfield-height <u32>] [--contour-step <i32>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults_and_convert_chunk_center_to_world_center() {
        let config = parse_args_from(["42"]).expect("config").validate().unwrap();

        assert_eq!(config.seed, 42);
        assert_eq!(config.center_chunk_x, 0);
        assert_eq!(config.center_chunk_z, 0);
        assert_eq!(config.radius, DEFAULT_RADIUS);
        assert_eq!(config.center_world_x(), 16);
        assert_eq!(config.center_world_z(), 16);
    }

    #[test]
    fn preview_steps_use_ordered_outputs_and_expected_coordinate_units() {
        let config = parse_args_from([
            "42",
            "--center-chunk-x",
            "-70",
            "--center-chunk-z",
            "0",
            "--radius",
            "8",
            "--overview-width",
            "320",
            "--overview-height",
            "180",
        ])
        .expect("config")
        .validate()
        .unwrap();
        let steps = preview_steps(&config);

        assert_eq!(steps.len(), 8);
        assert_eq!(steps[0].output_file, "01_graph_cont.png");
        assert_eq!(steps[1].output_file, "02_graph_elev.png");
        assert_eq!(steps[2].output_file, "03_macro_map.png");
        assert_eq!(steps[3].binary, "biome_map_preview");
        assert_eq!(steps[3].output_file, "04_biome_map.png");
        assert_eq!(steps[4].output_file, "05_macro_combined.png");
        assert_eq!(steps[7].output_file, "08_heightfield.png");
        assert_eq!(steps[0].args[1], "-2224");
        assert_eq!(steps[0].args[2], "16");
        assert_eq!(steps[3].args[1], "-2224");
        assert_eq!(steps[3].args[2], "16");
        assert_eq!(steps[6].args[1], "-70");
        assert_eq!(steps[6].args[2], "0");
        assert_eq!(steps[6].args[3], "8");
        assert!(
            steps[4]
                .args
                .windows(2)
                .any(|pair| pair == ["--contour-step", "8"])
        );
    }

    #[test]
    fn radius_zero_keeps_pixelize_center_chunk_and_gives_macro_field_a_nonzero_span() {
        let config = parse_args_from(["42", "--radius", "0"])
            .expect("config")
            .validate()
            .unwrap();
        let steps = preview_steps(&config);

        assert!(
            steps[5]
                .args
                .windows(2)
                .any(|pair| pair == ["--chunk-radius", "1"])
        );
        assert_eq!(steps[6].args[3], "0");
        assert!(
            steps[7]
                .args
                .windows(2)
                .any(|pair| pair == ["--chunk-radius", "0"])
        );
    }
}
