use super::GameApp;
use crate::jobs::JobResult;
use crate::renderer::RenderFrameInput;

impl GameApp {
    pub fn update(&mut self) {
        self.handle_ui_shortcuts();

        if !self.gameplay_active() {
            self.collect_job_results();
            return;
        }

        self.ecs
            .set_frame_delta_seconds(self.timing.frame_dt.as_secs_f32());
        self.bridge_platform_to_ecs();
        self.ecs.run_pre_update();
        self.ecs.run_update();
        self.collect_job_results();
        self.ecs.simulate_local_player_motion(&self.world);
        self.ecs.run_post_update();

        let requests = self
            .ecs
            .plan_chunk_job_requests(&self.world, self.created_world.as_ref());
        if let Err(error) = self.jobs.submit_all(requests) {
            eprintln!("[app] jobs submit failed: {:?}", error);
        }

        self.collect_job_results();

        let window = self.platform.window_state();
        self.ecs
            .update_selection_from_world(&self.world, window.width, window.height);
        self.log_clicked_block();

        let commands = self.ecs.drain_player_commands();
        if !commands.is_empty() {
            println!("[app] ecs commands: {:?}", commands);
        }
    }

    pub fn render(&mut self) {
        let (width, height) = {
            let window = self.platform.window_state();
            (window.width, window.height)
        };

        if self.renderer.surface_state().width() != width
            || self.renderer.surface_state().height() != height
        {
            if let Err(error) = self.renderer.resize(width, height) {
                eprintln!("[app] renderer resize failed: {:?}", error);
                return;
            }
        }

        let render_frame = self.bridge_app_to_render_frame();

        if let Err(error) = self.renderer.render(RenderFrameInput {
            camera: &render_frame.camera,
            draw_scene: render_frame.draw_scene,
            visible_chunks: &render_frame.visible_chunks,
            cube_instances: &render_frame.cube_instances,
            ui_sprites: &render_frame.ui_sprites,
            clear_color_override: render_frame.clear_color_override,
        }) {
            eprintln!("[app] renderer frame failed: {:?}", error);
        }
    }

    fn collect_job_results(&mut self) {
        for result in self.jobs.drain_completed() {
            self.ecs.apply_job_result(&result);

            match result {
                JobResult::WorldCreated { root, manifest } => {
                    self.handle_world_created_result(root, manifest);
                }
                JobResult::ChunkLoaded { coord, chunk } => {
                    self.world.insert_chunk(coord, chunk);
                }
                JobResult::ChunkGenerated { coord, chunk } => {
                    self.world.insert_chunk(coord, chunk);
                }
                JobResult::ChunkMeshBuilt { coord, mesh } => {
                    if let Err(error) = self.renderer.apply_upload(
                        self.bridge_world_mesh_to_render_upload(coord, mesh),
                    ) {
                        eprintln!("[app] renderer upload failed for {:?}: {:?}", coord, error);
                    }
                }
                JobResult::JobFailed { request, error } => {
                    self.handle_job_failure(&request, error.clone());
                    eprintln!("[app] job failed for {:?}: {:?}", request, error);
                }
            }
        }
    }

    fn log_clicked_block(&self) {
        let input = self.platform.raw_input_state();
        if !input.left_just_pressed && !input.right_just_pressed {
            return;
        }

        let current_selection = self.ecs.selection_state();
        let Some(block_pos) = current_selection.hovered_block else {
            return;
        };
        let Some(block_id) = self.world.get_block(block_pos) else {
            return;
        };

        let block = self.world.block_registry().block_or_missing(block_id);
        let trigger = if input.left_just_pressed { "left" } else { "right" };
        println!(
            "[app] clicked block: button={} key={} id={} pos=({}, {}, {}) face={:?}",
            trigger,
            block.key,
            block_id.raw(),
            block_pos.0,
            block_pos.1,
            block_pos.2,
            current_selection.hovered_face
        );
    }
}
