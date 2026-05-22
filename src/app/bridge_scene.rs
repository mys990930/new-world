use super::{
    AppMode, GameApp,
    bridge::AppRenderFrameData,
    bridge_ui::{build_ingame_ui_sprites, build_world_select_ui_sprites},
    ui::build_world_select_layout,
};
#[cfg(test)]
use crate::ecs::VOXEL_PLAYER_PART_COUNT;
#[cfg(test)]
use crate::ecs::quarter_view_camera_pose;
use crate::ecs::{
    CameraState, InventoryItem, PlayerInventory, VoxelPlayerAnimationState,
    VoxelPlayerFacingOctant, VoxelPlayerPart, VoxelPlayerPartPose, VoxelPlayerVisualState,
    default_voxel_player_part_poses, quarter_view_render_camera_pose,
};
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
const PLAYER_TEXTURE_LAYER: u32 = 0;

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
                let player_visual = self.ecs.local_player_visual_state();
                let local_environment = self.ecs.local_environment_status();
                let mut cube_instances = player_visual
                    .map(build_voxel_player_instances)
                    .unwrap_or_default();

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
                let layout = build_world_select_layout(
                    &self.ui.world_select,
                    viewport,
                    current_loaded_label,
                    self.created_world.is_some(),
                    self.timing.frame_index,
                );
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

fn build_voxel_player_instances(visual: VoxelPlayerVisualState) -> Vec<RenderCubeInstance> {
    default_voxel_player_part_poses()
        .into_iter()
        .map(|part| build_voxel_player_part_instance(visual, part))
        .collect()
}

fn build_voxel_player_part_instance(
    visual: VoxelPlayerVisualState,
    part: VoxelPlayerPartPose,
) -> RenderCubeInstance {
    let animated_center = animated_voxel_player_local_center(visual, part);
    let facing_center = rotate_player_local_offset(animated_center, visual.facing);
    let facing_half_extents = rotate_player_local_half_extents(part.half_extents, visual.facing);

    RenderCubeInstance {
        center: [
            visual.root_translation[0] + facing_center[0],
            visual.root_translation[1] + facing_center[1],
            visual.root_translation[2] + facing_center[2],
        ],
        half_extents: facing_half_extents,
        color: part.color,
        top_texture_layer: PLAYER_TEXTURE_LAYER,
        bottom_texture_layer: PLAYER_TEXTURE_LAYER,
        side_texture_layer: PLAYER_TEXTURE_LAYER,
        material_kind: RenderMaterialKind::Actor,
    }
}

fn animated_voxel_player_local_center(
    visual: VoxelPlayerVisualState,
    part: VoxelPlayerPartPose,
) -> [f32; 3] {
    let mut center = part.local_center;
    let speed_scale = (visual.horizontal_speed / 11.0).clamp(0.0, 1.0);
    let phase_rate = match visual.animation_state {
        VoxelPlayerAnimationState::Idle => 2.0,
        VoxelPlayerAnimationState::Walk => 7.0,
        VoxelPlayerAnimationState::Sprint => 10.0,
        VoxelPlayerAnimationState::Airborne => 4.0,
    };
    let phase = visual.animation_seconds * phase_rate;

    match visual.animation_state {
        VoxelPlayerAnimationState::Idle => {
            let bob = phase.sin() * 0.025;
            if !matches!(
                part.part,
                VoxelPlayerPart::LeftLeg | VoxelPlayerPart::RightLeg
            ) {
                center[1] += bob;
            }
        }
        VoxelPlayerAnimationState::Walk | VoxelPlayerAnimationState::Sprint => {
            let stride = phase.sin();
            let lift = phase.cos().max(0.0);
            let swing = 0.14 + 0.18 * speed_scale;
            let bob = (phase * 2.0).sin().abs() * (0.025 + 0.035 * speed_scale);
            center[1] += bob;
            match part.part {
                VoxelPlayerPart::LeftArm => {
                    center[2] -= stride * swing * 0.8;
                    center[1] += lift * 0.03;
                }
                VoxelPlayerPart::RightArm => {
                    center[2] += stride * swing * 0.8;
                    center[1] += (-phase).cos().max(0.0) * 0.03;
                }
                VoxelPlayerPart::LeftLeg => {
                    center[2] += stride * swing;
                    center[1] += lift * 0.08;
                }
                VoxelPlayerPart::RightLeg => {
                    center[2] -= stride * swing;
                    center[1] += (-phase).cos().max(0.0) * 0.08;
                }
                VoxelPlayerPart::Head | VoxelPlayerPart::Torso => {}
            }
        }
        VoxelPlayerAnimationState::Airborne => match part.part {
            VoxelPlayerPart::LeftArm => {
                center[1] += 0.16;
                center[2] -= 0.10;
            }
            VoxelPlayerPart::RightArm => {
                center[1] += 0.16;
                center[2] -= 0.10;
            }
            VoxelPlayerPart::LeftLeg => {
                center[1] += 0.10;
                center[2] += 0.12;
            }
            VoxelPlayerPart::RightLeg => {
                center[1] += 0.08;
                center[2] -= 0.10;
            }
            VoxelPlayerPart::Head | VoxelPlayerPart::Torso => {
                center[1] += phase.sin() * 0.02;
            }
        },
    }

    center
}

