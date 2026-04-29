use super::{
    AppMinimapViewport, MINIMAP_BLOCK_SPAN,
    ui::{
        UiRectPx, WorldSelectAction, WorldSelectButtonLayout, WorldSelectFieldLayout,
        WorldSelectInfoLineLayout, WorldSelectInputField, WorldSelectLayout, WorldSelectListLayout,
        WorldSelectLoadingPopupLayout, WorldSelectSectionLayout, WorldSelectState,
        WorldSelectWorldRowLayout,
    },
};
use crate::ecs::{
    InventoryItem, LocalEnvironmentSnapshot, PlayerInventory, QUICKSLOT_COUNT, Transform,
};
use crate::renderer::RenderUiSprite;
use crate::world::{
    BlockRegistry, TopdownColumnScan, TopdownEdge, TopdownSurfaceRange, WorldBlockCoord,
    color_topdown_cell, darken_topdown_color, topdown_edge_strength_for_cell, world_to_chunk_local,
};

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
const MINIMAP_CELL_SIZE_PX: f32 = 5.0;

pub(super) fn build_ingame_ui_sprites(
    show_minimap_overlay: bool,
    viewport: [f32; 2],
    inventory: Option<PlayerInventory>,
    player_transform: Option<Transform>,
    environment: Option<LocalEnvironmentSnapshot>,
    block_registry: &BlockRegistry,
    minimap_viewport: Option<&AppMinimapViewport>,
) -> Vec<RenderUiSprite> {
    let mut sprites = Vec::new();
    if show_minimap_overlay {
        push_minimap_overlay(
            &mut sprites,
            viewport,
            player_transform,
            environment,
            minimap_viewport,
            block_registry,
        );
    }

    push_current_chunk_panel(&mut sprites, viewport, player_transform);

    if let Some(inventory) = inventory {
        push_ingame_hud(&mut sprites, viewport, inventory, block_registry);
        if inventory.inventory_open {
            push_inventory_overlay(&mut sprites, viewport, inventory, block_registry);
        }
    }
    sprites
}

fn push_current_chunk_panel(
    sprites: &mut Vec<RenderUiSprite>,
    viewport: [f32; 2],
    player_transform: Option<Transform>,
) {
    let panel = UiRectPx {
        x: 28.0,
        y: viewport[1] - 164.0,
        w: 188.0,
        h: 56.0,
    };
    push_panel(
        sprites,
        panel,
        [0.18, 0.15, 0.13, 0.98],
        [0.70, 0.63, 0.46, 0.98],
    );
    push_text(
        sprites,
        panel.x + 14.0,
        panel.y + 10.0,
        1.0,
        "CHUNK",
        [0.95, 0.88, 0.70, 1.0],
    );

    let label = player_transform
        .map(|transform| {
            let (chunk, _) = world_to_chunk_local(WorldBlockCoord(
                transform.translation[0].floor() as i32,
                transform.translation[1].floor() as i32,
                transform.translation[2].floor() as i32,
            ));
            format!("X {} Y {} Z {}", chunk.0, chunk.1, chunk.2)
        })
        .unwrap_or_else(|| "X -- Y -- Z --".to_string());

    push_text(
        sprites,
        panel.x + 14.0,
        panel.y + 30.0,
        1.0,
        &truncate_text_to_width(&label, panel.w - 28.0, 1.0),
        [0.93, 0.96, 1.0, 1.0],
    );
}

pub(super) fn build_world_select_ui_sprites(
    layout: &WorldSelectLayout,
    state: &WorldSelectState,
    hovered_action: Option<WorldSelectAction>,
    hovered_input_field: Option<WorldSelectInputField>,
    hovered_world_index: Option<usize>,
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
        "CLICK VALUES TO TYPE  SCROLL THE WORLD LIST",
        [0.76, 0.71, 0.61, 1.0],
    );

    for section in &layout.sections {
        push_world_select_section_layout(
            &mut sprites,
            section,
            state.section == section.section
                || hovered_action.and_then(WorldSelectAction::section) == Some(section.section),
            hovered_action,
            hovered_input_field,
            hovered_world_index,
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
            truncate_text_to_width(
                &layout.footer_current_label,
                layout.footer_current_rect.w - 96.0,
                2.0,
            )
        ),
        [0.70, 0.74, 0.82, 1.0],
    );
    push_world_select_button(
        &mut sprites,
        &layout.close_button,
        hovered_action == Some(layout.close_button.action),
    );

    if let Some(popup) = &layout.loading_popup {
        push_world_select_loading_popup(&mut sprites, popup);
    }

    sprites
}

