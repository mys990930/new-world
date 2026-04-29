use std::time::{Duration, Instant};

use super::GameApp;
use crate::jobs::{JobRequest, JobRequestCounts, JobResult};
use crate::renderer::RenderFrameInput;

const MAX_GAMEPLAY_JOB_RESULTS_PER_FRAME: usize = 4;
const CHUNK_LIFECYCLE_LOG_INTERVAL_FRAMES: u64 = 30;
const SLOW_RENDER_UPLOAD_LOG_THRESHOLD: Duration = Duration::from_millis(8);

impl GameApp {
    pub fn update(&mut self) {
        let update_start = Instant::now();
        let mut timings = AppUpdateTimings::default();
        let mut result_stats = JobResultApplyStats::default();
        let mut lifecycle_stats = ChunkLifecycleRequestStats::default();

        let stage_start = Instant::now();
        self.handle_ui_shortcuts();
        timings.shortcuts_ms = duration_ms(stage_start.elapsed());

        if !self.gameplay_active() {
            let stage_start = Instant::now();
            result_stats.add(self.collect_job_results_with_limit(usize::MAX));
            timings.collect_jobs_ms = duration_ms(stage_start.elapsed());
            self.log_update_perf_if_needed(
                update_start.elapsed(),
                &timings,
                &result_stats,
                &lifecycle_stats,
            );
            return;
        }

        self.ecs
            .set_frame_delta_seconds(self.timing.frame_dt.as_secs_f32());
        let stage_start = Instant::now();
        self.run_fixed_updates();
        timings.fixed_ms = duration_ms(stage_start.elapsed());
        let stage_start = Instant::now();
        self.bridge_platform_to_ecs();
        timings.bridge_input_ms = duration_ms(stage_start.elapsed());
        let stage_start = Instant::now();
        self.ecs.run_pre_update();
        timings.ecs_pre_ms = duration_ms(stage_start.elapsed());
        let stage_start = Instant::now();
        self.ecs.run_update();
        timings.ecs_update_ms = duration_ms(stage_start.elapsed());
        let mut job_result_budget = MAX_GAMEPLAY_JOB_RESULTS_PER_FRAME;
        let stage_start = Instant::now();
        result_stats.add(self.collect_job_results_with_budget(&mut job_result_budget));
        timings.collect_jobs_ms = duration_ms(stage_start.elapsed());
        let stage_start = Instant::now();
        self.try_place_pending_player_spawn();
        timings.spawn_ms = duration_ms(stage_start.elapsed());
        if self.pending_player_spawn_anchor.is_none() {
            let stage_start = Instant::now();
            self.ecs.simulate_local_player_motion(&self.world);
            timings.player_motion_ms = duration_ms(stage_start.elapsed());
        }
        let stage_start = Instant::now();
        self.ecs.run_post_update();
        timings.ecs_post_ms = duration_ms(stage_start.elapsed());

        let stage_start = Instant::now();
        let lifecycle = self
            .ecs
            .plan_chunk_lifecycle(&self.world, self.created_world.as_ref());
        timings.lifecycle_plan_ms = duration_ms(stage_start.elapsed());
        lifecycle_stats = ChunkLifecycleRequestStats::from_lifecycle(&lifecycle);
        self.log_chunk_lifecycle_plan(&lifecycle);
        let stage_start = Instant::now();
        self.apply_chunk_unloads(&lifecycle.unload_coords);
        timings.unload_ms = duration_ms(stage_start.elapsed());
        let stage_start = Instant::now();
        if let Err(error) = self.jobs.submit_all(lifecycle.job_requests) {
            eprintln!("[app] jobs submit failed: {:?}", error);
        }
        timings.submit_jobs_ms = duration_ms(stage_start.elapsed());

        let stage_start = Instant::now();
        result_stats.add(self.collect_job_results_with_budget(&mut job_result_budget));
        timings.collect_jobs_after_submit_ms = duration_ms(stage_start.elapsed());
        let stage_start = Instant::now();
        self.try_place_pending_player_spawn();
        timings.spawn_after_submit_ms = duration_ms(stage_start.elapsed());
        self.queue_environment_region_resolve_for_focus();

        let window = self.platform.window_state();
        let stage_start = Instant::now();
        self.ecs.update_local_environment_from_world(&self.world);
        timings.environment_ms = duration_ms(stage_start.elapsed());
        let stage_start = Instant::now();
        self.ecs
            .update_selection_from_world(&self.world, window.width, window.height);
        timings.selection_ms = duration_ms(stage_start.elapsed());
        let stage_start = Instant::now();
        self.log_clicked_block();
        timings.click_log_ms = duration_ms(stage_start.elapsed());

        let stage_start = Instant::now();
        let _ = self.ecs.drain_player_commands();
        timings.command_drain_ms = duration_ms(stage_start.elapsed());
        self.log_update_perf_if_needed(
            update_start.elapsed(),
            &timings,
            &result_stats,
            &lifecycle_stats,
        );
    }

