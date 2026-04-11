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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderQualityTier {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowQuality {
    Off,
    HardSun,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderQualityConfig {
    pub tier: RenderQualityTier,
    pub fog_enabled: bool,
    pub color_grading_enabled: bool,
    pub climate_tint_enabled: bool,
    pub weather_tint_enabled: bool,
    pub shadow_quality: ShadowQuality,
}

impl RenderQualityConfig {
    pub const fn low() -> Self {
        Self {
            tier: RenderQualityTier::Low,
            fog_enabled: true,
            color_grading_enabled: false,
            climate_tint_enabled: false,
            weather_tint_enabled: false,
            shadow_quality: ShadowQuality::Off,
        }
    }

    pub const fn medium() -> Self {
        Self {
            tier: RenderQualityTier::Medium,
            fog_enabled: true,
            color_grading_enabled: true,
            climate_tint_enabled: true,
            weather_tint_enabled: true,
            shadow_quality: ShadowQuality::HardSun,
        }
    }

    pub const fn high() -> Self {
        Self {
            tier: RenderQualityTier::High,
            fog_enabled: true,
            color_grading_enabled: true,
            climate_tint_enabled: true,
            weather_tint_enabled: true,
            shadow_quality: ShadowQuality::HardSun,
        }
    }

    pub const fn shadow_map_size(self) -> Option<u32> {
        match (self.tier, self.shadow_quality) {
            (_, ShadowQuality::Off) => None,
            (RenderQualityTier::Medium, ShadowQuality::HardSun) => Some(1024),
            (RenderQualityTier::High, ShadowQuality::HardSun) => Some(2048),
            (RenderQualityTier::Low, ShadowQuality::HardSun) => Some(768),
        }
    }
}

impl Default for RenderQualityConfig {
    fn default() -> Self {
        Self::medium()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderEnvironment {
    pub time_of_day_hours: f32,
    pub sun_direction: [f32; 3],
    pub sun_color: [f32; 3],
    pub sun_intensity: f32,
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub fog_color: [f32; 3],
    pub fog_density: f32,
    pub fog_height_falloff: f32,
    pub sky_color: [f32; 3],
    pub horizon_color: [f32; 3],
    pub overcast: f32,
    pub weather_strength: f32,
    pub wetness: f32,
    pub climate_tint: [f32; 3],
    pub climate_humidity: f32,
    pub climate_temperature_bias: f32,
    pub top_face_boost: f32,
    pub side_shadow_strength: f32,
    pub silhouette_boost: f32,
    pub saturation_boost: f32,
}

impl RenderEnvironment {
    pub fn midday_quarter_view() -> Self {
        Self {
            time_of_day_hours: 12.0,
            sun_direction: normalize3([0.28, 0.95, -0.14]),
            sun_color: [1.02, 1.0, 0.94],
            sun_intensity: 1.18,
            ambient_color: [0.55, 0.63, 0.73],
            ambient_intensity: 0.98,
            fog_color: [0.74, 0.85, 0.97],
            fog_density: 0.010,
            fog_height_falloff: 0.045,
            sky_color: [0.40, 0.68, 0.98],
            horizon_color: [0.78, 0.90, 0.99],
            overcast: 0.08,
            weather_strength: 0.0,
            wetness: 0.0,
            climate_tint: [1.0, 1.0, 1.0],
            climate_humidity: 0.40,
            climate_temperature_bias: 0.02,
            top_face_boost: 0.18,
            side_shadow_strength: 0.27,
            silhouette_boost: 0.16,
            saturation_boost: 0.02,
        }
    }

    pub fn sunset_quarter_view() -> Self {
        Self {
            time_of_day_hours: 18.35,
            sun_direction: normalize3([0.52, 0.86, -0.18]),
            sun_color: [1.08, 0.74, 0.47],
            sun_intensity: 1.05,
            ambient_color: [0.46, 0.34, 0.31],
            ambient_intensity: 0.82,
            fog_color: [0.88, 0.62, 0.47],
            fog_density: 0.016,
            fog_height_falloff: 0.06,
            sky_color: [0.56, 0.42, 0.54],
            horizon_color: [0.96, 0.62, 0.42],
            overcast: 0.12,
            weather_strength: 0.0,
            wetness: 0.0,
            climate_tint: [1.03, 1.0, 0.98],
            climate_humidity: 0.48,
            climate_temperature_bias: 0.12,
            top_face_boost: 0.26,
            side_shadow_strength: 0.32,
            silhouette_boost: 0.22,
            saturation_boost: 0.05,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        let all_scalars = [
            self.time_of_day_hours,
            self.sun_intensity,
            self.ambient_intensity,
            self.fog_density,
            self.fog_height_falloff,
            self.overcast,
            self.weather_strength,
            self.wetness,
            self.climate_humidity,
            self.climate_temperature_bias,
            self.top_face_boost,
            self.side_shadow_strength,
            self.silhouette_boost,
            self.saturation_boost,
        ];
        if all_scalars.iter().any(|value| !value.is_finite()) {
            return Err("environment values must be finite");
        }

        if !(0.0..=24.0).contains(&self.time_of_day_hours) {
            return Err("time_of_day_hours must be between 0 and 24");
        }

        if self.sun_intensity < 0.0 || self.ambient_intensity < 0.0 || self.fog_density < 0.0 {
            return Err("light intensity and fog density must be non-negative");
        }

        if self.fog_height_falloff < 0.0 {
            return Err("fog_height_falloff must be non-negative");
        }

        if !(0.0..=1.0).contains(&self.overcast)
            || !(0.0..=1.0).contains(&self.weather_strength)
            || !(0.0..=1.0).contains(&self.wetness)
            || !(0.0..=1.0).contains(&self.climate_humidity)
        {
            return Err("weather/climate blend values must stay within 0..=1");
        }

        if !is_finite3(self.sun_direction)
            || !is_finite3(self.sun_color)
            || !is_finite3(self.ambient_color)
            || !is_finite3(self.fog_color)
            || !is_finite3(self.sky_color)
            || !is_finite3(self.horizon_color)
            || !is_finite3(self.climate_tint)
        {
            return Err("environment color/vector values must be finite");
        }

        Ok(())
    }

    pub fn resolved_clear_color(self, fallback: ClearColor) -> [f32; 4] {
        if !is_finite3(self.sky_color) || !is_finite3(self.horizon_color) {
            return fallback.to_array();
        }

        let rgb = lerp3(self.horizon_color, self.sky_color, 0.34);
        [rgb[0], rgb[1], rgb[2], 1.0]
    }
}

impl Default for RenderEnvironment {
    fn default() -> Self {
        Self::midday_quarter_view()
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
    pub quality: RenderQualityConfig,
    pub environment: RenderEnvironment,
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

        self.environment.validate()?;
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
            quality: RenderQualityConfig::default(),
            environment: RenderEnvironment::default(),
        }
    }
}

fn normalize3(vector: [f32; 3]) -> [f32; 3] {
    let length_sq = vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2];
    if length_sq <= f32::EPSILON {
        [0.0, 1.0, 0.0]
    } else {
        let inv_length = length_sq.sqrt().recip();
        [
            vector[0] * inv_length,
            vector[1] * inv_length,
            vector[2] * inv_length,
        ]
    }
}

fn is_finite3(vector: [f32; 3]) -> bool {
    vector.into_iter().all(f32::is_finite)
}

fn lerp3(start: [f32; 3], end: [f32; 3], t: f32) -> [f32; 3] {
    [
        start[0] + (end[0] - start[0]) * t,
        start[1] + (end[1] - start[1]) * t,
        start[2] + (end[2] - start[2]) * t,
    ]
}