fn push_minimap_overlay(
    sprites: &mut Vec<RenderUiSprite>,
    viewport: [f32; 2],
    player_transform: Option<Transform>,
    environment: Option<LocalEnvironmentSnapshot>,
    minimap_viewport: Option<&AppMinimapViewport>,
    block_registry: &BlockRegistry,
) {
    let panel = UiRectPx {
        x: viewport[0] - 264.0,
        y: 22.0,
        w: 232.0,
        h: 336.0,
    };
    let inset = panel.inset(18.0);
    let map_rect = UiRectPx {
        x: inset.x + ((inset.w - MINIMAP_BLOCK_SPAN as f32 * MINIMAP_CELL_SIZE_PX) * 0.5).floor(),
        y: panel.y + 60.0,
        w: MINIMAP_BLOCK_SPAN as f32 * MINIMAP_CELL_SIZE_PX,
        h: MINIMAP_BLOCK_SPAN as f32 * MINIMAP_CELL_SIZE_PX,
    };

    push_panel(
        sprites,
        panel,
        [0.18, 0.15, 0.13, 0.98],
        [0.70, 0.63, 0.46, 0.98],
    );
    push_fill(sprites, inset, TILE_PANEL_INSET, [0.10, 0.16, 0.12, 0.96]);
    push_text(
        sprites,
        panel.x + 18.0,
        panel.y + 18.0,
        2.0,
        "MINIMAP",
        [0.95, 0.88, 0.70, 1.0],
    );
    push_divider(
        sprites,
        UiRectPx {
            x: panel.x + 18.0,
            y: panel.y + 50.0,
            w: panel.w - 36.0,
            h: 8.0,
        },
        [0.58, 0.53, 0.40, 0.95],
    );
    push_small_panel(
        sprites,
        UiRectPx {
            x: map_rect.x - 6.0,
            y: map_rect.y - 6.0,
            w: map_rect.w + 12.0,
            h: map_rect.h + 12.0,
        },
        [0.42, 0.40, 0.32, 0.98],
        [0.08, 0.10, 0.12, 0.96],
    );
    push_minimap_status_panel(
        sprites,
        UiRectPx {
            x: inset.x,
            y: map_rect.y + map_rect.h + 18.0,
            w: inset.w,
            h: (panel.y + panel.h) - (map_rect.y + map_rect.h + 18.0) - 18.0,
        },
        environment,
    );

    let Some(player_transform) = player_transform else {
        return;
    };
    let Some(minimap_viewport) = minimap_viewport else {
        return;
    };
    let surface_range = minimap_viewport
        .surface_range
        .unwrap_or(TopdownSurfaceRange { min_y: 0, max_y: 0 });

    let grid_width = MINIMAP_BLOCK_SPAN as usize;
    let grid_height = MINIMAP_BLOCK_SPAN as usize;
    for z in 0..grid_height {
        for x in 0..grid_width {
            let scan = minimap_viewport.columns[z * grid_width + x];
            let base_color = color_topdown_cell(scan.visible, block_registry, surface_range);
            let rect = UiRectPx {
                x: map_rect.x + x as f32 * MINIMAP_CELL_SIZE_PX,
                y: map_rect.y + z as f32 * MINIMAP_CELL_SIZE_PX,
                w: MINIMAP_CELL_SIZE_PX,
                h: MINIMAP_CELL_SIZE_PX,
            };
            push_fill(sprites, rect, TILE_PANEL_CENTER, rgb8_tint(base_color, 1.0));
            push_minimap_cell_edges(
                sprites,
                rect,
                &minimap_viewport.columns,
                grid_width,
                grid_height,
                x,
                z,
                base_color,
            );
        }
    }

    let marker_center_x = map_rect.x
        + (player_transform.translation[0] - minimap_viewport.min_world_x as f32)
            * MINIMAP_CELL_SIZE_PX;
    let marker_center_y = map_rect.y
        + (player_transform.translation[2] - minimap_viewport.min_world_z as f32)
            * MINIMAP_CELL_SIZE_PX;
    push_tile_sprite(
        sprites,
        TILE_PANEL_MARKER,
        UiRectPx {
            x: marker_center_x - 8.0,
            y: marker_center_y - 8.0,
            w: 16.0,
            h: 16.0,
        },
        [0.98, 0.90, 0.30, 1.0],
    );
}

