use super::{
    AppMode, GameApp,
    ui::{
        build_world_select_layout, UiRectPx, WorldSelectAction, WorldSelectButtonLayout,
        WorldSelectFieldLayout, WorldSelectInfoLineLayout, WorldSelectLayout,
        WorldSelectSectionLayout, WorldSelectState,
    },
};
use crate::ecs::{
    CameraState, EcsInputSnapshot, InventoryItem, PlayerInventory, QUICKSLOT_COUNT,
    quarter_view_render_camera_pose,
};
#[cfg(test)]
use crate::ecs::quarter_view_camera_pose;
use crate::renderer::{
    ChunkCoord as RenderChunkCoord, CpuMesh as RenderCpuMesh, MeshVertex as RenderMeshVertex,
    RenderCameraState, RenderCubeInstance, RenderMaterialKind, RenderProjectionMode,
    RenderUiSprite, RenderUploadRequest, RenderViewBasis,
};
use crate::world::{
    BlockMaterialKind, ChunkCoord as WorldChunkCoord, CpuMesh as WorldCpuMesh,
    MeshVertex as WorldMeshVertex,
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
const TILE_SLOT_FILL: (u32, u32) = (14, 0);

#[derive(Debug, Clone, PartialEq)]
pub struct AppRenderFrameData {
    pub camera: RenderCameraState,
    pub draw_scene: bool,
    pub visible_chunks: Vec<RenderChunkCoord>,
    pub cube_instances: Vec<RenderCubeInstance>,
    pub ui_sprites: Vec<RenderUiSprite>,
    pub clear_color_override: Option<[f32; 4]>,
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
            zoom_scroll_delta: if input.modifiers.control {
                input.wheel_delta.1
            } else {
                0.0
            },
            quickslot_scroll_steps: if input.modifiers.control {
                0
            } else {
                wheel_steps(input.wheel_delta.1)
            },
            primary_down: input.left_pressed,
            primary_just_pressed: input.left_just_pressed,
            secondary_down: input.right_pressed,
            secondary_just_pressed: input.right_just_pressed,
            rotate_camera: camera_rotation_axis(
                input.just_pressed_keys.contains(&KeyCode::KeyQ),
                input.just_pressed_keys.contains(&KeyCode::KeyE),
            ),
            recenter_camera: input.just_pressed_keys.contains(&KeyCode::KeyY),
            toggle_manipulation_mode: input.just_pressed_keys.contains(&KeyCode::Tab),
            toggle_inventory: input.just_pressed_keys.contains(&KeyCode::KeyI),
            select_quickslot: direct_quickslot_selection(&input.just_pressed_keys),
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
                let inventory = self.ecs.local_player_inventory();
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
                push_selection_preview_instances(&mut cube_instances, &selection);

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
                    ui_sprites: build_ingame_ui_sprites(
                        self.ui.show_minimap_overlay,
                        viewport,
                        inventory,
                        self.world.block_registry(),
                    ),
                    clear_color_override: None,
                }
            }
            AppMode::WorldSelect => {
                let current_loaded_label = self
                    .created_world
                    .as_ref()
                    .and_then(|source| source.root().file_name())
                    .and_then(|name| name.to_str());
                let layout =
                    build_world_select_layout(&self.ui.world_select, viewport, current_loaded_label);
                let hovered_action =
                    layout.action_at(self.platform.raw_input_state().mouse_position);

                AppRenderFrameData {
                    camera,
                    draw_scene: false,
                    visible_chunks: Vec::new(),
                    cube_instances: Vec::new(),
                    ui_sprites: build_world_select_ui_sprites(
                        &layout,
                        &self.ui.world_select,
                        hovered_action,
                    ),
                    clear_color_override: Some([0.06, 0.07, 0.09, 1.0]),
                }
            }
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

fn camera_rotation_axis(q_pressed: bool, e_pressed: bool) -> i8 {
    axis(e_pressed, q_pressed)
}

fn wheel_steps(delta_y: f32) -> i8 {
    if !delta_y.is_finite() || delta_y.abs() <= f32::EPSILON {
        0
    } else if delta_y > 0.0 {
        -1
    } else {
        1
    }
}

