use bytemuck::{Pod, Zeroable};

use super::CameraProjectionConfig;

pub type Matrix4 = [[f32; 4]; 4];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderProjectionMode {
    Perspective,
    Orthographic { vertical_world_size: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderViewBasis {
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_projection: Matrix4,
    pub eye_position: [f32; 4],
}

impl CameraUniform {
    pub fn from_view_projection_and_eye(view_projection: Matrix4, eye_position: [f32; 3]) -> Self {
        Self {
            view_projection,
            eye_position: [eye_position[0], eye_position[1], eye_position[2], 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderCameraState {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub aspect_override: Option<f32>,
    pub projection_mode: RenderProjectionMode,
    pub basis_override: Option<RenderViewBasis>,
}

impl Default for RenderCameraState {
    fn default() -> Self {
        Self {
            eye: [16.0, 16.0, 16.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            aspect_override: None,
            projection_mode: RenderProjectionMode::Perspective,
            basis_override: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraGpuState {
    pub view: Matrix4,
    pub projection: Matrix4,
    pub view_projection: Matrix4,
    pub eye_position: [f32; 3],
    pub cached_aspect_ratio: f32,
    pub last_uploaded_frame: Option<u64>,
}

impl Default for CameraGpuState {
    fn default() -> Self {
        Self {
            view: identity_matrix(),
            projection: identity_matrix(),
            view_projection: identity_matrix(),
            eye_position: [0.0, 0.0, 0.0],
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

        let view = match camera.basis_override {
            Some(basis) => view_from_basis(camera.eye, basis)?,
            None => look_at_rh(camera.eye, camera.target, camera.up)?,
        };
        let projection = match camera.projection_mode {
            RenderProjectionMode::Perspective => perspective_rh(
                projection.vertical_fov_radians,
                aspect_ratio,
                projection.near_plane,
                projection.far_plane,
            )?,
            RenderProjectionMode::Orthographic {
                vertical_world_size,
            } => orthographic_rh(
                aspect_ratio,
                vertical_world_size,
                projection.near_plane,
                projection.far_plane,
            )?,
        };

        self.view = view;
        self.projection = projection;
        // The CPU-side math in this module is expressed as row-major matrices
        // multiplied by row vectors, so the composed order is view * projection.
        self.view_projection = multiply_matrix4(view, projection);
        self.eye_position = camera.eye;
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

fn view_from_basis(eye: [f32; 3], basis: RenderViewBasis) -> Result<Matrix4, CameraUpdateError> {
    let right = normalize3(basis.right).ok_or(CameraUpdateError::DegenerateView)?;
    let up = normalize3(basis.up).ok_or(CameraUpdateError::DegenerateView)?;
    let forward = normalize3(basis.forward).ok_or(CameraUpdateError::DegenerateView)?;

    Ok([
        [right[0], up[0], -forward[0], 0.0],
        [right[1], up[1], -forward[1], 0.0],
        [right[2], up[2], -forward[2], 0.0],
        [
            -dot3(right, eye),
            -dot3(up, eye),
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

fn orthographic_rh(
    aspect_ratio: f32,
    vertical_world_size: f32,
    near_plane: f32,
    far_plane: f32,
) -> Result<Matrix4, CameraUpdateError> {
    if aspect_ratio <= 0.0
        || vertical_world_size <= 0.0
        || near_plane <= 0.0
        || far_plane <= near_plane
    {
        return Err(CameraUpdateError::InvalidProjectionConfig);
    }

    let half_height = vertical_world_size * 0.5;
    let half_width = half_height * aspect_ratio;

    Ok([
        [1.0 / half_width, 0.0, 0.0, 0.0],
        [0.0, 1.0 / half_height, 0.0, 0.0],
        [0.0, 0.0, 1.0 / (near_plane - far_plane), 0.0],
        [0.0, 0.0, near_plane / (near_plane - far_plane), 1.0],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_major_uniform_memory_matches_wgsl_column_major_transform() {
        let camera = RenderCameraState {
            eye: [10.392304, 10.392304, -10.392304],
            target: [0.0, 0.5, 0.0],
            up: [-0.5, std::f32::consts::FRAC_1_SQRT_2, 0.5],
            aspect_override: Some(16.0 / 9.0),
            projection_mode: RenderProjectionMode::Orthographic {
                vertical_world_size: 5.0,
            },
            basis_override: Some(RenderViewBasis {
                right: [std::f32::consts::FRAC_1_SQRT_2, 0.0, std::f32::consts::FRAC_1_SQRT_2],
                up: [-0.5, std::f32::consts::FRAC_1_SQRT_2, 0.5],
                forward: [
                    -0.5,
                    -std::f32::consts::FRAC_1_SQRT_2,
                    0.5,
                ],
            }),
        };
        let projection = CameraProjectionConfig {
            vertical_fov_radians: std::f32::consts::FRAC_PI_3,
            near_plane: 0.1,
            far_plane: 1000.0,
        };

        let mut gpu = CameraGpuState::default();
        gpu.update(&camera, &projection, 1600, 900, 0).unwrap();
        let uniform =
            CameraUniform::from_view_projection_and_eye(gpu.view_projection, gpu.eye_position);
        let point = [0.5, 1.0, 0.5, 1.0];

        let cpu_clip = multiply_row_vector(point, gpu.view_projection);
        let gpu_clip = multiply_wgsl_column_major_uniform(uniform.view_projection, point);

        for index in 0..4 {
            assert!((cpu_clip[index] - gpu_clip[index]).abs() < 1e-5);
        }
    }

    fn multiply_row_vector(vector: [f32; 4], matrix: Matrix4) -> [f32; 4] {
        let mut result = [0.0; 4];
        for column in 0..4 {
            result[column] = (0..4).map(|index| vector[index] * matrix[index][column]).sum();
        }
        result
    }

    fn multiply_wgsl_column_major_uniform(matrix: Matrix4, vector: [f32; 4]) -> [f32; 4] {
        let flat = matrix
            .into_iter()
            .flat_map(|row| row.into_iter())
            .collect::<Vec<_>>();
        let columns = [
            [flat[0], flat[1], flat[2], flat[3]],
            [flat[4], flat[5], flat[6], flat[7]],
            [flat[8], flat[9], flat[10], flat[11]],
            [flat[12], flat[13], flat[14], flat[15]],
        ];
        let mut result = [0.0; 4];
        for row in 0..4 {
            result[row] = (0..4).map(|index| columns[index][row] * vector[index]).sum();
        }
        result
    }
}
