use std::f32::consts::FRAC_PI_3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentMode {
    Fifo,
    Mailbox,
    Immediate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceFormatPolicy {
    PreferredSrgb,
    PreferredLinear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthFormat {
    Depth24Plus,
    Depth32Float,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ClearColor {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl Default for ClearColor {
    fn default() -> Self {
        Self::new(0.09, 0.11, 0.14, 1.0)
    }
}

impl From<ClearColor> for [f32; 4] {
    fn from(value: ClearColor) -> Self {
        value.to_array()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugRenderConfig {
    pub debug_overlay: bool,
    pub wireframe: bool,
    pub log_lifecycle: bool,
}

impl Default for DebugRenderConfig {
    fn default() -> Self {
        Self {
            debug_overlay: false,
            wireframe: false,
            log_lifecycle: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraProjectionConfig {
    pub vertical_fov_radians: f32,
    pub near_plane: f32,
    pub far_plane: f32,
}

impl Default for CameraProjectionConfig {
    fn default() -> Self {
        Self {
            vertical_fov_radians: FRAC_PI_3,
            near_plane: 0.1,
            far_plane: 1_000.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderConfig {
    pub preferred_present_mode: PresentMode,
    pub preferred_surface_format: SurfaceFormatPolicy,
    pub depth_format: DepthFormat,
    pub clear_color: ClearColor,
    pub sample_count: u32,
    pub upload_budget_bytes_per_frame: usize,
    pub debug: DebugRenderConfig,
    pub camera_projection: CameraProjectionConfig,
}

impl RenderConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.sample_count == 0 {
            return Err("sample_count must be greater than zero");
        }

        if self.camera_projection.vertical_fov_radians <= 0.0
            || !self.camera_projection.vertical_fov_radians.is_finite()
        {
            return Err("vertical_fov_radians must be a finite positive value");
        }

        if self.camera_projection.near_plane <= 0.0
            || !self.camera_projection.near_plane.is_finite()
        {
            return Err("near_plane must be a finite positive value");
        }

        if self.camera_projection.far_plane <= self.camera_projection.near_plane
            || !self.camera_projection.far_plane.is_finite()
        {
            return Err("far_plane must be greater than near_plane");
        }

        Ok(())
    }
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            preferred_present_mode: PresentMode::Fifo,
            preferred_surface_format: SurfaceFormatPolicy::PreferredSrgb,
            depth_format: DepthFormat::Depth24Plus,
            clear_color: ClearColor::default(),
            sample_count: 1,
            upload_budget_bytes_per_frame: 8 * 1024 * 1024,
            debug: DebugRenderConfig::default(),
            camera_projection: CameraProjectionConfig::default(),
        }
    }
}
