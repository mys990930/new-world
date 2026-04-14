use super::{
    AppMode, GameApp,
    ui::{WorldSelectSection, WorldSelectState},
};
use crate::ecs::{CameraState, EcsInputSnapshot, quarter_view_camera_pose};
use crate::renderer::{
    ChunkCoord as RenderChunkCoord, CpuMesh as RenderCpuMesh, MeshVertex as RenderMeshVertex,
    RenderCameraState, RenderCubeInstance, RenderMaterialKind, RenderProjectionMode,
    RenderUiSprite, RenderUploadRequest, RenderViewBasis,
};
use crate::world::{
    BlockFace, BlockMaterialKind, ChunkCoord as WorldChunkCoord, CpuMesh as WorldCpuMesh,
    MeshVertex as WorldMeshVertex, WorldBlockCoord,
};
use winit::keyboard::KeyCode;

const UI_TILE_SIZE_PX: f32 = 8.0;
const UI_ATLAS_COLUMNS: u32 = 16;
const UI_ATLAS_ROWS: u32 = 8;
const UI_FONT_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789:-_/().,?#";

const TILE_PANEL_CENTER: (u32, u32) = (0, 0);
const TILE_PANEL_TOP: (u32, u32) = (1, 0);
const TILE_PANEL_BOTTOM: (u32, u32) = (2, 0);
const TILE_PANEL_LEFT: (u32, u32) = (3, 0);
const TILE_PANEL_RIGHT: (u32, u32) = (4, 0);
const TILE_PANEL_CORNER_TL: (u32, u32) = (5, 0);
const TILE_PANEL_CORNER_TR: (u32, u32) = (6, 0);
const TILE_PANEL_CORNER_BL: (u32, u32) = (7, 0);
const TILE_PANEL_CORNER_BR: (u32, u32) = (8, 0);
const TILE_PANEL_HEADER: (u32, u32) = (9, 0);
const TILE_PANEL_INSET: (u32, u32) = (10, 0);
const TILE_PANEL_MARKER: (u32, u32) = (11, 0);
const TILE_PANEL_DIVIDER: (u32, u32) = (12, 0);

#[derive(Debug, Clone, PartialEq)]
pub struct AppRenderFrameData {
    pub camera: RenderCameraState,
    pub draw_scene: bool,
    pub visible_chunks: Vec<RenderChunkCoord>,
    pub cube_instances: Vec<RenderCubeInstance>,
    pub ui_sprites: Vec<RenderUiSprite>,
    pub clear_color_override: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Copy)]
struct UiRectPx {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl UiRectPx {
    fn inset(self, amount: f32) -> Self {
        Self {
            x: self.x + amount,
            y: self.y + amount,
            w: (self.w - amount * 2.0).max(0.0),
            h: (self.h - amount * 2.0).max(0.0),
        }
    }
}

impl GameApp {
    pub fn bridge_platform_to_ecs(&mut self) {
        let window = self.platform.window_state();
        let input = self.platform.raw_input_state();
        let lifecycle = self.platform.lifecycle_state();

        self.ecs.insert_resource(EcsInputSnapshot {
            move_screen_x: axis(
                input.pressed_keys.contains(&KeyCode::KeyA),
                input.pressed_keys.contains(&KeyCode::KeyD),
            ),
            move_screen_y: axis(
                input.pressed_keys.contains(&KeyCode::KeyW),
                input.pressed_keys.contains(&KeyCode::KeyS),
            ),
            zoom_scroll_delta: input.wheel_delta.1,
            primary_down: input.left_pressed,
            primary_just_pressed: input.left_just_pressed,
            secondary_down: input.right_pressed,
            secondary_just_pressed: input.right_just_pressed,
            rotate_camera: axis(
                input.just_pressed_keys.contains(&KeyCode::KeyQ),
                input.just_pressed_keys.contains(&KeyCode::KeyE),
            ),
            recenter_camera: input.just_pressed_keys.contains(&KeyCode::KeyY),
            cursor_screen_pos: input.mouse_position,
            cursor_screen_delta: input.mouse_delta,
            focused: window.focused,
            active: lifecycle.active,
        });
    }

