use super::{CameraUpdateError, ChunkCoord, RenderCameraState, RenderSurfaceError, Renderer};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderFrameInput<'a> {
    pub camera: &'a RenderCameraState,
    pub visible_chunks: &'a [ChunkCoord],
    pub clear_color_override: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderStats {
    pub frame_index: u64,
    pub draw_call_count: u32,
    pub submitted_chunk_count: u32,
    pub visible_chunk_count: u32,
    pub uploaded_mesh_count: u32,
    pub removed_mesh_count: u32,
    pub presented: bool,
    pub clear_color: [f32; 4],
}

impl Default for RenderStats {
    fn default() -> Self {
        Self {
            frame_index: 0,
            draw_call_count: 0,
            submitted_chunk_count: 0,
            visible_chunk_count: 0,
            uploaded_mesh_count: 0,
            removed_mesh_count: 0,
            presented: false,
            clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderError {
    Surface(RenderSurfaceError),
    Camera(CameraUpdateError),
}

impl Renderer {
    pub fn render(&mut self, frame: RenderFrameInput<'_>) -> Result<RenderStats, RenderError> {
        let frame_index = self.frame_index;
        self.frame_index = self.frame_index.saturating_add(1);

        let (uploaded_mesh_count, removed_mesh_count) = self.world.finish_frame();
        let clear_color = frame
            .clear_color_override
            .unwrap_or_else(|| self.config.clear_color.to_array());

        let visible_chunk_count = u32::try_from(frame.visible_chunks.len()).unwrap_or(u32::MAX);
        let mut stats = RenderStats {
            frame_index,
            draw_call_count: 0,
            submitted_chunk_count: 0,
            visible_chunk_count,
            uploaded_mesh_count,
            removed_mesh_count,
            presented: false,
            clear_color,
        };

        if !self.surface.is_configured() {
            self.last_stats = stats;
            return Ok(stats);
        }

        self.camera
            .update(
                frame.camera,
                &self.config.camera_projection,
                self.surface.width(),
                self.surface.height(),
                frame_index,
            )
            .map_err(RenderError::Camera)?;

        let submitted_chunk_count = frame
            .visible_chunks
            .iter()
            .filter(|coord| self.world.chunk_meshes.contains_key(coord))
            .count();
        let submitted_chunk_count = u32::try_from(submitted_chunk_count).unwrap_or(u32::MAX);

        stats.submitted_chunk_count = submitted_chunk_count;
        stats.draw_call_count = submitted_chunk_count;
        stats.presented = true;

        self.surface.mark_presented();
        self.last_stats = stats;
        Ok(stats)
    }
}