fn direct_quickslot_selection(keys: &std::collections::HashSet<KeyCode>) -> Option<u8> {
    const DIGITS: [(KeyCode, u8); 10] = [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
        (KeyCode::Digit5, 4),
        (KeyCode::Digit6, 5),
        (KeyCode::Digit7, 6),
        (KeyCode::Digit8, 7),
        (KeyCode::Digit9, 8),
        (KeyCode::Digit0, 9),
    ];

    DIGITS
        .into_iter()
        .find_map(|(key, slot)| keys.contains(&key).then_some(slot))
}

fn build_quarter_view_camera(camera_state: CameraState) -> RenderCameraState {
    let pose = quarter_view_render_camera_pose(camera_state);

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

fn push_selection_preview_instances(
    cube_instances: &mut Vec<RenderCubeInstance>,
    selection: &crate::ecs::SelectionState,
) {
    for preview in &selection.interaction_preview_blocks {
        cube_instances.push(RenderCubeInstance {
            center: [
                preview.block.0 as f32 + 0.5,
                preview.block.1 as f32 + 0.5,
                preview.block.2 as f32 + 0.5,
            ],
            half_extents: [0.505, 0.505, 0.505],
            color: [1.0, 0.22, 0.22, 0.18],
            material_kind: RenderMaterialKind::Highlight,
        });
    }

    if let Some(block) = selection.build_preview_block {
        cube_instances.push(RenderCubeInstance {
            center: [block.0 as f32 + 0.5, block.1 as f32 + 0.5, block.2 as f32 + 0.5],
            half_extents: [0.49, 0.49, 0.49],
            color: [1.0, 0.95, 0.35, 0.35],
            material_kind: RenderMaterialKind::Highlight,
        });
    }
}

fn build_ingame_ui_sprites(
    show_minimap_overlay: bool,
    viewport: [f32; 2],
    inventory: Option<PlayerInventory>,
    registry: &crate::world::BlockRegistry,
) -> Vec<RenderUiSprite> {
    let mut sprites = Vec::new();
    if show_minimap_overlay {
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
    }

    if let Some(inventory) = inventory {
        push_ingame_hud(&mut sprites, viewport, inventory, registry);
        if inventory.inventory_open {
            push_inventory_overlay(&mut sprites, viewport, inventory, registry);
        }
    }
    sprites
}

fn push_ingame_hud(
    sprites: &mut Vec<RenderUiSprite>,
    viewport: [f32; 2],
    inventory: PlayerInventory,
    registry: &crate::world::BlockRegistry,
) {
    let mode_panel = UiRectPx {
        x: 28.0,
        y: viewport[1] - 96.0,
        w: 188.0,
        h: 60.0,
    };
    let quickbar_panel = UiRectPx {
        x: (viewport[0] * 0.5 - 278.0).floor(),
        y: viewport[1] - 102.0,
        w: 556.0,
        h: 72.0,
    };

    push_panel(sprites, mode_panel, [0.18, 0.15, 0.13, 0.98], [0.70, 0.63, 0.46, 0.98]);
    push_text(
        sprites,
        mode_panel.x + 14.0,
        mode_panel.y + 10.0,
        1.0,
        "MODE",
        [0.95, 0.88, 0.70, 1.0],
    );
    push_text(
        sprites,
        mode_panel.x + 14.0,
        mode_panel.y + 28.0,
        2.0,
        inventory.manipulation_mode.label(),
        [0.93, 0.96, 1.0, 1.0],
    );

    push_panel(
        sprites,
        quickbar_panel,
        [0.18, 0.15, 0.13, 0.98],
        [0.70, 0.63, 0.46, 0.98],
    );
    push_text(
        sprites,
        quickbar_panel.x + 14.0,
        quickbar_panel.y + 10.0,
        1.0,
        inventory.manipulation_mode.label(),
        [0.95, 0.88, 0.70, 1.0],
    );

    push_quickslot_row(
        sprites,
        UiRectPx {
            x: quickbar_panel.x + 12.0,
            y: quickbar_panel.y + 26.0,
            w: quickbar_panel.w - 24.0,
            h: 36.0,
        },
        inventory.active_quickslots(),
        inventory.active_selected_slot(),
        registry,
    );
}

fn push_inventory_overlay(
    sprites: &mut Vec<RenderUiSprite>,
    viewport: [f32; 2],
    inventory: PlayerInventory,
    registry: &crate::world::BlockRegistry,
) {
    let panel = UiRectPx {
        x: (viewport[0] * 0.5 - 360.0).floor(),
        y: (viewport[1] * 0.5 - 226.0).floor(),
        w: 720.0,
        h: 452.0,
    };
    push_panel(sprites, panel, [0.20, 0.17, 0.14, 0.98], [0.83, 0.70, 0.44, 0.98]);
    push_text(
        sprites,
        panel.x + 20.0,
        panel.y + 16.0,
        2.0,
        "INVENTORY",
        [0.97, 0.91, 0.76, 1.0],
    );
    push_text(
        sprites,
        panel.x + panel.w - 206.0,
        panel.y + 18.0,
        1.0,
        "I TO CLOSE",
        [0.80, 0.82, 0.88, 1.0],
    );

    let general_origin_x = panel.x + 18.0;
    let general_origin_y = panel.y + 68.0;
    let slot_size = 32.0;
    let slot_gap = 6.0;

    for row in 0..4 {
        for column in 0..10 {
            let index = row * 10 + column;
            let rect = UiRectPx {
                x: general_origin_x + column as f32 * (slot_size + slot_gap),
                y: general_origin_y + row as f32 * (slot_size + slot_gap),
                w: slot_size,
                h: slot_size,
            };
            push_inventory_slot(
                sprites,
                rect,
                inventory.general_slots[index],
                false,
                registry,
            );
        }
    }

    push_text(
        sprites,
        panel.x + 20.0,
        panel.y + panel.h - 112.0,
        1.0,
        "TOOLS",
        [0.95, 0.88, 0.70, 1.0],
    );
    push_quickslot_row(
        sprites,
        UiRectPx {
            x: panel.x + 92.0,
            y: panel.y + panel.h - 126.0,
            w: 560.0,
            h: 36.0,
        },
        inventory.tool_quickslots,
        inventory.selected_tool_slot as usize,
        registry,
    );

    push_text(
        sprites,
        panel.x + 20.0,
        panel.y + panel.h - 64.0,
        1.0,
        "BLOCKS",
        [0.95, 0.88, 0.70, 1.0],
    );
    push_quickslot_row(
        sprites,
        UiRectPx {
            x: panel.x + 92.0,
            y: panel.y + panel.h - 78.0,
            w: 560.0,
            h: 36.0,
        },
        inventory.block_quickslots,
        inventory.selected_block_slot as usize,
        registry,
    );
}

fn push_quickslot_row(
    sprites: &mut Vec<RenderUiSprite>,
    rect: UiRectPx,
    slots: [Option<crate::ecs::InventorySlot>; QUICKSLOT_COUNT],
    selected_index: usize,
    registry: &crate::world::BlockRegistry,
) {
    let slot_gap = 6.0;
    let slot_width = ((rect.w - slot_gap * (QUICKSLOT_COUNT as f32 - 1.0)) / QUICKSLOT_COUNT as f32)
        .floor();
    for index in 0..QUICKSLOT_COUNT {
        let slot_rect = UiRectPx {
            x: rect.x + index as f32 * (slot_width + slot_gap),
            y: rect.y,
            w: slot_width,
            h: rect.h,
        };
        push_inventory_slot(
            sprites,
            slot_rect,
            slots[index],
            index == selected_index,
            registry,
        );
        push_text(
            sprites,
            slot_rect.x + 2.0,
            slot_rect.y + slot_rect.h - 10.0,
            1.0,
            &format!("{}", (index + 1) % 10),
            [0.72, 0.76, 0.84, 1.0],
        );
    }
}

fn push_inventory_slot(
    sprites: &mut Vec<RenderUiSprite>,
    rect: UiRectPx,
    slot: Option<crate::ecs::InventorySlot>,
    selected: bool,
    registry: &crate::world::BlockRegistry,
) {
    let frame = if selected {
        [0.92, 0.76, 0.30, 1.0]
    } else {
        [0.46, 0.42, 0.34, 0.98]
    };
    let fill = if selected {
        [0.16, 0.15, 0.10, 0.98]
    } else {
        [0.08, 0.10, 0.12, 0.96]
    };
    push_small_panel(sprites, rect, frame, fill);
    push_fill(sprites, rect.inset(6.0), TILE_SLOT_FILL, [0.07, 0.09, 0.11, 0.92]);

    if let Some(slot) = slot {
        let label = inventory_slot_short_label(slot, registry);
        push_text_centered(
            sprites,
            UiRectPx {
                x: rect.x + 2.0,
                y: rect.y + 6.0,
                w: rect.w - 4.0,
                h: 12.0,
            },
            1.0,
            label,
            [0.96, 0.96, 0.98, 1.0],
        );
        if matches!(slot.item, InventoryItem::Block(_)) {
            push_text(
                sprites,
                rect.x + 4.0,
                rect.y + rect.h - 12.0,
                1.0,
                &format!("{}", slot.count),
                [0.84, 0.88, 0.94, 1.0],
            );
        }
    }
}

fn inventory_slot_short_label(
    slot: crate::ecs::InventorySlot,
    registry: &crate::world::BlockRegistry,
) -> &'static str {
    match slot.item {
        InventoryItem::Tool(tool) => tool.short_label(),
        InventoryItem::Block(block) => match registry.block_or_missing(block).key.as_str() {
            "grass" => "GRAS",
            "dirt" => "DIRT",
            "stone" => "STON",
            "__missing" => "MISS",
            _ => "BLCK",
        },
    }
}