    pub fn bridge_app_to_render_frame(&self) -> AppRenderFrameData {
        let camera_state = self.ecs.camera_state();
        let camera = build_quarter_view_camera(camera_state);
        let window = self.platform.window_state();
        let viewport = [window.width as f32, window.height as f32];

        match self.ui.mode {
            AppMode::InGame => {
                let mut cube_instances = self
                    .ecs
                    .local_player_transform()
                    .zip(self.ecs.local_player_body())
                    .map(|(transform, body)| {
                        vec![RenderCubeInstance {
                            center: transform.translation,
                            half_extents: body.half_extents,
                            color: [1.0, 1.0, 1.0, 1.0],
                            material_kind: RenderMaterialKind::Actor,
                        }]
                    })
                    .unwrap_or_default();

                if let Some((player, body)) = self
                    .ecs
                    .local_player_transform()
                    .zip(self.ecs.local_player_body())
                {
                    cube_instances.insert(
                        0,
                        build_ground_shadow_instance(player.translation, body.half_extents),
                    );
                }

                let selection = self.ecs.selection_state();
                if let (Some(block), Some(face)) = (selection.hovered_block, selection.hovered_face)
                {
                    cube_instances.push(build_selection_face_instance(block, face));
                }

                AppRenderFrameData {
                    camera,
                    draw_scene: true,
                    visible_chunks: self
                        .ecs
                        .visible_chunks()
                        .into_iter()
                        .map(world_chunk_to_render)
                        .collect(),
                    cube_instances,
                    ui_sprites: build_ingame_ui_sprites(self.ui.show_minimap_overlay, viewport),
                    clear_color_override: None,
                }
            }
            AppMode::WorldSelect => AppRenderFrameData {
                camera,
                draw_scene: false,
                visible_chunks: Vec::new(),
                cube_instances: Vec::new(),
                ui_sprites: build_world_select_ui_sprites(
                    &self.ui.world_select,
                    viewport,
                    self.baked_world
                        .as_ref()
                        .and_then(|source| source.root().file_name())
                        .and_then(|name| name.to_str()),
                ),
                clear_color_override: Some([0.06, 0.07, 0.09, 1.0]),
            },
        }
    }