fn rotate_player_local_offset(local: [f32; 3], facing: VoxelPlayerFacingOctant) -> [f32; 3] {
    let yaw = facing.0 as f32 * std::f32::consts::FRAC_PI_4;
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    [
        local[0] * cos_yaw + local[2] * sin_yaw,
        local[1],
        -local[0] * sin_yaw + local[2] * cos_yaw,
    ]
}

fn rotate_player_local_half_extents(
    half_extents: [f32; 3],
    facing: VoxelPlayerFacingOctant,
) -> [f32; 3] {
    let yaw = facing.0 as f32 * std::f32::consts::FRAC_PI_4;
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let abs_sin = sin_yaw.abs();
    let abs_cos = cos_yaw.abs();
    [
        half_extents[0] * abs_cos + half_extents[2] * abs_sin,
        half_extents[1],
        half_extents[0] * abs_sin + half_extents[2] * abs_cos,
    ]
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
            center: [
                block.0 as f32 + 0.5,
                block.1 as f32 + 0.5,
                block.2 as f32 + 0.5,
            ],
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
        vertices: mesh
            .vertices
            .into_iter()
            .map(world_vertex_to_render)
            .collect(),
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
        let basis = camera
            .basis_override
            .expect("quarter-view basis should exist");

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
        let basis = camera
            .basis_override
            .expect("quarter-view basis should exist");

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

    #[test]
    fn voxel_player_bridge_emits_one_actor_cube_per_part() {
        let instances = build_voxel_player_instances(test_visual_state(0.0));

        assert_eq!(instances.len(), VOXEL_PLAYER_PART_COUNT);
        assert!(
            instances
                .iter()
                .all(|instance| instance.material_kind == RenderMaterialKind::Actor)
        );
    }

    #[test]
    fn voxel_player_bridge_animates_limb_centers_over_time() {
        let at_rest = build_voxel_player_instances(test_visual_state(0.0));
        let moving = build_voxel_player_instances(test_visual_state(0.25));

        assert_ne!(at_rest[4].center, moving[4].center);
        assert_ne!(at_rest[5].center, moving[5].center);
    }

    #[test]
    fn voxel_player_bridge_rotates_local_forward_to_facing() {
        let north = rotate_player_local_offset([0.0, 0.0, 1.0], VoxelPlayerFacingOctant::NORTH);
        let east = rotate_player_local_offset([0.0, 0.0, 1.0], VoxelPlayerFacingOctant::EAST);

        assert!((north[2] - 1.0).abs() < 1e-5);
        assert!(north[0].abs() < 1e-5);
        assert!((east[0] - 1.0).abs() < 1e-5);
        assert!(east[2].abs() < 1e-5);
    }

    #[test]
    fn voxel_player_bridge_rotates_part_extents_to_facing_aabb() {
        let local = [0.46, 0.55, 0.28];
        let north = rotate_player_local_half_extents(local, VoxelPlayerFacingOctant::NORTH);
        let east = rotate_player_local_half_extents(local, VoxelPlayerFacingOctant::EAST);

        assert_eq!(north, local);
        assert!((east[0] - local[2]).abs() < 1e-5);
        assert!((east[1] - local[1]).abs() < 1e-5);
        assert!((east[2] - local[0]).abs() < 1e-5);
    }

    fn test_visual_state(animation_seconds: f32) -> VoxelPlayerVisualState {
        VoxelPlayerVisualState {
            root_translation: [10.0, 20.0, 30.0],
            facing: VoxelPlayerFacingOctant::NORTH,
            animation_state: VoxelPlayerAnimationState::Walk,
            horizontal_speed: 7.0,
            grounded: true,
            animation_seconds,
        }
    }
}
