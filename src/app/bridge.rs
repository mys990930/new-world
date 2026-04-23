use crate::renderer::{ChunkCoord as RenderChunkCoord, RenderCameraState, RenderCubeInstance, RenderUiSprite};

#[derive(Debug, Clone, PartialEq)]
pub struct AppRenderFrameData {
    pub camera: RenderCameraState,
    pub draw_scene: bool,
    pub visible_chunks: Vec<RenderChunkCoord>,
    pub cube_instances: Vec<RenderCubeInstance>,
    pub ui_sprites: Vec<RenderUiSprite>,
    pub clear_color_override: Option<[f32; 4]>,
}