    pub fn render(&mut self) {
        let render_start = Instant::now();
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

        let bridge_start = Instant::now();
        let render_frame = self.bridge_app_to_render_frame();
        let bridge_ms = duration_ms(bridge_start.elapsed());
        let visible_chunks = render_frame.visible_chunks.len();
        let cube_instances = render_frame.cube_instances.len();
        let ui_sprites = render_frame.ui_sprites.len();
        let draw_scene = render_frame.draw_scene;

        let draw_start = Instant::now();
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
        let draw_ms = duration_ms(draw_start.elapsed());
        let total = render_start.elapsed();
        if trace_frame_perf_logs_enabled() {
            println!(
                "[perf] render frame: frame={} total_ms={:.2} bridge_ms={:.2} draw_ms={:.2} draw_scene={} visible_chunks={} cube_instances={} ui_sprites={} surface={}x{}",
                self.timing.frame_index,
                duration_ms(total),
                bridge_ms,
                draw_ms,
                draw_scene,
                visible_chunks,
                cube_instances,
                ui_sprites,
                width,
                height
            );
        }
    }

    fn collect_job_results_with_budget(&mut self, budget: &mut usize) -> JobResultApplyStats {
        if *budget == 0 {
            return JobResultApplyStats::default();
        }

        let stats = self.collect_job_results_with_limit(*budget);
        *budget = budget.saturating_sub(stats.processed);
        stats
    }