fn build_world_select_ui_sprites(
    layout: &WorldSelectLayout,
    state: &WorldSelectState,
    hovered_action: Option<WorldSelectAction>,
) -> Vec<RenderUiSprite> {
    let mut sprites = Vec::new();

    push_panel(
        &mut sprites,
        layout.title_rect,
        [0.20, 0.17, 0.14, 0.98],
        [0.83, 0.70, 0.44, 0.98],
    );
    push_text(
        &mut sprites,
        layout.title_rect.x + 22.0,
        layout.title_rect.y + 18.0,
        4.0,
        "WORLD SELECT",
        [0.97, 0.91, 0.76, 1.0],
    );
    push_text(
        &mut sprites,
        layout.title_rect.x + 24.0,
        layout.title_rect.y + 58.0,
        2.0,
        "MOUSE SPINNERS  CLICK BUTTONS",
        [0.76, 0.71, 0.61, 1.0],
    );

    for section in &layout.sections {
        push_world_select_section_layout(
            &mut sprites,
            section,
            state.section == section.section
                || hovered_action.and_then(WorldSelectAction::section) == Some(section.section),
            hovered_action,
        );
    }

    push_panel(
        &mut sprites,
        layout.footer_rect,
        [0.16, 0.14, 0.12, 0.98],
        [0.62, 0.56, 0.43, 0.98],
    );
    push_text(
        &mut sprites,
        layout.footer_status_rect.x,
        layout.footer_status_rect.y,
        2.0,
        &truncate_text_to_width(&state.status_line, layout.footer_status_rect.w, 2.0),
        [0.95, 0.88, 0.72, 1.0],
    );
    push_text(
        &mut sprites,
        layout.footer_current_rect.x,
        layout.footer_current_rect.y,
        2.0,
        &format!(
            "CURRENT {}",
            truncate_text_to_width(&layout.footer_current_label, layout.footer_current_rect.w - 96.0, 2.0)
        ),
        [0.70, 0.74, 0.82, 1.0],
    );
    push_world_select_button(
        &mut sprites,
        &layout.close_button,
        hovered_action == Some(layout.close_button.action),
    );

    sprites
}

