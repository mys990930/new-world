use super::types::{MacroFieldTile, MacroFieldTileConfig};
use crate::world::generation::graph::WorldPlanePoint;
pub const MACRO_FIELD_CONTOUR_NORMALIZED_MIN: f32 = -0.5;
pub const MACRO_FIELD_CONTOUR_NORMALIZED_MAX: f32 = 1.0;
pub const MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS: f32 = -1024.0;
pub const MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS: f32 = 2048.0;
pub const DEFAULT_MACRO_FIELD_CONTOUR_STEP_BLOCKS: f32 = 32.0;
pub const DEFAULT_MACRO_FIELD_CONTOUR_MAJOR_EVERY: u32 = 5;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroFieldContourSegment {
    pub start: WorldPlanePoint,
    pub end: WorldPlanePoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MacroFieldContourLevel {
    pub height_blocks: f32,
    pub is_major: bool,
    pub segments: Vec<MacroFieldContourSegment>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MacroFieldContourSet {
    pub step_blocks: f32,
    pub major_every: u32,
    pub min_level_blocks: f32,
    pub max_level_blocks: f32,
    pub total_segment_count: usize,
    pub levels: Vec<MacroFieldContourLevel>,
}

pub fn combined_macro_height_to_blocks(value: f32) -> f32 {
    if value >= 0.0 {
        let t = (value / MACRO_FIELD_CONTOUR_NORMALIZED_MAX.max(f32::EPSILON)).clamp(0.0, 1.0);
        t * MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS
    } else {
        let t = (value / MACRO_FIELD_CONTOUR_NORMALIZED_MIN.min(-f32::EPSILON)).clamp(0.0, 1.0);
        t * MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS
    }
}

pub fn extract_macro_field_contours(
    tile: &MacroFieldTile,
    step_blocks: f32,
    major_every: u32,
) -> MacroFieldContourSet {
    assert!(
        step_blocks.is_finite() && step_blocks > 0.0,
        "contour step must be finite and positive"
    );
    let major_every = major_every.max(1);
    let width = tile.config.width as usize;
    let height = tile.config.height as usize;
    if width < 2 || height < 2 || tile.samples.len() != width * height {
        return MacroFieldContourSet {
            step_blocks,
            major_every,
            min_level_blocks: 0.0,
            max_level_blocks: 0.0,
            total_segment_count: 0,
            levels: Vec::new(),
        };
    }

    let mut min_height = f32::INFINITY;
    let mut max_height = f32::NEG_INFINITY;
    let heights = tile
        .samples
        .iter()
        .map(|sample| {
            let height = combined_macro_height_to_blocks(sample.combined_macro_height);
            min_height = min_height.min(height);
            max_height = max_height.max(height);
            height
        })
        .collect::<Vec<_>>();
    if !min_height.is_finite()
        || !max_height.is_finite()
        || (max_height - min_height).abs() <= f32::EPSILON
    {
        return MacroFieldContourSet {
            step_blocks,
            major_every,
            min_level_blocks: min_height,
            max_level_blocks: max_height,
            total_segment_count: 0,
            levels: Vec::new(),
        };
    }

    let first_level = (min_height / step_blocks).ceil() as i32;
    let last_level = (max_height / step_blocks).floor() as i32;
    let mut levels = Vec::new();
    let mut total_segment_count = 0;
    for level_index in first_level..=last_level {
        let height_blocks = level_index as f32 * step_blocks;
        let mut segments = Vec::new();
        for z in 0..height - 1 {
            for x in 0..width - 1 {
                append_contour_cell_segments(
                    &mut segments,
                    &heights,
                    tile.config,
                    width,
                    x,
                    z,
                    height_blocks,
                );
            }
        }
        total_segment_count += segments.len();
        levels.push(MacroFieldContourLevel {
            height_blocks,
            is_major: level_index.rem_euclid(major_every as i32) == 0,
            segments,
        });
    }

    MacroFieldContourSet {
        step_blocks,
        major_every,
        min_level_blocks: first_level as f32 * step_blocks,
        max_level_blocks: last_level as f32 * step_blocks,
        total_segment_count,
        levels,
    }
}
pub(super) fn append_contour_cell_segments(
    output: &mut Vec<MacroFieldContourSegment>,
    heights: &[f32],
    config: MacroFieldTileConfig,
    width: usize,
    x: usize,
    z: usize,
    level: f32,
) {
    let i00 = z * width + x;
    let i10 = z * width + x + 1;
    let i11 = (z + 1) * width + x + 1;
    let i01 = (z + 1) * width + x;
    let p00 = config.sample_position(i00);
    let p10 = config.sample_position(i10);
    let p11 = config.sample_position(i11);
    let p01 = config.sample_position(i01);
    let mut points = Vec::with_capacity(4);

    if let Some(point) = contour_edge_intersection(p00, heights[i00], p10, heights[i10], level) {
        points.push(point);
    }
    if let Some(point) = contour_edge_intersection(p10, heights[i10], p11, heights[i11], level) {
        points.push(point);
    }
    if let Some(point) = contour_edge_intersection(p11, heights[i11], p01, heights[i01], level) {
        points.push(point);
    }
    if let Some(point) = contour_edge_intersection(p01, heights[i01], p00, heights[i00], level) {
        points.push(point);
    }

    match points.len() {
        2 => output.push(MacroFieldContourSegment {
            start: points[0],
            end: points[1],
        }),
        4 => {
            output.push(MacroFieldContourSegment {
                start: points[0],
                end: points[1],
            });
            output.push(MacroFieldContourSegment {
                start: points[2],
                end: points[3],
            });
        }
        _ => {}
    }
}

pub(super) fn contour_edge_intersection(
    start: WorldPlanePoint,
    start_height: f32,
    end: WorldPlanePoint,
    end_height: f32,
    level: f32,
) -> Option<WorldPlanePoint> {
    if !start_height.is_finite() || !end_height.is_finite() {
        return None;
    }
    let delta = end_height - start_height;
    if delta.abs() <= f32::EPSILON {
        return None;
    }
    let t = (level - start_height) / delta;
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    Some(WorldPlanePoint::new(
        start.x + (end.x - start.x) * t,
        start.z + (end.z - start.z) * t,
    ))
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::world::generation::boundary::{
        BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
    };
    use crate::world::generation::graph::{
        VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId, WorldPlanePoint,
    };
    use crate::world::generation::macro_field::test_support::*;
    use crate::world::generation::macro_field::{
        MacroFieldRasterContext, MacroFieldTileConfig, generate_macro_field_tile,
        sample_macro_field_point,
    };
    use crate::world::generation::macro_map::{
        GraphMacroMap, MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
    };
    use crate::world::generation::river_plan::{
        DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS, RiverPlan,
    };

    #[test]
    fn contour_extraction_is_deterministic_and_finite() {
        let tile = test_contour_tile(&[0.0, 16.0, 8.0, 24.0], 2, 2);

        let first = extract_macro_field_contours(&tile, 8.0, 5);
        let second = extract_macro_field_contours(&tile, 8.0, 5);

        assert_eq!(first, second);
        assert!(first.total_segment_count > 0);
        assert!(first.levels.iter().all(|level| {
            level.height_blocks.is_finite()
                && level.segments.iter().all(|segment| {
                    segment.start.x.is_finite()
                        && segment.start.z.is_finite()
                        && segment.end.x.is_finite()
                        && segment.end.z.is_finite()
                })
        }));
    }

    #[test]
    fn contour_block_scale_keeps_signed_zero_at_sea_level() {
        assert_eq!(combined_macro_height_to_blocks(0.0), 0.0);
        assert_eq!(
            combined_macro_height_to_blocks(MACRO_FIELD_CONTOUR_NORMALIZED_MIN),
            MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS
        );
        assert_eq!(
            combined_macro_height_to_blocks(MACRO_FIELD_CONTOUR_NORMALIZED_MAX),
            MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS
        );
    }

    #[test]
    fn contour_block_scale_uses_experimental_large_block_domain() {
        assert_eq!(MACRO_FIELD_CONTOUR_NORMALIZED_MIN, -0.5);
        assert_eq!(MACRO_FIELD_CONTOUR_NORMALIZED_MAX, 1.0);
        assert_eq!(MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS, -1024.0);
        assert_eq!(MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS, 2048.0);
        assert_eq!(combined_macro_height_to_blocks(-0.25), -512.0);
        assert_eq!(combined_macro_height_to_blocks(0.75), 1536.0);
    }

    #[test]
    fn contour_block_scale_saturates_outside_effective_range() {
        assert_eq!(combined_macro_height_to_blocks(-0.75), -1024.0);
        assert_eq!(combined_macro_height_to_blocks(1.25), 2048.0);
    }

    #[test]
    fn default_contour_preview_step_is_practical_for_large_block_domain() {
        assert_eq!(DEFAULT_MACRO_FIELD_CONTOUR_STEP_BLOCKS, 32.0);
    }

    #[test]
    fn simple_ramp_field_produces_contour_crossing() {
        let tile = test_contour_tile(&[0.0, 128.0, 0.0, 128.0], 2, 2);

        let contours =
            extract_macro_field_contours(&tile, DEFAULT_MACRO_FIELD_CONTOUR_STEP_BLOCKS, 5);

        let level = contours
            .levels
            .iter()
            .find(|level| {
                (level.height_blocks - DEFAULT_MACRO_FIELD_CONTOUR_STEP_BLOCKS).abs()
                    <= f32::EPSILON
            })
            .expect("ramp should produce a default-step contour");
        assert_eq!(level.segments.len(), 1);
        assert!(
            (level.segments[0].start.x - 16.0).abs() <= 0.01
                || (level.segments[0].end.x - 16.0).abs() <= 0.01,
            "default 32-block contour should cross one quarter across a 64-block sample cell: {:?}",
            level.segments[0]
        );
    }

    #[test]
    fn flat_field_produces_no_contours() {
        let tile = test_contour_tile(&[12.0, 12.0, 12.0, 12.0], 2, 2);

        let contours = extract_macro_field_contours(&tile, 8.0, 5);

        assert_eq!(contours.total_segment_count, 0);
        assert!(contours.levels.is_empty());
    }
}