    fn collect_job_results_with_limit(&mut self, max_results: usize) -> JobResultApplyStats {
        let results = self.jobs.drain_completed_limit(max_results);
        let mut stats = JobResultApplyStats {
            processed: results.len(),
            ..JobResultApplyStats::default()
        };
        for result in results {
            self.ecs.apply_job_result(&result);

            match result {
                JobResult::CreateWorldProgress {
                    root,
                    completed_chunks,
                    total_chunks,
                } => {
                    stats.create_progress += 1;
                    println!(
                        "[app] create-world progress: root={} chunks={}/{}",
                        root.display(),
                        completed_chunks,
                        total_chunks
                    );
                    self.handle_create_world_progress(
                        root.as_path(),
                        completed_chunks,
                        total_chunks,
                    );
                }
                JobResult::WorldCreated { root, manifest } => {
                    stats.world_created += 1;
                    let min = manifest.min_chunk_coord();
                    let max = manifest.max_chunk_coord();
                    println!(
                        "[app] world created: root={} min=({}, {}, {}) max=({}, {}, {}) preview=({}, {}) stacks={}",
                        root.display(),
                        min.0,
                        min.1,
                        min.2,
                        max.0,
                        max.1,
                        max.2,
                        manifest.default_preview_center[0],
                        manifest.default_preview_center[1],
                        manifest.stacks.len()
                    );
                    self.handle_world_created_result(root, manifest);
                }
                JobResult::ChunkLoaded { coord, chunk } => {
                    if self.ecs.retains_chunk(coord) {
                        stats.disk_loaded += 1;
                        self.world.insert_chunk(coord, chunk);
                        println!(
                            "[app] chunk loaded: pos=({}, {}, {}) source=disk",
                            coord.0, coord.1, coord.2
                        );
                        self.refresh_minimap_chunk_column_after_world_change(
                            crate::world::TopdownChunkColumnCoord {
                                chunk_x: coord.0,
                                chunk_z: coord.2,
                            },
                        );
                    } else {
                        stats.ignored += 1;
                        println!(
                            "[app] chunk load ignored: pos=({}, {}, {}) reason=outside-retain",
                            coord.0, coord.1, coord.2
                        );
                    }
                }
                JobResult::ChunkGenerated { coord, chunk } => {
                    if self.ecs.retains_chunk(coord) {
                        stats.generated += 1;
                        self.world.insert_chunk(coord, chunk);
                        println!(
                            "[app] chunk loaded: pos=({}, {}, {}) source=generated",
                            coord.0, coord.1, coord.2
                        );
                        self.refresh_minimap_chunk_column_after_world_change(
                            crate::world::TopdownChunkColumnCoord {
                                chunk_x: coord.0,
                                chunk_z: coord.2,
                            },
                        );
                    } else {
                        stats.ignored += 1;
                        println!(
                            "[app] chunk generation ignored: pos=({}, {}, {}) reason=outside-retain",
                            coord.0, coord.1, coord.2
                        );
                    }
                }
                JobResult::ChunkMeshBuilt { coord, mesh } => {
                    if self.ecs.retains_chunk(coord) && self.world.has_chunk(coord) {
                        let triangles = mesh.triangle_count();
                        stats.mesh_triangles += triangles;
                        if mesh.is_empty() {
                            stats.mesh_empty += 1;
                            self.renderer.remove_chunk_mesh(crate::renderer::ChunkCoord(
                                coord.0, coord.1, coord.2,
                            ));
                            println!(
                                "[app] chunk mesh empty: pos=({}, {}, {})",
                                coord.0, coord.1, coord.2
                            );
                        } else {
                            let upload_start = Instant::now();
                            let upload_result = self
                                .renderer
                                .apply_upload(self.bridge_world_mesh_to_render_upload(coord, mesh));
                            let upload_ms = duration_ms(upload_start.elapsed());
                            stats.mesh_upload_ms += upload_ms;
                            if let Err(error) = upload_result {
                                stats.failed += 1;
                                eprintln!(
                                    "[app] renderer upload failed for {:?}: {:?}",
                                    coord, error
                                );
                            } else {
                                stats.mesh_uploaded += 1;
                                if upload_start.elapsed() >= SLOW_RENDER_UPLOAD_LOG_THRESHOLD {
                                    println!(
                                        "[perf] slow mesh upload: pos=({}, {}, {}) triangles={} elapsed_ms={:.2}",
                                        coord.0, coord.1, coord.2, triangles, upload_ms
                                    );
                                }
                                println!(
                                    "[app] chunk mesh uploaded: pos=({}, {}, {}) triangles={}",
                                    coord.0, coord.1, coord.2, triangles
                                );
                            }
                        }
                    } else {
                        stats.ignored += 1;
                        println!(
                            "[app] chunk mesh ignored: pos=({}, {}, {}) reason=outside-retain-or-missing-world",
                            coord.0, coord.1, coord.2
                        );
                    }
                }
                JobResult::MinimapChunkColumnBuilt { coord, patch } => {
                    stats.minimap += 1;
                    self.handle_minimap_chunk_column_built(coord, patch);
                }
                JobResult::RegionClassResolved { area: _, classes } => {
                    stats.region_class_resolved += 1;
                    self.world.cache_region_class_map(&classes);
                    self.sync_renderer_environment_from_world();
                }
                JobResult::JobFailed { request, error } => {
                    stats.failed += 1;
                    self.handle_job_failure(&request, error.clone());
                    eprintln!("[app] job failed for {:?}: {:?}", request, error);
                }
            }
        }
        stats
    }

    fn apply_chunk_unloads(&mut self, unload_coords: &[crate::world::ChunkCoord]) {
        use std::collections::BTreeSet;

        let mut affected_columns = BTreeSet::new();
        for &coord in unload_coords {
            let had_chunk = self.world.has_chunk(coord);
            self.world.remove_chunk(coord);
            self.ecs.apply_chunk_unloaded(coord);
            self.renderer
                .remove_chunk_mesh(crate::renderer::ChunkCoord(coord.0, coord.1, coord.2));
            if had_chunk {
                println!(
                    "[app] chunk unloaded: pos=({}, {}, {})",
                    coord.0, coord.1, coord.2
                );
            }
            affected_columns.insert(crate::world::TopdownChunkColumnCoord {
                chunk_x: coord.0,
                chunk_z: coord.2,
            });
        }

        for column in affected_columns {
            self.refresh_minimap_chunk_column_after_world_change(column);
        }
    }

