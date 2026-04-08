use super::GameApp;
use crate::jobs::JobResult;
use crate::renderer::RenderFrameInput;

impl GameApp {
    pub fn update(&mut self) {
        self.bridge_platform_to_ecs();
        self.ecs.run_pre_update();
        self.ecs.run_update();
        self.ecs.run_post_update();
        self.collect_job_results();

        let requests = self.ecs.plan_chunk_job_requests(&self.world);
        if let Err(error) = self.jobs.submit_all(requests) {
            eprintln!("[app] jobs submit failed: {:?}", error);
        }

        self.collect_job_results();

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

        let render_frame = self.bridge_ecs_to_render_frame();

        if let Err(error) = self.renderer.render(RenderFrameInput {
            camera: &render_frame.camera,
            visible_chunks: &render_frame.visible_chunks,
            cube_instances: &render_frame.cube_instances,
            clear_color_override: None,
        }) {
            eprintln!("[app] renderer frame failed: {:?}", error);
        }
    }

    fn collect_job_results(&mut self) {
        for result in self.jobs.drain_completed() {
            self.ecs.apply_job_result(&result);

            match result {
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
                    eprintln!("[app] job failed for {:?}: {:?}", request, error);
                }
            }
        }
    }
}