    pub fn bridge_world_mesh_to_render_upload(
        &self,
        coord: WorldChunkCoord,
        mesh: WorldCpuMesh,
    ) -> RenderUploadRequest {
        RenderUploadRequest::UpsertChunkMesh {
            coord: world_chunk_to_render(coord),
            mesh: world_mesh_to_render(mesh),
        }
    }
}

fn axis(negative: bool, positive: bool) -> i8 {
    (positive as i8) - (negative as i8)
}

fn build_quarter_view_camera(camera_state: CameraState) -> RenderCameraState {
    let pose = quarter_view_camera_pose(camera_state);

    RenderCameraState {
        eye: pose.eye,
        target: pose.target,
        up: pose.basis.up,
        aspect_override: None,
        projection_mode: RenderProjectionMode::Perspective,
        basis_override: Some(RenderViewBasis {
            right: pose.basis.right,
            up: pose.basis.up,
            forward: pose.basis.forward,
        }),
    }
}

fn build_ground_shadow_instance(center: [f32; 3], half_extents: [f32; 3]) -> RenderCubeInstance {
    let shadow_offset = [-0.18, 0.0, 0.12];
    RenderCubeInstance {
        center: [
            center[0] + shadow_offset[0],
            center[1] - half_extents[1] + 0.01,
            center[2] + shadow_offset[2],
        ],
        half_extents: [half_extents[0] * 0.96, 0.01, half_extents[2] * 0.96],
        color: [0.08, 0.08, 0.10, 1.0],
        material_kind: RenderMaterialKind::Shadow,
    }
}

fn build_selection_face_instance(block: WorldBlockCoord, face: BlockFace) -> RenderCubeInstance {
    const HIGHLIGHT_HALF_THICKNESS: f32 = 0.02;
    const HIGHLIGHT_HALF_SPAN: f32 = 0.52;
    const HIGHLIGHT_FACE_OFFSET: f32 = 0.02;
    let center = [
        block.0 as f32 + 0.5,
        block.1 as f32 + 0.5,
        block.2 as f32 + 0.5,
    ];

    match face {
        BlockFace::NegX => RenderCubeInstance {
            center: [center[0] - 0.5 - HIGHLIGHT_FACE_OFFSET, center[1], center[2]],
            half_extents: [HIGHLIGHT_HALF_THICKNESS, HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_SPAN],
            color: [1.0, 0.92, 0.20, 1.0],
            material_kind: RenderMaterialKind::Highlight,
        },
        BlockFace::PosX => RenderCubeInstance {
            center: [center[0] + 0.5 + HIGHLIGHT_FACE_OFFSET, center[1], center[2]],
            half_extents: [HIGHLIGHT_HALF_THICKNESS, HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_SPAN],
            color: [1.0, 0.92, 0.20, 1.0],
            material_kind: RenderMaterialKind::Highlight,
        },
        BlockFace::NegY => RenderCubeInstance {
            center: [center[0], center[1] - 0.5 - HIGHLIGHT_FACE_OFFSET, center[2]],
            half_extents: [HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_THICKNESS, HIGHLIGHT_HALF_SPAN],
            color: [1.0, 0.92, 0.20, 1.0],
            material_kind: RenderMaterialKind::Highlight,
        },
        BlockFace::PosY => RenderCubeInstance {
            center: [center[0], center[1] + 0.5 + HIGHLIGHT_FACE_OFFSET, center[2]],
            half_extents: [HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_THICKNESS, HIGHLIGHT_HALF_SPAN],
            color: [1.0, 0.92, 0.20, 1.0],
            material_kind: RenderMaterialKind::Highlight,
        },
        BlockFace::NegZ => RenderCubeInstance {
            center: [center[0], center[1], center[2] - 0.5 - HIGHLIGHT_FACE_OFFSET],
            half_extents: [HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_THICKNESS],
            color: [1.0, 0.92, 0.20, 1.0],
            material_kind: RenderMaterialKind::Highlight,
        },
        BlockFace::PosZ => RenderCubeInstance {
            center: [center[0], center[1], center[2] + 0.5 + HIGHLIGHT_FACE_OFFSET],
            half_extents: [HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_THICKNESS],
            color: [1.0, 0.92, 0.20, 1.0],
            material_kind: RenderMaterialKind::Highlight,
        },
    }
}

fn build_ingame_ui_sprites(show_minimap_overlay: bool, viewport: [f32; 2]) -> Vec<RenderUiSprite> {
    if !show_minimap_overlay {
        return Vec::new();
    }

    let mut sprites = Vec::new();
    let panel = UiRectPx {
        x: viewport[0] - 278.0,
        y: 22.0,
        w: 240.0,
        h: 188.0,
    };
    let inset = panel.inset(18.0);

    push_panel(&mut sprites, panel, [0.18, 0.15, 0.13, 0.98], [0.70, 0.63, 0.46, 0.98]);
    push_fill(&mut sprites, inset, TILE_PANEL_INSET, [0.10, 0.16, 0.12, 0.96]);
    push_text(
        &mut sprites,
        panel.x + 18.0,
        panel.y + 18.0,
        2.0,
        "MINIMAP",
        [0.95, 0.88, 0.70, 1.0],
    );
    push_divider(
        &mut sprites,
        UiRectPx {
            x: panel.x + 18.0,
            y: panel.y + 54.0,
            w: panel.w - 36.0,
            h: 8.0,
        },
        [0.58, 0.53, 0.40, 0.95],
    );
    push_fill(
        &mut sprites,
        UiRectPx {
            x: inset.x + 18.0,
            y: inset.y + 24.0,
            w: inset.w - 36.0,
            h: inset.h - 54.0,
        },
        TILE_PANEL_CENTER,
        [0.16, 0.22, 0.18, 0.90],
    );
    push_tile_sprite(
        &mut sprites,
        TILE_PANEL_MARKER,
        UiRectPx {
            x: inset.x + inset.w * 0.5 - 12.0,
            y: inset.y + inset.h * 0.5 - 12.0,
            w: 24.0,
            h: 24.0,
        },
        [0.94, 0.86, 0.30, 1.0],
    );
    sprites
}

fn build_world_select_ui_sprites(
    state: &WorldSelectState,
    viewport: [f32; 2],
    current_loaded_label: Option<&str>,
) -> Vec<RenderUiSprite> {
    let mut sprites = Vec::new();
    let outer = UiRectPx {
        x: 56.0,
        y: 42.0,
        w: (viewport[0] - 112.0).max(640.0),
        h: (viewport[1] - 84.0).max(420.0),
    };
    let title = UiRectPx {
        x: outer.x,
        y: outer.y,
        w: outer.w,
        h: 76.0,
    };
    let body_y = title.y + title.h + 20.0;
    let bottom_h = 84.0;
    let section_gap = 20.0;
    let section_w = ((outer.w - section_gap * 2.0) / 3.0).floor();
    let section_h = outer.h - title.h - bottom_h - 40.0;
    let bake_panel = UiRectPx {
        x: outer.x,
        y: body_y,
        w: section_w,
        h: section_h,
    };
    let select_panel = UiRectPx {
        x: bake_panel.x + section_w + section_gap,
        y: body_y,
        w: section_w,
        h: section_h,
    };
    let spawn_panel = UiRectPx {
        x: select_panel.x + section_w + section_gap,
        y: body_y,
        w: section_w,
        h: section_h,
    };
    let footer = UiRectPx {
        x: outer.x,
        y: outer.y + outer.h - bottom_h,
        w: outer.w,
        h: bottom_h,
    };

    push_panel(&mut sprites, title, [0.20, 0.17, 0.14, 0.98], [0.83, 0.70, 0.44, 0.98]);
    push_text(
        &mut sprites,
        title.x + 22.0,
        title.y + 18.0,
        3.0,
        "WORLD SELECT",
        [0.97, 0.91, 0.76, 1.0],
    );
    push_text(
        &mut sprites,
        title.x + 24.0,
        title.y + 46.0,
        1.0,
        "PIXEL UI  BAKE  SELECT  SPAWN",
        [0.74, 0.70, 0.62, 1.0],
    );

    push_world_select_section(
        &mut sprites,
        bake_panel,
        state.section == WorldSelectSection::Bake,
        WorldSelectSection::Bake.label(),
        &[
            format!("SEED {}", state.bake_seed),
            format!("RADIUS {}", state.bake_radius),
            format!("CENTER {} {}", state.spawn_chunk_x, state.spawn_chunk_z),
            "ENTER OR B".to_string(),
            "REBUILD BAKE".to_string(),
        ],
    );
    push_world_select_section(
        &mut sprites,
        select_panel,
        state.section == WorldSelectSection::Bakes,
        WorldSelectSection::Bakes.label(),
        &build_bake_selection_lines(state),
    );
    push_world_select_section(
        &mut sprites,
        spawn_panel,
        state.section == WorldSelectSection::SpawnChunk,
        WorldSelectSection::SpawnChunk.label(),
        &[
            format!("X {}", state.spawn_chunk_x),
            format!("Z {}", state.spawn_chunk_z),
            "LEFT RIGHT X".to_string(),
            "Q E OR W S Z".to_string(),
            "ENTER LOAD  R RESET".to_string(),
        ],
    );

    push_panel(
        &mut sprites,
        footer,
        [0.16, 0.14, 0.12, 0.98],
        [0.62, 0.56, 0.43, 0.98],
    );
    push_text(
        &mut sprites,
        footer.x + 18.0,
        footer.y + 16.0,
        1.0,
        &truncate_text(&state.status_line, 56),
        [0.95, 0.88, 0.72, 1.0],
    );
    let current_label = current_loaded_label.unwrap_or("NONE");
    push_text(
        &mut sprites,
        footer.x + 18.0,
        footer.y + 40.0,
        1.0,
        &format!("CURRENT {}", truncate_text(current_label, 44)),
        [0.70, 0.74, 0.82, 1.0],
    );

    sprites
}

fn build_bake_selection_lines(state: &WorldSelectState) -> [String; 5] {
    if let Some(selected) = state.selected_bake() {
        let min = selected.manifest.min_chunk_coord();
        let max = selected.manifest.max_chunk_coord();
        [
            truncate_text(&selected.label, 22),
            format!(
                "{} OF {}",
                state.selected_bake_index.saturating_add(1),
                state.available_bakes.len()
            ),
            format!("SEED {}", selected.manifest.seed),
            format!("BOUNDS {} {} {} {}", min.0, min.2, max.0, max.2),
            "ENTER LOAD".to_string(),
        ]
    } else {
        [
            "NO BAKES".to_string(),
            "PRESS B".to_string(),
            "TO CREATE".to_string(),
            "A RUNTIME".to_string(),
            "BAKE".to_string(),
        ]
    }
}

fn push_world_select_section(
    sprites: &mut Vec<RenderUiSprite>,
    panel: UiRectPx,
    selected: bool,
    title: &str,
    lines: &[String],
) {
    let frame_tint = if selected {
        [0.27, 0.22, 0.15, 1.0]
    } else {
        [0.16, 0.14, 0.12, 0.98]
    };
    let header_tint = if selected {
        [0.88, 0.72, 0.34, 1.0]
    } else {
        [0.54, 0.47, 0.34, 0.98]
    };
    let inset = panel.inset(18.0);

    push_panel(sprites, panel, frame_tint, header_tint);
    push_fill(sprites, inset, TILE_PANEL_INSET, [0.08, 0.10, 0.12, 0.96]);
    if selected {
        push_tile_sprite(
            sprites,
            TILE_PANEL_MARKER,
            UiRectPx {
                x: panel.x + panel.w - 30.0,
                y: panel.y + 16.0,
                w: 14.0,
                h: 14.0,
            },
            [0.97, 0.89, 0.36, 1.0],
        );
    }

    push_text(
        sprites,
        panel.x + 18.0,
        panel.y + 16.0,
        2.0,
        title,
        [0.96, 0.91, 0.78, 1.0],
    );
    push_divider(
        sprites,
        UiRectPx {
            x: panel.x + 18.0,
            y: panel.y + 56.0,
            w: panel.w - 36.0,
            h: 8.0,
        },
        [0.58, 0.52, 0.40, 0.94],
    );

    for (index, line) in lines.iter().enumerate() {
        push_text(
            sprites,
            inset.x + 12.0,
            inset.y + 18.0 + index as f32 * 28.0,
            1.0,
            line,
            [0.78, 0.82, 0.89, 1.0],
        );
    }
}

fn push_panel(
    sprites: &mut Vec<RenderUiSprite>,
    rect: UiRectPx,
    frame_tint: [f32; 4],
    header_tint: [f32; 4],
) {
    push_nine_slice_panel(sprites, rect, 16.0, frame_tint);
    push_fill(
        sprites,
        UiRectPx {
            x: rect.x + 16.0,
            y: rect.y + 16.0,
            w: rect.w - 32.0,
            h: 20.0,
        },
        TILE_PANEL_HEADER,
        header_tint,
    );
}

fn push_nine_slice_panel(
    sprites: &mut Vec<RenderUiSprite>,
    rect: UiRectPx,
    border_px: f32,
    tint: [f32; 4],
) {
    let center = UiRectPx {
        x: rect.x + border_px,
        y: rect.y + border_px,
        w: (rect.w - border_px * 2.0).max(0.0),
        h: (rect.h - border_px * 2.0).max(0.0),
    };
    push_fill(sprites, center, TILE_PANEL_CENTER, tint);
    push_fill(
        sprites,
        UiRectPx {
            x: rect.x + border_px,
            y: rect.y,
            w: center.w,
            h: border_px,
        },
        TILE_PANEL_TOP,
        tint,
    );
    push_fill(
        sprites,
        UiRectPx {
            x: rect.x + border_px,
            y: rect.y + rect.h - border_px,
            w: center.w,
            h: border_px,
        },
        TILE_PANEL_BOTTOM,
        tint,
    );
    push_fill(
        sprites,
        UiRectPx {
            x: rect.x,
            y: rect.y + border_px,
            w: border_px,
            h: center.h,
        },
        TILE_PANEL_LEFT,
        tint,
    );
    push_fill(
        sprites,
        UiRectPx {
            x: rect.x + rect.w - border_px,
            y: rect.y + border_px,
            w: border_px,
            h: center.h,
        },
        TILE_PANEL_RIGHT,
        tint,
    );
    push_tile_sprite(
        sprites,
        TILE_PANEL_CORNER_TL,
        UiRectPx {
            x: rect.x,
            y: rect.y,
            w: border_px,
            h: border_px,
        },
        tint,
    );
    push_tile_sprite(
        sprites,
        TILE_PANEL_CORNER_TR,
        UiRectPx {
            x: rect.x + rect.w - border_px,
            y: rect.y,
            w: border_px,
            h: border_px,
        },
        tint,
    );
    push_tile_sprite(
        sprites,
        TILE_PANEL_CORNER_BL,
        UiRectPx {
            x: rect.x,
            y: rect.y + rect.h - border_px,
            w: border_px,
            h: border_px,
        },
        tint,
    );
    push_tile_sprite(
        sprites,
        TILE_PANEL_CORNER_BR,
        UiRectPx {
            x: rect.x + rect.w - border_px,
            y: rect.y + rect.h - border_px,
            w: border_px,
            h: border_px,
        },
        tint,
    );
}

fn push_divider(sprites: &mut Vec<RenderUiSprite>, rect: UiRectPx, tint: [f32; 4]) {
    push_fill(sprites, rect, TILE_PANEL_DIVIDER, tint);
}

fn push_fill(
    sprites: &mut Vec<RenderUiSprite>,
    rect: UiRectPx,
    tile: (u32, u32),
    tint: [f32; 4],
) {
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return;
    }
    push_tile_sprite(sprites, tile, rect, tint);
}

