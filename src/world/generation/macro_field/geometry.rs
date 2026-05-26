use super::context::{MACRO_FIELD_CURVE_BUCKET_BLOCKS, MACRO_FIELD_SITE_BUCKET_BLOCKS};
use crate::world::generation::graph::WorldPlanePoint;

#[derive(Debug, Clone, Copy)]
pub(super) struct NearestPolylineSegment {
    pub(super) start: WorldPlanePoint,
    pub(super) end: WorldPlanePoint,
    pub(super) distance: f32,
}

pub(super) fn polyline_distance(position: WorldPlanePoint, points: &[WorldPlanePoint]) -> f32 {
    let distance_squared = match points {
        [] => f32::INFINITY,
        [point] => squared_distance(position, *point),
        _ => points
            .windows(2)
            .map(|segment| point_segment_distance_squared(position, segment[0], segment[1]))
            .fold(f32::INFINITY, f32::min),
    };
    distance_squared.sqrt()
}

pub(super) fn nearest_polyline_segment(
    position: WorldPlanePoint,
    points: &[WorldPlanePoint],
) -> Option<NearestPolylineSegment> {
    match points {
        [] | [_] => None,
        _ => points
            .windows(2)
            .map(|segment| {
                let distance_squared =
                    point_segment_distance_squared(position, segment[0], segment[1]);
                (segment, distance_squared)
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(segment, distance_squared)| NearestPolylineSegment {
                start: segment[0],
                end: segment[1],
                distance: distance_squared.sqrt(),
            }),
    }
}

fn point_segment_distance_squared(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> f32 {
    let projected = nearest_point_on_segment(point, start, end);
    squared_distance(point, projected)
}

fn nearest_point_on_segment(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> WorldPlanePoint {
    let t = projected_t_on_segment(point, start, end);
    WorldPlanePoint::new(
        start.x + (end.x - start.x) * t,
        start.z + (end.z - start.z) * t,
    )
}

fn projected_t_on_segment(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> f32 {
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    let len2 = dx * dx + dz * dz;
    if len2 <= f32::EPSILON {
        return 0.0;
    }
    (((point.x - start.x) * dx + (point.z - start.z) * dz) / len2).clamp(0.0, 1.0)
}

pub(super) fn squared_distance(a: WorldPlanePoint, b: WorldPlanePoint) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz
}

pub(super) fn curve_bucket(point: WorldPlanePoint) -> (i32, i32) {
    (
        (point.x / MACRO_FIELD_CURVE_BUCKET_BLOCKS).floor() as i32,
        (point.z / MACRO_FIELD_CURVE_BUCKET_BLOCKS).floor() as i32,
    )
}

pub(super) fn curve_bucket_search_radius(radius: f32) -> i32 {
    ((radius + MACRO_FIELD_CURVE_BUCKET_BLOCKS * 0.5) / MACRO_FIELD_CURVE_BUCKET_BLOCKS).ceil()
        as i32
}

pub(super) fn site_bucket(point: WorldPlanePoint) -> (i32, i32) {
    (
        (point.x / MACRO_FIELD_SITE_BUCKET_BLOCKS).floor() as i32,
        (point.z / MACRO_FIELD_SITE_BUCKET_BLOCKS).floor() as i32,
    )
}

pub(super) fn signed_side(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> f32 {
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    (point.x - start.x) * dz - (point.z - start.z) * dx
}

pub(super) fn usable_side(preferred: f32, fallback: f32) -> f32 {
    if preferred.abs() > f32::EPSILON {
        preferred
    } else if fallback.abs() > f32::EPSILON {
        -fallback
    } else {
        1.0
    }
}