fn push_minimap_status_panel(
    sprites: &mut Vec<RenderUiSprite>,
    rect: UiRectPx,
    environment: Option<LocalEnvironmentSnapshot>,
) {
    push_small_panel(
        sprites,
        rect,
        [0.40, 0.38, 0.30, 0.98],
        [0.08, 0.10, 0.12, 0.96],
    );
    push_text(
        sprites,
        rect.x + 10.0,
        rect.y + 8.0,
        1.0,
        "STATUS",
        [0.95, 0.88, 0.70, 1.0],
    );

    let lines = minimap_status_lines(environment);
    let mut y = rect.y + 24.0;
    for (index, line) in lines.iter().enumerate() {
        push_text(
            sprites,
            rect.x + 10.0,
            y,
            1.0,
            &truncate_text_to_width(line, rect.w - 20.0, 1.0),
            if index < 2 {
                [0.92, 0.96, 1.0, 1.0]
            } else {
                [0.74, 0.80, 0.88, 1.0]
            },
        );
        y += 12.0;
    }
}

fn minimap_status_lines(environment: Option<LocalEnvironmentSnapshot>) -> Vec<String> {
    let Some(environment) = environment else {
        return vec![
            "BIOME UNAVAILABLE".to_string(),
            "FORM UNAVAILABLE".to_string(),
            "SEASON --".to_string(),
            "DAY -- --:--".to_string(),
            "WX --".to_string(),
            "TEMP --".to_string(),
            "HUM --".to_string(),
        ];
    };

    let day = environment.calendar.day.saturating_add(1);
    let day_text = if day < 1000 {
        format!("{day:03}")
    } else {
        day.to_string()
    };
    let humidity_percent = environment
        .display
        .humidity_percent
        .round()
        .clamp(0.0, 100.0) as i32;
    let temperature_celsius = environment.display.temperature_celsius.round() as i32;

    vec![
        format!("BIOME {}", ui_label(environment.region.biome_family)),
        format!("FORM {}", ui_label(environment.region.terrain_form_family)),
        format!("SEASON {}", ui_label(environment.calendar.season_phase)),
        format!(
            "DAY {} {:02}:{:02}",
            day_text, environment.calendar.hour, environment.calendar.minute
        ),
        format!("WX {}", ui_label(environment.weather.kind)),
        format!("TEMP {}C", temperature_celsius),
        format!("HUM {} PCT", humidity_percent),
    ]
}

fn ui_label(value: impl std::fmt::Debug) -> String {
    let raw = format!("{value:?}");
    let mut label = String::with_capacity(raw.len() + 4);
    let mut previous_was_lowercase = false;

    for character in raw.chars() {
        let uppercase = character.to_ascii_uppercase();
        if previous_was_lowercase && uppercase.is_ascii_uppercase() {
            label.push(' ');
        }
        label.push(uppercase);
        previous_was_lowercase = character.is_ascii_lowercase();
    }

    label
}

fn push_minimap_cell_edges(
    sprites: &mut Vec<RenderUiSprite>,
    rect: UiRectPx,
    columns: &[TopdownColumnScan],
    width: usize,
    height: usize,
    x: usize,
    z: usize,
    base_color: [u8; 3],
) {
    let left = topdown_edge_strength_for_cell(columns, width, height, x, z, TopdownEdge::Left);
    if left > 0.0 {
        push_fill(
            sprites,
            UiRectPx {
                x: rect.x,
                y: rect.y,
                w: 1.0,
                h: rect.h,
            },
            TILE_PANEL_CENTER,
            rgb8_tint(darken_topdown_color(base_color, left), 1.0),
        );
    }

    let top = topdown_edge_strength_for_cell(columns, width, height, x, z, TopdownEdge::Top);
    if top > 0.0 {
        push_fill(
            sprites,
            UiRectPx {
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h: 1.0,
            },
            TILE_PANEL_CENTER,
            rgb8_tint(darken_topdown_color(base_color, top), 1.0),
        );
    }

    let right = topdown_edge_strength_for_cell(columns, width, height, x, z, TopdownEdge::Right);
    if right > 0.0 {
        push_fill(
            sprites,
            UiRectPx {
                x: rect.x + rect.w - 1.0,
                y: rect.y,
                w: 1.0,
                h: rect.h,
            },
            TILE_PANEL_CENTER,
            rgb8_tint(darken_topdown_color(base_color, right), 1.0),
        );
    }

    let bottom = topdown_edge_strength_for_cell(columns, width, height, x, z, TopdownEdge::Bottom);
    if bottom > 0.0 {
        push_fill(
            sprites,
            UiRectPx {
                x: rect.x,
                y: rect.y + rect.h - 1.0,
                w: rect.w,
                h: 1.0,
            },
            TILE_PANEL_CENTER,
            rgb8_tint(darken_topdown_color(base_color, bottom), 1.0),
        );
    }
}

fn rgb8_tint(color: [u8; 3], alpha: f32) -> [f32; 4] {
    [
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
        alpha.clamp(0.0, 1.0),
    ]
}