fn push_tile_sprite(
    sprites: &mut Vec<RenderUiSprite>,
    tile: (u32, u32),
    rect: UiRectPx,
    tint: [f32; 4],
) {
    let (uv_min, uv_max) = atlas_tile_uv(tile.0, tile.1);
    sprites.push(RenderUiSprite {
        min_screen_px: [rect.x, rect.y],
        max_screen_px: [rect.x + rect.w, rect.y + rect.h],
        uv_min,
        uv_max,
        tint,
    });
}

fn push_text(
    sprites: &mut Vec<RenderUiSprite>,
    mut x: f32,
    y: f32,
    scale: f32,
    text: &str,
    tint: [f32; 4],
) {
    let advance = UI_TILE_SIZE_PX * scale;
    for character in text.chars() {
        if character == ' ' {
            x += advance;
            continue;
        }

        let Some((tile_x, tile_y)) = font_tile(character) else {
            x += advance;
            continue;
        };

        push_tile_sprite(
            sprites,
            (tile_x, tile_y),
            UiRectPx {
                x,
                y,
                w: UI_TILE_SIZE_PX * scale,
                h: UI_TILE_SIZE_PX * scale,
            },
            tint,
        );
        x += advance;
    }
}

fn font_tile(character: char) -> Option<(u32, u32)> {
    let upper = character.to_ascii_uppercase();
    let index = UI_FONT_CHARS.find(upper)? as u32;
    Some((index % UI_ATLAS_COLUMNS, 1 + index / UI_ATLAS_COLUMNS))
}