fn push_world_select_section_layout(
    sprites: &mut Vec<RenderUiSprite>,
    section: &WorldSelectSectionLayout,
    selected: bool,
    hovered_action: Option<WorldSelectAction>,
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

    push_panel(sprites, section.rect, frame_tint, header_tint);
    push_fill(
        sprites,
        section.rect.inset(18.0),
        TILE_PANEL_INSET,
        [0.08, 0.10, 0.12, 0.96],
    );
    push_text(
        sprites,
        section.rect.x + 18.0,
        section.rect.y + 16.0,
        2.0,
        section.section.label(),
        [0.96, 0.91, 0.78, 1.0],
    );
    push_divider(
        sprites,
        UiRectPx {
            x: section.rect.x + 18.0,
            y: section.rect.y + 56.0,
            w: section.rect.w - 36.0,
            h: 8.0,
        },
        [0.58, 0.52, 0.40, 0.94],
    );

    for field in &section.fields {
        push_world_select_field(
            sprites,
            field,
            hovered_action == Some(field.increase_action),
            hovered_action == Some(field.decrease_action),
        );
    }
    for info_line in &section.info_lines {
        push_world_select_info_line(sprites, info_line);
    }
    for button in &section.buttons {
        push_world_select_button(sprites, button, hovered_action == Some(button.action));
    }
}