fn push_ingame_hud(
    sprites: &mut Vec<RenderUiSprite>,
    viewport: [f32; 2],
    inventory: PlayerInventory,
    registry: &BlockRegistry,
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

    push_panel(
        sprites,
        mode_panel,
        [0.18, 0.15, 0.13, 0.98],
        [0.70, 0.63, 0.46, 0.98],
    );
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
    registry: &BlockRegistry,
) {
    let panel = UiRectPx {
        x: (viewport[0] * 0.5 - 360.0).floor(),
        y: (viewport[1] * 0.5 - 226.0).floor(),
        w: 720.0,
        h: 452.0,
    };
    push_panel(
        sprites,
        panel,
        [0.20, 0.17, 0.14, 0.98],
        [0.83, 0.70, 0.44, 0.98],
    );
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
    registry: &BlockRegistry,
) {
    let slot_gap = 6.0;
    let slot_width =
        ((rect.w - slot_gap * (QUICKSLOT_COUNT as f32 - 1.0)) / QUICKSLOT_COUNT as f32).floor();
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
    registry: &BlockRegistry,
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
    push_fill(
        sprites,
        rect.inset(6.0),
        TILE_SLOT_FILL,
        [0.07, 0.09, 0.11, 0.92],
    );

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
    registry: &BlockRegistry,
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

fn push_world_select_section_layout(
    sprites: &mut Vec<RenderUiSprite>,
    section: &WorldSelectSectionLayout,
    selected: bool,
    hovered_action: Option<WorldSelectAction>,
    hovered_input_field: Option<WorldSelectInputField>,
    hovered_world_index: Option<usize>,
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
            hovered_input_field == Some(field.field),
            hovered_action == Some(field.increase_action),
            hovered_action == Some(field.decrease_action),
        );
    }
    if let Some(world_list) = &section.world_list {
        push_world_select_list(sprites, world_list, hovered_world_index);
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
    value_hovered: bool,
    increase_hovered: bool,
    decrease_hovered: bool,
) {
    let label_frame = if field.enabled {
        [0.23, 0.19, 0.15, 1.0]
    } else {
        [0.14, 0.12, 0.11, 0.92]
    };
    let value_frame = if field.enabled && (field.focused || value_hovered) {
        [0.28, 0.40, 0.50, 1.0]
    } else if field.enabled {
        [0.14, 0.18, 0.22, 1.0]
    } else {
        [0.09, 0.10, 0.11, 0.90]
    };
    let label_fill = if field.enabled {
        [0.11, 0.10, 0.09, 0.98]
    } else {
        [0.07, 0.07, 0.07, 0.92]
    };
    let value_fill = if field.enabled && field.focused {
        [0.10, 0.20, 0.28, 0.98]
    } else if field.enabled {
        [0.08, 0.12, 0.16, 0.98]
    } else {
        [0.06, 0.07, 0.08, 0.92]
    };
    let value_text = if field.focused && field.value.is_empty() {
        "_".to_string()
    } else {
        field.value.clone()
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
        &truncate_text_to_width(&value_text, field.value_rect.w - 10.0, 2.0),
        if field.enabled {
            [0.78, 0.86, 0.95, 1.0]
        } else {
            [0.48, 0.52, 0.56, 0.98]
        },
    );

    push_world_select_arrow_button(
        sprites,
        field.increase_rect,
        "UP",
        increase_hovered,
        field.enabled,
    );
    push_world_select_arrow_button(
        sprites,
        field.decrease_rect,
        "DN",
        decrease_hovered,
        field.enabled,
    );
}

fn push_world_select_list(
    sprites: &mut Vec<RenderUiSprite>,
    list: &WorldSelectListLayout,
    hovered_world_index: Option<usize>,
) {
    push_small_panel(
        sprites,
        list.rect,
        [0.24, 0.22, 0.18, 1.0],
        [0.08, 0.10, 0.12, 0.98],
    );

    if list.rows.is_empty() {
        push_text_centered(sprites, list.rect, 2.0, "EMPTY", [0.50, 0.54, 0.58, 0.98]);
        return;
    }

    for row in &list.rows {
        push_world_select_world_row(sprites, row, hovered_world_index == Some(row.world_index));
    }
}