fn atlas_tile_uv(tile_x: u32, tile_y: u32) -> ([f32; 2], [f32; 2]) {
    let tile_w = 1.0 / UI_ATLAS_COLUMNS as f32;
    let tile_h = 1.0 / UI_ATLAS_ROWS as f32;
    let min = [tile_x as f32 * tile_w, tile_y as f32 * tile_h];
    let max = [min[0] + tile_w, min[1] + tile_h];
    (min, max)
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let mut truncated = text.chars().take(max_chars).collect::<String>();
    truncated.make_ascii_uppercase();
    truncated
}

fn world_chunk_to_render(coord: WorldChunkCoord) -> RenderChunkCoord {
    RenderChunkCoord(coord.0, coord.1, coord.2)
}

fn world_mesh_to_render(mesh: WorldCpuMesh) -> RenderCpuMesh {
    RenderCpuMesh {
        vertices: mesh.vertices.into_iter().map(world_vertex_to_render).collect(),
        indices: mesh.indices,
        bounds: mesh.bounds.map(|bounds| crate::renderer::RenderBounds {
            min: bounds.min,
            max: bounds.max,
        }),
    }
}

fn world_vertex_to_render(vertex: WorldMeshVertex) -> RenderMeshVertex {
    RenderMeshVertex {
        position: vertex.position,
        color: vertex.color,
        normal: vertex.normal,
        uv: vertex.uv,
        texture_layer: vertex.texture_layer,
        material_kind: render_material_kind_from_world(vertex.material_kind).as_u32(),
        contour_edges: vertex.contour_edges,
    }
}