fn push_world_select_field(
    sprites: &mut Vec<RenderUiSprite>,
    field: &WorldSelectFieldLayout,
    increase_hovered: bool,
    decrease_hovered: bool,
) {
    let label_frame = if field.enabled {
        [0.23, 0.19, 0.15, 1.0]
    } else {
        [0.14, 0.12, 0.11, 0.92]
    };
    let value_frame = if field.enabled {
        [0.14, 0.18, 0.22, 1.0]
    } else {
        [0.09, 0.10, 0.11, 0.90]
    };
    let label_fill = if field.enabled {
        [0.11, 0.10, 0.09, 0.98]
    } else {
        [0.07, 0.07, 0.07, 0.92]
    };
    let value_fill = if field.enabled {
        [0.08, 0.12, 0.16, 0.98]
    } else {
        [0.06, 0.07, 0.08, 0.92]
    };

    push_small_panel(sprites, field.label_rect, label_frame, label_fill);
    push_small_panel(sprites, field.value_rect, value_frame, value_fill);
    push_text_centered(
        sprites,
        field.label_rect,
        2.0,
        field.label,
        [0.96, 0.90, 0.78, 1.0],
    );
    push_text_centered(
        sprites,
        field.value_rect,
        2.0,
        &truncate_text_to_width(&field.value, field.value_rect.w - 10.0, 2.0),
        if field.enabled {
            [0.78, 0.86, 0.95, 1.0]
        } else {
            [0.48, 0.52, 0.56, 0.98]
        },
    );

    push_world_select_arrow_button(sprites, field.increase_rect, "UP", increase_hovered, field.enabled);
    push_world_select_arrow_button(sprites, field.decrease_rect, "DN", decrease_hovered, field.enabled);
}

fn push_world_select_info_line(sprites: &mut Vec<RenderUiSprite>, info_line: &WorldSelectInfoLineLayout) {
    push_text(
        sprites,
        info_line.rect.x,
        info_line.rect.y,
        1.0,
        &truncate_text_to_width(&info_line.text, info_line.rect.w, 1.0),
        [0.76, 0.80, 0.86, 1.0],
    );
}

fn push_world_select_button(
    sprites: &mut Vec<RenderUiSprite>,
    button: &WorldSelectButtonLayout,
    hovered: bool,
) {
    let (frame_tint, fill_tint, text_tint) = if button.enabled {
        if hovered {
            ([0.90, 0.73, 0.32, 1.0], [0.21, 0.16, 0.10, 1.0], [0.99, 0.94, 0.82, 1.0])
        } else {
            ([0.64, 0.55, 0.36, 1.0], [0.16, 0.13, 0.10, 0.98], [0.95, 0.88, 0.74, 1.0])
        }
    } else {
        ([0.28, 0.26, 0.22, 0.94], [0.09, 0.09, 0.09, 0.90], [0.45, 0.45, 0.45, 0.96])
    };

    push_small_panel(sprites, button.rect, frame_tint, fill_tint);
    push_text_centered(sprites, button.rect, 2.0, button.label, text_tint);
}