fn push_world_select_world_row(
    sprites: &mut Vec<RenderUiSprite>,
    row: &WorldSelectWorldRowLayout,
    hovered: bool,
) {
    let (frame_tint, fill_tint, title_tint, detail_tint) = if !row.enabled {
        (
            [0.22, 0.22, 0.22, 0.92],
            [0.08, 0.08, 0.08, 0.90],
            [0.44, 0.44, 0.44, 0.96],
            [0.38, 0.38, 0.38, 0.92],
        )
    } else if row.selected {
        (
            [0.88, 0.72, 0.34, 1.0],
            [0.18, 0.14, 0.10, 0.98],
            [0.99, 0.94, 0.82, 1.0],
            [0.88, 0.84, 0.76, 0.98],
        )
    } else if hovered {
        (
            [0.62, 0.54, 0.36, 1.0],
            [0.14, 0.12, 0.10, 0.98],
            [0.95, 0.90, 0.78, 1.0],
            [0.78, 0.80, 0.84, 0.98],
        )
    } else {
        (
            [0.34, 0.32, 0.28, 0.98],
            [0.10, 0.10, 0.11, 0.96],
            [0.86, 0.88, 0.92, 1.0],
            [0.62, 0.66, 0.72, 0.98],
        )
    };

    push_small_panel(sprites, row.rect, frame_tint, fill_tint);
    push_text(
        sprites,
        row.rect.x + 12.0,
        row.rect.y + 8.0,
        2.0,
        &truncate_text_to_width(&row.label, row.rect.w - 24.0, 2.0),
        title_tint,
    );
    push_text(
        sprites,
        row.rect.x + 12.0,
        row.rect.y + 32.0,
        1.0,
        &truncate_text_to_width(&row.detail, row.rect.w - 24.0, 1.0),
        detail_tint,
    );
}

fn push_world_select_loading_popup(
    sprites: &mut Vec<RenderUiSprite>,
    popup: &WorldSelectLoadingPopupLayout,
) {
    push_fill(
        sprites,
        popup.overlay_rect,
        TILE_PANEL_CENTER,
        [0.02, 0.03, 0.05, 0.78],
    );
    push_panel(
        sprites,
        popup.rect,
        [0.22, 0.18, 0.14, 1.0],
        [0.86, 0.70, 0.30, 1.0],
    );
    push_fill(
        sprites,
        popup.rect.inset(18.0),
        TILE_PANEL_INSET,
        [0.08, 0.10, 0.12, 0.98],
    );
    push_text_centered(
        sprites,
        popup.title_rect,
        2.0,
        &popup.title,
        [0.98, 0.93, 0.82, 1.0],
    );
    push_text_centered(
        sprites,
        popup.message_rect,
        3.0,
        &popup.message,
        [0.92, 0.96, 0.98, 1.0],
    );
    push_text_centered(
        sprites,
        popup.detail_rect,
        2.0,
        &popup.detail,
        [0.80, 0.86, 0.94, 1.0],
    );
    push_small_panel(
        sprites,
        popup.progress_track_rect,
        [0.34, 0.30, 0.24, 1.0],
        [0.05, 0.07, 0.09, 0.98],
    );
    push_fill(
        sprites,
        popup.progress_fill_rect,
        TILE_PANEL_HEADER,
        [0.70, 0.86, 0.96, 1.0],
    );
    push_text_centered(
        sprites,
        popup.progress_label_rect,
        1.0,
        &popup.progress_label,
        [0.98, 0.99, 1.0, 1.0],
    );
}

fn push_world_select_info_line(
    sprites: &mut Vec<RenderUiSprite>,
    info_line: &WorldSelectInfoLineLayout,
) {
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
            (
                [0.90, 0.73, 0.32, 1.0],
                [0.21, 0.16, 0.10, 1.0],
                [0.99, 0.94, 0.82, 1.0],
            )
        } else {
            (
                [0.64, 0.55, 0.36, 1.0],
                [0.16, 0.13, 0.10, 0.98],
                [0.95, 0.88, 0.74, 1.0],
            )
        }
    } else {
        (
            [0.28, 0.26, 0.22, 0.94],
            [0.09, 0.09, 0.09, 0.90],
            [0.45, 0.45, 0.45, 0.96],
        )
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
            (
                [0.86, 0.70, 0.30, 1.0],
                [0.22, 0.16, 0.10, 1.0],
                [0.99, 0.94, 0.82, 1.0],
            )
        } else {
            (
                [0.56, 0.50, 0.37, 1.0],
                [0.14, 0.12, 0.10, 0.98],
                [0.90, 0.84, 0.72, 1.0],
            )
        }
    } else {
        (
            [0.26, 0.24, 0.22, 0.94],
            [0.08, 0.08, 0.08, 0.90],
            [0.42, 0.42, 0.42, 0.96],
        )
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

fn push_fill(sprites: &mut Vec<RenderUiSprite>, rect: UiRectPx, tile: (u32, u32), tint: [f32; 4]) {
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
