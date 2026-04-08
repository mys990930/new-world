use super::{CameraGpuState, PipelineSet, RenderConfig, RenderStats, RenderWorld, Renderer};

pub trait RenderSurfaceTarget {
    fn drawable_size(&self) -> (u32, u32);

    fn debug_name(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StubSurfaceTarget {
    pub width: u32,
    pub height: u32,
    pub debug_name: Option<String>,
}

impl StubSurfaceTarget {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            debug_name: None,
        }
    }

    pub fn with_name(width: u32, height: u32, debug_name: impl Into<String>) -> Self {
        Self {
            width,
            height,
            debug_name: Some(debug_name.into()),
        }
    }
}

impl RenderSurfaceTarget for StubSurfaceTarget {
    fn drawable_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn debug_name(&self) -> Option<&str> {
        self.debug_name.as_deref()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SurfaceSnapshot {
    pub width: u32,
    pub height: u32,
    pub debug_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceState {
    pub snapshot: SurfaceSnapshot,
    pub configured: bool,
    pub minimized: bool,
    pub resize_generation: u64,
    pub present_generation: u64,
}

impl SurfaceState {
    pub fn width(&self) -> u32 {
        self.snapshot.width
    }

    pub fn height(&self) -> u32 {
        self.snapshot.height
    }

    pub fn is_configured(&self) -> bool {
        self.configured
    }

    pub(crate) fn from_target(target: &impl RenderSurfaceTarget) -> Self {
        let snapshot = SurfaceSnapshot {
            width: target.drawable_size().0,
            height: target.drawable_size().1,
            debug_name: target.debug_name().map(ToOwned::to_owned),
        };
        let configured = snapshot.width > 0 && snapshot.height > 0;

        Self {
            minimized: !configured,
            configured,
            snapshot,
            resize_generation: 0,
            present_generation: 0,
        }
    }

    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        self.snapshot.width = width;
        self.snapshot.height = height;
        self.configured = width > 0 && height > 0;
        self.minimized = !self.configured;
        self.resize_generation = self.resize_generation.saturating_add(1);
    }

    pub(crate) fn mark_presented(&mut self) {
        self.present_generation = self.present_generation.saturating_add(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderInitError {
    InvalidConfig(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSurfaceError {
    InvalidSurfaceSize,
}

impl Renderer {
    pub fn new(
        target: &impl RenderSurfaceTarget,
        config: RenderConfig,
    ) -> Result<Self, RenderInitError> {
        config.validate().map_err(RenderInitError::InvalidConfig)?;

        let surface = SurfaceState::from_target(target);
        let pipelines = PipelineSet::new(&config, &surface);
        let camera = CameraGpuState::new(
            &config.camera_projection,
            surface.width(),
            surface.height(),
        )
        .map_err(|_| RenderInitError::InvalidConfig("camera projection config is invalid"))?;

        Ok(Self {
            config,
            surface,
            pipelines,
            world: RenderWorld::default(),
            camera,
            last_stats: RenderStats::default(),
            frame_index: 0,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), RenderSurfaceError> {
        if width == u32::MAX || height == u32::MAX {
            return Err(RenderSurfaceError::InvalidSurfaceSize);
        }

        self.surface.resize(width, height);
        self.pipelines.handle_surface_reconfigured(&self.surface);
        self.camera.handle_resize(width, height);
        Ok(())
    }
}