fn render_material_kind_from_world(kind: BlockMaterialKind) -> RenderMaterialKind {
    match kind {
        BlockMaterialKind::GenericOpaque => RenderMaterialKind::GenericOpaque,
        BlockMaterialKind::Grass => RenderMaterialKind::Grass,
        BlockMaterialKind::Soil => RenderMaterialKind::Soil,
        BlockMaterialKind::Stone => RenderMaterialKind::Stone,
        BlockMaterialKind::Sand => RenderMaterialKind::Sand,
        BlockMaterialKind::Foliage => RenderMaterialKind::Foliage,
        BlockMaterialKind::Water => RenderMaterialKind::Water,
        BlockMaterialKind::Emissive => RenderMaterialKind::Emissive,
    }
}

#[cfg(test)]
fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_view_basis_maps_world_axes_to_expected_screen_directions() {
        let camera = build_quarter_view_camera(CameraState {
            quarter_turns: 0,
            smoothed_target: [0.0, 0.5, 0.0],
            desired_target: [0.0, 0.5, 0.0],
            vertical_world_size: 24.0,
            desired_vertical_world_size: 24.0,
            recenter_requested: false,
            recentering: false,
            initialized: true,
        });
        let basis = camera.basis_override.expect("quarter-view basis should exist");

        let east = project_to_screen_axes([1.0, 0.0, 0.0], basis);
        let north = project_to_screen_axes([0.0, 0.0, 1.0], basis);
        let up = project_to_screen_axes([0.0, 1.0, 0.0], basis);

        assert!(east.0 > 0.0);
        assert!(east.1 < 0.0);
        assert!(north.0 > 0.0);
        assert!(north.1 > 0.0);
        assert!(up.0.abs() < 1e-5);
        assert!(up.1 > 0.0);
    }

    #[test]
    fn top_face_projects_as_cardinal_diamond() {
        let camera = build_quarter_view_camera(CameraState {
            quarter_turns: 0,
            smoothed_target: [0.0, 0.5, 0.0],
            desired_target: [0.0, 0.5, 0.0],
            vertical_world_size: 24.0,
            desired_vertical_world_size: 24.0,
            recenter_requested: false,
            recentering: false,
            initialized: true,
        });
        let basis = camera.basis_override.expect("quarter-view basis should exist");

        let projected = [
            project_to_screen_axes([-0.5, 1.0, -0.5], basis),
            project_to_screen_axes([0.5, 1.0, -0.5], basis),
            project_to_screen_axes([0.5, 1.0, 0.5], basis),
            project_to_screen_axes([-0.5, 1.0, 0.5], basis),
        ];

        let left = projected
            .iter()
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .copied()
            .unwrap();
        let right = projected
            .iter()
            .max_by(|left, right| left.0.total_cmp(&right.0))
            .copied()
            .unwrap();
        let top = projected
            .iter()
            .max_by(|left, right| left.1.total_cmp(&right.1))
            .copied()
            .unwrap();
        let bottom = projected
            .iter()
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .copied()
            .unwrap();

        assert!(left.0 < 0.0);
        assert!(right.0 > 0.0);
        assert!(top.1 > 0.0);
        assert!(bottom.1 > 0.0);
        assert!(left.1 > bottom.1);
        assert!(right.1 > bottom.1);
        assert!(top.0.abs() < 1e-5);
        assert!(bottom.0.abs() < 1e-5);
    }

    #[test]
    fn render_camera_uses_weak_perspective_projection() {
        let camera = build_quarter_view_camera(CameraState {
            vertical_world_size: 28.0,
            desired_vertical_world_size: 28.0,
            initialized: true,
            ..CameraState::default()
        });

        assert_eq!(camera.projection_mode, RenderProjectionMode::Perspective);
        let distance = ((camera.eye[0] - camera.target[0]).powi(2)
            + (camera.eye[1] - camera.target[1]).powi(2)
            + (camera.eye[2] - camera.target[2]).powi(2))
        .sqrt();
        assert!((distance - crate::ecs::quarter_view_perspective_distance(28.0)).abs() < 1e-4);
    }

    fn project_to_screen_axes(point: [f32; 3], basis: RenderViewBasis) -> (f32, f32) {
        (dot3(point, basis.right), dot3(point, basis.up))
    }
}