    fn try_place_pending_player_spawn(&mut self) {
        let Some(anchor) = self.pending_player_spawn_anchor else {
            return;
        };

        if self.ecs.place_local_player_on_surface(&self.world, anchor) {
            self.pending_player_spawn_anchor = None;
            println!(
                "[app] player placed on streamed surface near [{:.1}, {:.1}]",
                anchor[0], anchor[1]
            );
        } else if self.timing.frame_index % 60 == 0 {
            println!(
                "[app] waiting for streamed spawn surface near [{:.1}, {:.1}]; loaded_bounds={:?}",
                anchor[0],
                anchor[1],
                self.world.loaded_chunk_bounds()
            );
        }
    }

    fn log_chunk_lifecycle_plan(&self, lifecycle: &crate::ecs::ChunkLifecyclePlan) {
        if lifecycle.job_requests.is_empty() && lifecycle.unload_coords.is_empty() {
            return;
        }
        if lifecycle.unload_coords.is_empty()
            && self.timing.frame_index % CHUNK_LIFECYCLE_LOG_INTERVAL_FRAMES != 0
        {
            return;
        }

        let mut load = 0_usize;
        let mut generate = 0_usize;
        let mut mesh = 0_usize;
        let mut minimap = 0_usize;
        let mut create_world = 0_usize;
        for request in &lifecycle.job_requests {
            match request {
                JobRequest::CreateWorld { .. } => create_world += 1,
                JobRequest::LoadChunk { .. } => load += 1,
                JobRequest::GenerateChunk { .. } => generate += 1,
                JobRequest::BuildChunkMesh { .. } => mesh += 1,
                JobRequest::BuildMinimapChunkColumn { .. } => minimap += 1,
                JobRequest::ResolveRegionClassArea { .. } => {}
            }
        }

        println!(
            "[app] chunk lifecycle plan: interest={} retain={} requests={} load={} generate={} mesh={} minimap={} create_world={} unload={} spawn_pending={}",
            lifecycle.interest.len(),
            lifecycle.retain.len(),
            lifecycle.job_requests.len(),
            load,
            generate,
            mesh,
            minimap,
            create_world,
            lifecycle.unload_coords.len(),
            self.pending_player_spawn_anchor.is_some()
        );
    }

