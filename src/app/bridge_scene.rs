use super::{
    bridge::AppRenderFrameData,
    bridge_ui::{build_ingame_ui_sprites, build_world_select_ui_sprites},
    ui::build_world_select_layout,
    AppMode,
    GameApp,
};
use crate::ecs::{
    quarter_view_render_camera_pose, CameraState, InventoryItem, PlayerInventory,
};
#[cfg(test)]
use crate::ecs::quarter_view_camera_pose;
use crate::renderer::{
    ChunkCoord as RenderChunkCoord, CpuMesh as RenderCpuMesh, MeshVertex as RenderMeshVertex,
    RenderCameraState, RenderCubeInstance, RenderMaterialKind, RenderProjectionMode,
    RenderUploadRequest, RenderViewBasis,
};
use crate::world::{
    BlockId, BlockMaterialKind, ChunkCoord as WorldChunkCoord, CpuMesh as WorldCpuMesh,
    MeshVertex as WorldMeshVertex, WorldCore,
};

const DEFAULT_PREVIEW_BLOCK_ID: BlockId = BlockId::STONE;

impl GameApp {
    pub fn bridge_app_to_render_frame(&self) -> AppRenderFrameData {
        let camera_state = self.ecs.camera_state();
        let camera = build_quarter_view_camera(camera_state);
        let window = self.platform.window_state();
        let viewport = [window.width as f32, window.height as f32];

        match self.ui.mode {
            AppMode::InGame => {
                let inventory = self.ecs.local_player_inventory();
                let player_transform = self.ecs.local_player_transform();
                let player_body = self.ecs.local_player_body();
                let local_environment = self.ecs.local_environment_status();
                let mut cube_instances = player_transform
                    .zip(player_body)
                    .map(|(transform, body)| {
                        vec![RenderCubeInstance {
                            center: transform.translation,
                            half_extents: body.half_extents,
                            color: [1.0, 1.0, 1.0, 1.0],
                            top_texture_layer: 0,
                            bottom_texture_layer: 0,
                            side_texture_layer: 0,
                            material_kind: RenderMaterialKind::Actor,
                        }]
                    })
                    .unwrap_or_default();

                if let Some((player, body)) = player_transform.zip(player_body) {
                    cube_instances.insert(
                        0,
                        build_ground_shadow_instance(player.translation, body.half_extents),
                    );
                }

                let selection = self.ecs.selection_state();
                push_selection_preview_instances(
                    &mut cube_instances,
                    &selection,
                    &self.world,
                    inventory,
                );
                let minimap_viewport = player_transform.map(|transform| {
                    self.minimap
                        .compose_viewport(transform.translation[0], transform.translation[2])
                });

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
                        player_transform,
                        local_environment,
                        self.world.block_registry(),
                        minimap_viewport.as_ref(),
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
                let mouse_position = self.platform.raw_input_state().mouse_position;
                let hovered_action = layout.action_at(mouse_position);
                let hovered_input_field = layout.input_field_at(mouse_position);
                let hovered_world_index = layout.created_world_index_at(mouse_position);

                AppRenderFrameData {
                    camera,
                    draw_scene: false,
                    visible_chunks: Vec::new(),
                    cube_instances: Vec::new(),
                    ui_sprites: build_world_select_ui_sprites(
                        &layout,
                        &self.ui.world_select,
                        hovered_action,
                        hovered_input_field,
                        hovered_world_index,
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
        top_texture_layer: 0,
        bottom_texture_layer: 0,
        side_texture_layer: 0,
        material_kind: RenderMaterialKind::Shadow,
    }
}

fn push_selection_preview_instances(
    cube_instances: &mut Vec<RenderCubeInstance>,
    selection: &crate::ecs::SelectionState,
    world: &WorldCore,
    inventory: Option<PlayerInventory>,
) {
    for preview in &selection.interaction_preview_blocks {
        let face_textures = world
            .get_block(preview.block)
            .map(|block| block_face_texture_layers(world.block_registry(), block))
            .unwrap_or_else(|| default_preview_texture_layers(world.block_registry()));
        cube_instances.push(RenderCubeInstance {
            center: [
                preview.block.0 as f32 + 0.5,
                preview.block.1 as f32 + 0.5,
                preview.block.2 as f32 + 0.5,
            ],
            half_extents: [0.505, 0.505, 0.505],
            color: [1.0, 0.22, 0.22, 0.18],
            top_texture_layer: face_textures[0],
            bottom_texture_layer: face_textures[1],
            side_texture_layer: face_textures[2],
            material_kind: RenderMaterialKind::Highlight,
        });
    }

    if let Some(block) = selection.build_preview_block {
        let face_textures = inventory
            .and_then(|player_inventory| player_inventory.selected_block())
            .and_then(|slot| match slot.item {
                InventoryItem::Block(block_id) => {
                    Some(block_face_texture_layers(world.block_registry(), block_id))
                }
                InventoryItem::Tool(_) => None,
            })
            .unwrap_or_else(|| default_preview_texture_layers(world.block_registry()));
        cube_instances.push(RenderCubeInstance {
            center: [block.0 as f32 + 0.5, block.1 as f32 + 0.5, block.2 as f32 + 0.5],
            half_extents: [0.49, 0.49, 0.49],
            color: [1.0, 0.95, 0.35, 0.35],
            top_texture_layer: face_textures[0],
            bottom_texture_layer: face_textures[1],
            side_texture_layer: face_textures[2],
            material_kind: RenderMaterialKind::Highlight,
        });
    }
}

fn default_preview_texture_layers(registry: &crate::world::BlockRegistry) -> [u32; 3] {
    block_face_texture_layers(registry, DEFAULT_PREVIEW_BLOCK_ID)
}

fn block_face_texture_layers(
    registry: &crate::world::BlockRegistry,
    block_id: BlockId,
) -> [u32; 3] {
    let def = registry.block_or_missing(block_id);
    [
        u32::from(def.face_textures.top.0),
        u32::from(def.face_textures.bottom.0),
        u32::from(def.face_textures.side.0),
    ]
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
mod tests {
    use super::*;

    fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
        left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
    }

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

    fn project_to_screen_axes(point: [f32; 3], basis: RenderViewBasis) -> (f32, f32) {
        (dot3(point, basis.right), dot3(point, basis.up))
    }
}
