use bytemuck::{Pod, Zeroable};

use super::CameraProjectionConfig;

pub type Matrix4 = [[f32; 4]; 4];

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_projection: Matrix4,
}

impl CameraUniform {
    pub fn from_view_projection(view_projection: Matrix4) -> Self {
        Self {
            // The CPU camera math stores matrices row-major.
            // WGSL uniforms are consumed as column-major matrices.
            view_projection: transpose_matrix4(view_projection),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderCameraState {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub aspect_override: Option<f32>,
}

impl Default for RenderCameraState {
    fn default() -> Self {
        Self {
            eye: [16.0, 16.0, 16.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            aspect_override: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraGpuState {
    pub view: Matrix4,
    pub projection: Matrix4,
    pub view_projection: Matrix4,
    pub cached_aspect_ratio: f32,
    pub last_uploaded_frame: Option<u64>,
}

impl Default for CameraGpuState {
    fn default() -> Self {
        Self {
            view: identity_matrix(),
            projection: identity_matrix(),
            view_projection: identity_matrix(),
            cached_aspect_ratio: 1.0,
            last_uploaded_frame: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraUpdateError {
    InvalidProjectionConfig,
    DegenerateView,
}

impl CameraGpuState {
    pub(crate) fn new(
        projection: &CameraProjectionConfig,
        width: u32,
        height: u32,
    ) -> Result<Self, CameraUpdateError> {
        let mut state = Self::default();
        state.handle_resize(width, height);
        state.update(&RenderCameraState::default(), projection, width, height, 0)?;
        Ok(state)
    }

    pub(crate) fn handle_resize(&mut self, width: u32, height: u32) {
        self.cached_aspect_ratio = aspect_ratio_from_size(width, height);
    }

    pub(crate) fn update(
        &mut self,
        camera: &RenderCameraState,
        projection: &CameraProjectionConfig,
        width: u32,
        height: u32,
        frame_index: u64,
    ) -> Result<(), CameraUpdateError> {
        if projection.near_plane <= 0.0
            || projection.far_plane <= projection.near_plane
            || projection.vertical_fov_radians <= 0.0
        {
            return Err(CameraUpdateError::InvalidProjectionConfig);
        }

        let aspect_ratio = camera
            .aspect_override
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or_else(|| aspect_ratio_from_size(width, height));

        let view = look_at_rh(camera.eye, camera.target, camera.up)?;
        let projection = perspective_rh(
            projection.vertical_fov_radians,
            aspect_ratio,
            projection.near_plane,
            projection.far_plane,
        )?;

        self.view = view;
        self.projection = projection;
        self.view_projection = multiply_matrix4(projection, view);
        self.cached_aspect_ratio = aspect_ratio;
        self.last_uploaded_frame = Some(frame_index);
        Ok(())
    }
}

pub(crate) fn identity_matrix() -> Matrix4 {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn aspect_ratio_from_size(width: u32, height: u32) -> f32 {
    if width == 0 || height == 0 {
        1.0
    } else {
        width as f32 / height as f32
    }
}

fn look_at_rh(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> Result<Matrix4, CameraUpdateError> {
    let forward = normalize3(subtract3(target, eye)).ok_or(CameraUpdateError::DegenerateView)?;
    let right = normalize3(cross3(forward, up)).ok_or(CameraUpdateError::DegenerateView)?;
    let recalculated_up = cross3(right, forward);

    Ok([
        [right[0], recalculated_up[0], -forward[0], 0.0],
        [right[1], recalculated_up[1], -forward[1], 0.0],
        [right[2], recalculated_up[2], -forward[2], 0.0],
        [
            -dot3(right, eye),
            -dot3(recalculated_up, eye),
            dot3(forward, eye),
            1.0,
        ],
    ])
}

fn perspective_rh(
    vertical_fov_radians: f32,
    aspect_ratio: f32,
    near_plane: f32,
    far_plane: f32,
) -> Result<Matrix4, CameraUpdateError> {
    if vertical_fov_radians <= 0.0
        || aspect_ratio <= 0.0
        || near_plane <= 0.0
        || far_plane <= near_plane
    {
        return Err(CameraUpdateError::InvalidProjectionConfig);
    }

    let focal_length = 1.0 / (vertical_fov_radians * 0.5).tan();
    Ok([
        [focal_length / aspect_ratio, 0.0, 0.0, 0.0],
        [0.0, focal_length, 0.0, 0.0],
        [0.0, 0.0, far_plane / (near_plane - far_plane), -1.0],
        [0.0, 0.0, (near_plane * far_plane) / (near_plane - far_plane), 0.0],
    ])
}

fn multiply_matrix4(left: Matrix4, right: Matrix4) -> Matrix4 {
    let mut result = [[0.0; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            result[row][col] = (0..4).map(|idx| left[row][idx] * right[idx][col]).sum();
        }
    }
    result
}

fn subtract3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn normalize3(value: [f32; 3]) -> Option<[f32; 3]> {
    let length_sq = dot3(value, value);
    if length_sq <= f32::EPSILON {
        return None;
    }

    let inv_length = length_sq.sqrt().recip();
    Some([
        value[0] * inv_length,
        value[1] * inv_length,
        value[2] * inv_length,
    ])
}

fn cross3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn transpose_matrix4(matrix: Matrix4) -> Matrix4 {
    let mut result = [[0.0; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            result[row][col] = matrix[col][row];
        }
    }
    result
}