fn push_world_select_arrow_button(
    sprites: &mut Vec<RenderUiSprite>,
    rect: UiRectPx,
    label: &str,
    hovered: bool,
    enabled: bool,
) {
    let (frame_tint, fill_tint, text_tint) = if enabled {
        if hovered {
            ([0.86, 0.70, 0.30, 1.0], [0.22, 0.16, 0.10, 1.0], [0.99, 0.94, 0.82, 1.0])
        } else {
            ([0.56, 0.50, 0.37, 1.0], [0.14, 0.12, 0.10, 0.98], [0.90, 0.84, 0.72, 1.0])
        }
    } else {
        ([0.26, 0.24, 0.22, 0.94], [0.08, 0.08, 0.08, 0.90], [0.42, 0.42, 0.42, 0.96])
    };

    push_small_panel(sprites, rect, frame_tint, fill_tint);
    push_text_centered(sprites, rect, 1.0, label, text_tint);
}

fn push_small_panel(
    sprites: &mut Vec<RenderUiSprite>,
    rect: UiRectPx,
    frame_tint: [f32; 4],
    fill_tint: [f32; 4],
) {
    push_nine_slice_panel(sprites, rect, 8.0, frame_tint);
    push_fill(sprites, rect.inset(8.0), TILE_PANEL_INSET, fill_tint);
}

fn push_text_centered(
    sprites: &mut Vec<RenderUiSprite>,
    rect: UiRectPx,
    scale: f32,
    text: &str,
    tint: [f32; 4],
) {
    let clipped = truncate_text_to_width(text, (rect.w - 8.0).max(0.0), scale);
    let text_width = measure_text_width(&clipped, scale);
    let text_height = UI_TILE_SIZE_PX * scale;
    let x = rect.x + ((rect.w - text_width).max(0.0) * 0.5).floor();
    let y = rect.y + ((rect.h - text_height).max(0.0) * 0.5).floor();
    push_text(sprites, x, y, scale, &clipped, tint);
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

fn measure_text_width(text: &str, scale: f32) -> f32 {
    text.chars().count() as f32 * UI_TILE_SIZE_PX * scale
}

fn truncate_text_to_width(text: &str, max_width: f32, scale: f32) -> String {
    if max_width <= 0.0 {
        return String::new();
    }

    let max_chars = (max_width / (UI_TILE_SIZE_PX * scale)).floor().max(0.0) as usize;
    truncate_text(text, max_chars)
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
            ..CameraState::default()
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
            ..CameraState::default()
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

    #[test]
    fn render_camera_uses_visual_rotation_during_turn_transition() {
        let camera_state = CameraState {
            quarter_turns: 1,
            smoothed_target: [0.0, 0.5, 0.0],
            desired_target: [0.0, 0.5, 0.0],
            render_yaw_radians: std::f32::consts::FRAC_PI_4,
            desired_render_yaw_radians: std::f32::consts::FRAC_PI_2,
            vertical_world_size: 24.0,
            desired_vertical_world_size: 24.0,
            render_rotation_initialized: true,
            initialized: true,
            ..CameraState::default()
        };

        let render_camera = build_quarter_view_camera(camera_state);
        let gameplay_pose = quarter_view_camera_pose(camera_state);
        let render_basis = render_camera
            .basis_override
            .expect("quarter-view basis should exist");

        assert!(render_basis.right[0] > gameplay_pose.basis.right[0]);
        assert!(render_basis.right[2] > gameplay_pose.basis.right[2]);
    }

    #[test]
    fn q_rotation_maps_to_positive_quarter_turn() {
        assert_eq!(camera_rotation_axis(true, false), 1);
    }

    #[test]
    fn e_rotation_maps_to_negative_quarter_turn() {
        assert_eq!(camera_rotation_axis(false, true), -1);
    }

    fn project_to_screen_axes(point: [f32; 3], basis: RenderViewBasis) -> (f32, f32) {
        (dot3(point, basis.right), dot3(point, basis.up))
    }
}