    fn log_update_perf_if_needed(
        &self,
        total: Duration,
        timings: &AppUpdateTimings,
        results: &JobResultApplyStats,
        lifecycle: &ChunkLifecycleRequestStats,
    ) {
        if !trace_frame_perf_logs_enabled() {
            return;
        }

        let jobs = self.jobs.diagnostic_snapshot();
        let minimap = self.minimap.diagnostic_snapshot();
        println!(
            "[perf] update frame: frame={} total_ms={:.2} frame_dt_ms={:.2} shortcuts_ms={:.2} fixed_ms={:.2} bridge_input_ms={:.2} ecs_pre_ms={:.2} ecs_update_ms={:.2} collect_jobs_ms={:.2}+{:.2} spawn_ms={:.2}+{:.2} motion_ms={:.2} ecs_post_ms={:.2} lifecycle_ms={:.2} unload_ms={:.2} submit_ms={:.2} env_ms={:.2} selection_ms={:.2} click_ms={:.2} command_ms={:.2}",
            self.timing.frame_index,
            duration_ms(total),
            duration_ms(self.timing.frame_dt),
            timings.shortcuts_ms,
            timings.fixed_ms,
            timings.bridge_input_ms,
            timings.ecs_pre_ms,
            timings.ecs_update_ms,
            timings.collect_jobs_ms,
            timings.collect_jobs_after_submit_ms,
            timings.spawn_ms,
            timings.spawn_after_submit_ms,
            timings.player_motion_ms,
            timings.ecs_post_ms,
            timings.lifecycle_plan_ms,
            timings.unload_ms,
            timings.submit_jobs_ms,
            timings.environment_ms,
            timings.selection_ms,
            timings.click_log_ms,
            timings.command_drain_ms
        );
        println!(
            "[perf] update workload: frame={} lifecycle interest={} retain={} unload={} requests={} [{}] results processed={} load={} gen={} mesh_upload={} mesh_empty={} minimap={} region={} ignored={} failed={} triangles={} upload_ms={:.2} jobs pending={} [{}] running={} [{}] completed={} [{}] intermediate={} available_workers={}/{} shutdown={} minimap_cache cached={} pending={} dirty={}",
            self.timing.frame_index,
            lifecycle.interest,
            lifecycle.retain,
            lifecycle.unload,
            lifecycle.requests.total(),
            format_job_counts(lifecycle.requests),
            results.processed,
            results.disk_loaded,
            results.generated,
            results.mesh_uploaded,
            results.mesh_empty,
            results.minimap,
            results.region_class_resolved,
            results.ignored,
            results.failed,
            results.mesh_triangles,
            results.mesh_upload_ms,
            jobs.pending_requests,
            format_job_counts(jobs.pending_by_kind),
            jobs.running_requests,
            format_job_counts(jobs.running_by_kind),
            jobs.completed_results,
            format_job_counts(jobs.completed_by_kind),
            jobs.intermediate_results,
            jobs.available_workers,
            jobs.worker_count,
            jobs.shutdown_requested,
            minimap.cached_columns,
            minimap.pending_columns,
            minimap.dirty_columns
        );
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
        let trigger = if input.left_just_pressed {
            "left"
        } else {
            "right"
        };
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

#[derive(Debug, Default)]
struct AppUpdateTimings {
    shortcuts_ms: f64,
    fixed_ms: f64,
    bridge_input_ms: f64,
    ecs_pre_ms: f64,
    ecs_update_ms: f64,
    collect_jobs_ms: f64,
    spawn_ms: f64,
    player_motion_ms: f64,
    ecs_post_ms: f64,
    lifecycle_plan_ms: f64,
    unload_ms: f64,
    submit_jobs_ms: f64,
    collect_jobs_after_submit_ms: f64,
    spawn_after_submit_ms: f64,
    environment_ms: f64,
    selection_ms: f64,
    click_log_ms: f64,
    command_drain_ms: f64,
}

#[derive(Debug, Default, Clone, Copy)]
struct JobResultApplyStats {
    processed: usize,
    create_progress: usize,
    world_created: usize,
    disk_loaded: usize,
    generated: usize,
    mesh_uploaded: usize,
    mesh_empty: usize,
    minimap: usize,
    region_class_resolved: usize,
    ignored: usize,
    failed: usize,
    mesh_triangles: usize,
    mesh_upload_ms: f64,
}

impl JobResultApplyStats {
    fn add(&mut self, other: Self) {
        self.processed += other.processed;
        self.create_progress += other.create_progress;
        self.world_created += other.world_created;
        self.disk_loaded += other.disk_loaded;
        self.generated += other.generated;
        self.mesh_uploaded += other.mesh_uploaded;
        self.mesh_empty += other.mesh_empty;
        self.minimap += other.minimap;
        self.region_class_resolved += other.region_class_resolved;
        self.ignored += other.ignored;
        self.failed += other.failed;
        self.mesh_triangles += other.mesh_triangles;
        self.mesh_upload_ms += other.mesh_upload_ms;
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct ChunkLifecycleRequestStats {
    interest: usize,
    retain: usize,
    unload: usize,
    requests: JobRequestCounts,
}

impl ChunkLifecycleRequestStats {
    fn from_lifecycle(lifecycle: &crate::ecs::ChunkLifecyclePlan) -> Self {
        let mut requests = JobRequestCounts::default();
        for request in &lifecycle.job_requests {
            requests.add_request(request);
        }

        Self {
            interest: lifecycle.interest.len(),
            retain: lifecycle.retain.len(),
            unload: lifecycle.unload_coords.len(),
            requests,
        }
    }
}

fn format_job_counts(counts: JobRequestCounts) -> String {
    format!(
        "create_world={} load={} generate={} mesh={} minimap={} region={}",
        counts.create_world,
        counts.load_chunk,
        counts.generate_chunk,
        counts.build_chunk_mesh,
        counts.build_minimap_chunk_column,
        counts.resolve_region_class_area
    )
}

fn trace_frame_perf_logs_enabled() -> bool {
    std::env::var_os("NEW_WORLD_TRACE_FRAME_LOGS").is_some()
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
