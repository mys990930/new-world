use std::time::{Duration, Instant};

use super::{GameApp, state::PendingChunkMeshCommit};
use crate::ecs::{InventoryItem, ManipulationMode, PlayerCommand};
use crate::jobs::{JobRequest, JobRequestCounts, JobResult};
use crate::renderer::RenderFrameInput;
use crate::world::{BlockId, ChunkCoord, WorldBlockCoord, WorldEdit};

const MAX_GAMEPLAY_JOB_RESULTS_PER_FRAME: usize = 4;
const MAX_CHUNK_MESH_UPLOADS_PER_FRAME: usize = 1;
const MAX_EMPTY_CHUNK_MESH_REMOVES_PER_FRAME: usize = 8;
const CHUNK_LIFECYCLE_LOG_INTERVAL_FRAMES: u64 = 30;
const SLOW_RENDER_UPLOAD_LOG_THRESHOLD: Duration = Duration::from_millis(8);
const DEFAULT_UPDATE_HITCH_LOG_THRESHOLD_MS: f64 = 33.0;

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
        self.ecs.tick_tool_interaction_state(&self.world);

        let stage_start = Instant::now();
        let lifecycle = self
            .ecs
            .plan_chunk_lifecycle(&self.world, self.created_world.as_ref());
        timings.lifecycle_plan_ms = duration_ms(stage_start.elapsed());
        lifecycle_stats = ChunkLifecycleRequestStats::from_lifecycle(&lifecycle);
        self.log_chunk_lifecycle_plan(&lifecycle);
        timings.unload_ms = 0.0;
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
        let stage_start = Instant::now();
        result_stats.add(self.apply_pending_chunk_mesh_commits());
        timings.chunk_mesh_commit_ms = duration_ms(stage_start.elapsed());
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
        self.apply_player_commands();
        timings.command_drain_ms = duration_ms(stage_start.elapsed());
        self.log_update_perf_if_needed(
            update_start.elapsed(),
            &timings,
            &result_stats,
            &lifecycle_stats,
        );
        self.log_update_hitch_if_needed(
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
        let occlusion_blocks = render_frame.occlusion_blocks.len();
        let cube_instances = render_frame.cube_instances.len();
        let ui_sprites = render_frame.ui_sprites.len();
        let draw_scene = render_frame.draw_scene;

        let draw_start = Instant::now();
        if let Err(error) = self.renderer.render(RenderFrameInput {
            camera: &render_frame.camera,
            draw_scene: render_frame.draw_scene,
            visible_chunks: &render_frame.visible_chunks,
            occlusion_blocks: &render_frame.occlusion_blocks,
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
                "[perf] render frame: frame={} total_ms={:.2} bridge_ms={:.2} draw_ms={:.2} draw_scene={} visible_chunks={} occlusion_blocks={} cube_instances={} ui_sprites={} surface={}x{}",
                self.timing.frame_index,
                duration_ms(total),
                bridge_ms,
                draw_ms,
                draw_scene,
                visible_chunks,
                occlusion_blocks,
                cube_instances,
                ui_sprites,
                width,
                height
            );
        }
        self.log_render_hitch_if_needed(
            total,
            bridge_ms,
            draw_ms,
            draw_scene,
            visible_chunks,
            cube_instances,
            ui_sprites,
            width,
            height,
        );
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
            let ecs_apply_start = Instant::now();
            self.ecs.apply_job_result(&result);
            stats.ecs_apply_result_ms += duration_ms(ecs_apply_start.elapsed());

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
                        let insert_start = Instant::now();
                        self.world.insert_chunk(coord, chunk);
                        stats.world_insert_ms += duration_ms(insert_start.elapsed());
                        println!(
                            "[app] chunk loaded: frame={} pos=({}, {}, {}) source=disk",
                            self.timing.frame_index, coord.0, coord.1, coord.2
                        );
                        let minimap_queue_start = Instant::now();
                        self.refresh_minimap_chunk_column_after_world_change(
                            crate::world::TopdownChunkColumnCoord {
                                chunk_x: coord.0,
                                chunk_z: coord.2,
                            },
                        );
                        stats.minimap_queue_ms += duration_ms(minimap_queue_start.elapsed());
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
                        let insert_start = Instant::now();
                        self.world.insert_chunk(coord, chunk);
                        stats.world_insert_ms += duration_ms(insert_start.elapsed());
                        println!(
                            "[app] chunk loaded: frame={} pos=({}, {}, {}) source=generated",
                            self.timing.frame_index, coord.0, coord.1, coord.2
                        );
                        let minimap_queue_start = Instant::now();
                        self.refresh_minimap_chunk_column_after_world_change(
                            crate::world::TopdownChunkColumnCoord {
                                chunk_x: coord.0,
                                chunk_z: coord.2,
                            },
                        );
                        stats.minimap_queue_ms += duration_ms(minimap_queue_start.elapsed());
                    } else {
                        stats.ignored += 1;
                        println!(
                            "[app] chunk generation ignored: pos=({}, {}, {}) reason=outside-retain",
                            coord.0, coord.1, coord.2
                        );
                    }
                }
                JobResult::ChunkUnloaded { coord } => {
                    stats.unloaded += 1;
                    let unload_start = Instant::now();
                    self.apply_chunk_unloads(&[coord]);
                    stats.unload_apply_ms += duration_ms(unload_start.elapsed());
                }
                JobResult::ChunkMeshBuilt { coord, mesh } => {
                    if self.ecs.retains_chunk(coord) && self.world.has_chunk(coord) {
                        let triangles = mesh.triangle_count();
                        stats.mesh_triangles += triangles;
                        if mesh.is_empty() {
                            stats.mesh_queued += 1;
                            self.pending_chunk_mesh_commits
                                .push_back(PendingChunkMeshCommit::Remove { coord });
                        } else {
                            let bytes = estimate_world_mesh_upload_bytes(&mesh);
                            stats.mesh_queued += 1;
                            self.pending_chunk_mesh_commits.push_back(
                                PendingChunkMeshCommit::Upsert {
                                    coord,
                                    mesh,
                                    triangles,
                                    bytes,
                                },
                            );
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
                    let minimap_apply_start = Instant::now();
                    self.handle_minimap_chunk_column_built(coord, patch);
                    stats.minimap_apply_ms += duration_ms(minimap_apply_start.elapsed());
                }
                JobResult::RegionClassResolved { area: _, classes } => {
                    stats.region_class_resolved += 1;
                    let region_apply_start = Instant::now();
                    self.world.cache_region_class_map(&classes);
                    self.sync_renderer_environment_from_world();
                    stats.region_apply_ms += duration_ms(region_apply_start.elapsed());
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

    fn apply_pending_chunk_mesh_commits(&mut self) -> JobResultApplyStats {
        let mut stats = JobResultApplyStats::default();
        stats.mesh_commit_pending_before = self.pending_chunk_mesh_commits.len();
        let byte_budget = self.renderer.config().upload_budget_bytes_per_frame;
        let mut uploaded = 0_usize;
        let mut removed = 0_usize;
        let mut uploaded_bytes = 0_usize;

        while let Some(commit) = self.pending_chunk_mesh_commits.pop_front() {
            stats.mesh_commit_visited += 1;
            match commit {
                PendingChunkMeshCommit::Upsert {
                    coord,
                    mesh,
                    triangles,
                    bytes,
                } => {
                    if !self.should_commit_chunk_mesh(coord) {
                        stats.ignored += 1;
                        continue;
                    }
                    let budget_exceeded = byte_budget > 0
                        && uploaded > 0
                        && uploaded_bytes.saturating_add(bytes) > byte_budget;
                    if uploaded >= MAX_CHUNK_MESH_UPLOADS_PER_FRAME || budget_exceeded {
                        stats.mesh_commit_deferred += 1;
                        self.pending_chunk_mesh_commits.push_front(
                            PendingChunkMeshCommit::Upsert {
                                coord,
                                mesh,
                                triangles,
                                bytes,
                            },
                        );
                        break;
                    }

                    let upload_start = Instant::now();
                    let upload_result = self
                        .renderer
                        .apply_upload(self.bridge_world_mesh_to_render_upload(coord, mesh));
                    let upload_elapsed = upload_start.elapsed();
                    let upload_ms = duration_ms(upload_elapsed);
                    stats.mesh_upload_ms += upload_ms;
                    if let Err(error) = upload_result {
                        stats.failed += 1;
                        eprintln!("[app] renderer upload failed for {:?}: {:?}", coord, error);
                    } else {
                        uploaded += 1;
                        uploaded_bytes = uploaded_bytes.saturating_add(bytes);
                        stats.mesh_uploaded += 1;
                        stats.mesh_upload_bytes = stats.mesh_upload_bytes.saturating_add(bytes);
                        stats.mesh_triangles += triangles;
                        stats.record_mesh_upload_sample(upload_ms, triangles, bytes);
                        if upload_elapsed >= SLOW_RENDER_UPLOAD_LOG_THRESHOLD {
                            println!(
                                "[perf] slow mesh upload: frame={} pos=({}, {}, {}) triangles={} bytes={} elapsed_ms={:.2}",
                                self.timing.frame_index,
                                coord.0,
                                coord.1,
                                coord.2,
                                triangles,
                                bytes,
                                upload_ms
                            );
                        }
                        println!(
                            "[app] chunk mesh uploaded: frame={} pos=({}, {}, {}) triangles={} bytes={}",
                            self.timing.frame_index, coord.0, coord.1, coord.2, triangles, bytes
                        );
                    }
                }
                PendingChunkMeshCommit::Remove { coord } => {
                    if !self.should_commit_chunk_mesh(coord) {
                        stats.ignored += 1;
                        continue;
                    }
                    if removed >= MAX_EMPTY_CHUNK_MESH_REMOVES_PER_FRAME {
                        stats.mesh_commit_deferred += 1;
                        self.pending_chunk_mesh_commits
                            .push_front(PendingChunkMeshCommit::Remove { coord });
                        break;
                    }
                    self.renderer
                        .remove_chunk_mesh(crate::renderer::ChunkCoord(coord.0, coord.1, coord.2));
                    removed += 1;
                    stats.mesh_empty += 1;
                    println!(
                        "[app] chunk mesh empty: frame={} pos=({}, {}, {})",
                        self.timing.frame_index, coord.0, coord.1, coord.2
                    );
                }
            }
        }

        stats.mesh_commit_pending_after = self.pending_chunk_mesh_commits.len();
        stats
    }

    fn should_commit_chunk_mesh(&self, coord: crate::world::ChunkCoord) -> bool {
        self.ecs.retains_chunk(coord) && self.world.has_chunk(coord)
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
            self.pending_chunk_mesh_commits
                .retain(|commit| commit.coord() != coord);
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

    fn apply_player_commands(&mut self) {
        for command in self.ecs.drain_player_commands() {
            match command {
                PlayerCommand::PrimaryAction => self.try_apply_primary_tool_action(),
                PlayerCommand::PlaceBlock => self.try_place_selected_block(),
                PlayerCommand::RotateCamera { .. }
                | PlayerCommand::RecenterCamera
                | PlayerCommand::ToggleManipulationMode
                | PlayerCommand::ToggleInventory
                | PlayerCommand::CycleQuickslot { .. }
                | PlayerCommand::SelectQuickslot { .. } => {}
            }
        }
    }

    fn try_apply_primary_tool_action(&mut self) {
        let outcome = self.ecs.apply_primary_tool_action(&self.world);
        if outcome.broken_blocks.is_empty() {
            return;
        }

        let mut changed_chunks = std::collections::BTreeSet::new();
        let mut remesh_chunks = std::collections::BTreeSet::new();
        let mut destroyed_blocks = 0_usize;
        for broken in outcome.broken_blocks {
            let result = self.world.apply_edit(WorldEdit::SetBlock {
                pos: broken.pos,
                block: BlockId::AIR,
            });
            if !result.applied {
                if let Some(error) = result.error {
                    eprintln!(
                        "[app] block break failed: pos=({}, {}, {}) block={} error={:?}",
                        broken.pos.0,
                        broken.pos.1,
                        broken.pos.2,
                        broken.block.raw(),
                        error
                    );
                }
                continue;
            }

            let drop_block = result.previous_block.unwrap_or(broken.block);
            if !drop_block.is_air() {
                self.ecs.spawn_block_drop(broken.pos, drop_block);
            }
            changed_chunks.extend(result.changed_chunks);
            remesh_chunks.extend(result.remesh_chunks);
            destroyed_blocks += 1;
        }

        if destroyed_blocks == 0 {
            return;
        }

        self.ecs
            .mark_chunks_for_remesh(remesh_chunks.iter().copied());
        self.refresh_minimap_after_changed_chunks(
            &changed_chunks.iter().copied().collect::<Vec<_>>(),
        );
        println!(
            "[app] blocks broken: count={} remesh_chunks={}",
            destroyed_blocks,
            remesh_chunks.len()
        );
    }

    fn try_place_selected_block(&mut self) {
        let Some(inventory) = self.ecs.local_player_inventory() else {
            return;
        };
        if !matches!(inventory.manipulation_mode, ManipulationMode::Build) {
            return;
        }
        let Some(slot) = inventory.selected_block() else {
            return;
        };
        let InventoryItem::Block(block) = slot.item else {
            return;
        };
        if block.is_air() || slot.count == 0 {
            return;
        }

        let selection = self.ecs.selection_state();
        let Some(pos) = selection.build_preview_block else {
            return;
        };
        if self
            .world
            .get_block(pos)
            .is_some_and(|existing| !existing.is_air())
        {
            return;
        }
        if self.placement_intersects_local_player(pos) {
            return;
        }

        let result = self.world.apply_edit(WorldEdit::SetBlock { pos, block });
        if !result.applied {
            if let Some(error) = result.error {
                eprintln!(
                    "[app] block placement failed: pos=({}, {}, {}) block={} error={:?}",
                    pos.0,
                    pos.1,
                    pos.2,
                    block.raw(),
                    error
                );
            }
            return;
        }

        let _ = self.ecs.consume_selected_build_block();
        self.ecs
            .mark_chunks_for_remesh(result.remesh_chunks.iter().copied());
        self.refresh_minimap_after_changed_chunks(&result.changed_chunks);
        println!(
            "[app] block placed: pos=({}, {}, {}) block={} remesh_chunks={}",
            pos.0,
            pos.1,
            pos.2,
            block.raw(),
            result.remesh_chunks.len()
        );
    }

    fn refresh_minimap_after_changed_chunks(&mut self, chunks: &[ChunkCoord]) {
        let mut columns = std::collections::BTreeSet::new();
        for coord in chunks {
            columns.insert(crate::world::TopdownChunkColumnCoord {
                chunk_x: coord.0,
                chunk_z: coord.2,
            });
        }
        for column in columns {
            self.refresh_minimap_chunk_column_after_world_change(column);
        }
    }

    fn placement_intersects_local_player(&self, block: WorldBlockCoord) -> bool {
        let Some(transform) = self.ecs.local_player_transform() else {
            return false;
        };
        let Some(body) = self.ecs.local_player_body() else {
            return false;
        };

        let block_min = [block.0 as f32, block.1 as f32, block.2 as f32];
        let block_max = [
            block.0 as f32 + 1.0,
            block.1 as f32 + 1.0,
            block.2 as f32 + 1.0,
        ];
        let player_min = [
            transform.translation[0] - body.half_extents[0],
            transform.translation[1] - body.half_extents[1],
            transform.translation[2] - body.half_extents[2],
        ];
        let player_max = [
            transform.translation[0] + body.half_extents[0],
            transform.translation[1] + body.half_extents[1],
            transform.translation[2] + body.half_extents[2],
        ];

        block_min[0] < player_max[0]
            && block_max[0] > player_min[0]
            && block_min[1] < player_max[1]
            && block_max[1] > player_min[1]
            && block_min[2] < player_max[2]
            && block_max[2] > player_min[2]
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
        let mut unload = 0_usize;
        let mut mesh = 0_usize;
        let mut minimap = 0_usize;
        let mut create_world = 0_usize;
        for request in &lifecycle.job_requests {
            match request {
                JobRequest::CreateWorld { .. } => create_world += 1,
                JobRequest::LoadChunk { .. } => load += 1,
                JobRequest::GenerateChunk { .. } => generate += 1,
                JobRequest::UnloadChunk { .. } => unload += 1,
                JobRequest::BuildChunkMesh { .. } => mesh += 1,
                JobRequest::BuildMinimapChunkColumn { .. } => minimap += 1,
                JobRequest::ResolveRegionClassArea { .. } => {}
            }
        }

        println!(
            "[app] chunk lifecycle plan: interest={} retain={} requests={} load={} generate={} unload={} mesh={} minimap={} create_world={} unload_coords={} spawn_pending={}",
            lifecycle.interest.len(),
            lifecycle.retain.len(),
            lifecycle.job_requests.len(),
            load,
            generate,
            unload,
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
            "[perf] update frame: frame={} total_ms={:.2} frame_dt_ms={:.2} shortcuts_ms={:.2} fixed_ms={:.2} bridge_input_ms={:.2} ecs_pre_ms={:.2} ecs_update_ms={:.2} collect_jobs_ms={:.2}+{:.2} spawn_ms={:.2}+{:.2} motion_ms={:.2} ecs_post_ms={:.2} lifecycle_ms={:.2} unload_ms={:.2} submit_ms={:.2} mesh_commit_ms={:.2} env_ms={:.2} selection_ms={:.2} click_ms={:.2} command_ms={:.2}",
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
            timings.chunk_mesh_commit_ms,
            timings.environment_ms,
            timings.selection_ms,
            timings.click_log_ms,
            timings.command_drain_ms
        );
        println!(
            "[perf] update workload: frame={} lifecycle interest={} retain={} unload={} requests={} [{}] results processed={} load={} gen={} unload={} mesh_queued={} mesh_upload={} mesh_empty={} mesh_pending={} minimap={} region={} ignored={} failed={} triangles={} upload_ms={:.2} unload_ms={:.2} jobs pending={} [{}] running={} [{}] completed={} [{}] intermediate={} available_workers={}/{} shutdown={} minimap_cache cached={} pending={} dirty={}",
            self.timing.frame_index,
            lifecycle.interest,
            lifecycle.retain,
            lifecycle.unload,
            lifecycle.requests.total(),
            format_job_counts(lifecycle.requests),
            results.processed,
            results.disk_loaded,
            results.generated,
            results.unloaded,
            results.mesh_queued,
            results.mesh_uploaded,
            results.mesh_empty,
            self.pending_chunk_mesh_commits.len(),
            results.minimap,
            results.region_class_resolved,
            results.ignored,
            results.failed,
            results.mesh_triangles,
            results.mesh_upload_ms,
            results.unload_apply_ms,
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

    fn log_update_hitch_if_needed(
        &self,
        total: Duration,
        timings: &AppUpdateTimings,
        results: &JobResultApplyStats,
        lifecycle: &ChunkLifecycleRequestStats,
    ) {
        let total_ms = duration_ms(total);
        let frame_dt_ms = duration_ms(self.timing.frame_dt);
        let threshold_ms = update_hitch_log_threshold_ms();
        if total_ms < threshold_ms && frame_dt_ms < threshold_ms * 2.0 {
            return;
        }

        let jobs = self.jobs.diagnostic_snapshot();
        let minimap = self.minimap.diagnostic_snapshot();
        let (slowest_stage, slowest_ms) = slowest_update_stage(timings);
        println!(
            "[hitch] update frame={} total_ms={:.2} frame_dt_ms={:.2} threshold_ms={:.2} slowest_stage={} slowest_ms={:.2}",
            self.timing.frame_index, total_ms, frame_dt_ms, threshold_ms, slowest_stage, slowest_ms
        );
        println!(
            "[hitch] update stages: shortcuts={:.2} fixed={:.2} input={:.2} ecs_pre={:.2} ecs_update={:.2} collect_jobs={:.2}+{:.2} spawn={:.2}+{:.2} motion={:.2} ecs_post={:.2} lifecycle={:.2} submit={:.2} mesh_commit={:.2} env={:.2} selection={:.2} click={:.2} command={:.2}",
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
            timings.submit_jobs_ms,
            timings.chunk_mesh_commit_ms,
            timings.environment_ms,
            timings.selection_ms,
            timings.click_log_ms,
            timings.command_drain_ms
        );
        println!(
            "[hitch] results: processed={} load={} gen={} unload={} mesh_queued={} mesh_upload={} mesh_empty={} mesh_pending_before={} mesh_pending_after={} mesh_visited={} mesh_deferred={} mesh_bytes={} mesh_upload_ms={:.2} mesh_upload_max_ms={:.2} mesh_upload_max_triangles={} mesh_upload_max_bytes={} ecs_apply_ms={:.2} world_insert_ms={:.2} minimap_queue_ms={:.2} minimap_apply_ms={:.2} region_apply_ms={:.2} unload_apply_ms={:.2} minimap={} region={} ignored={} failed={}",
            results.processed,
            results.disk_loaded,
            results.generated,
            results.unloaded,
            results.mesh_queued,
            results.mesh_uploaded,
            results.mesh_empty,
            results.mesh_commit_pending_before,
            results.mesh_commit_pending_after,
            results.mesh_commit_visited,
            results.mesh_commit_deferred,
            results.mesh_upload_bytes,
            results.mesh_upload_ms,
            results.mesh_upload_max_ms,
            results.mesh_upload_max_triangles,
            results.mesh_upload_max_bytes,
            results.ecs_apply_result_ms,
            results.world_insert_ms,
            results.minimap_queue_ms,
            results.minimap_apply_ms,
            results.region_apply_ms,
            results.unload_apply_ms,
            results.minimap,
            results.region_class_resolved,
            results.ignored,
            results.failed
        );
        println!(
            "[hitch] pressure: lifecycle interest={} retain={} unload={} requests={} [{}] jobs pending={} [{}] running={} [{}] completed={} [{}] intermediate={} workers={}/{} shutdown={} minimap_cache cached={} pending={} dirty={}",
            lifecycle.interest,
            lifecycle.retain,
            lifecycle.unload,
            lifecycle.requests.total(),
            format_job_counts(lifecycle.requests),
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

    fn log_render_hitch_if_needed(
        &self,
        total: Duration,
        bridge_ms: f64,
        draw_ms: f64,
        draw_scene: bool,
        visible_chunks: usize,
        cube_instances: usize,
        ui_sprites: usize,
        width: u32,
        height: u32,
    ) {
        let total_ms = duration_ms(total);
        let threshold_ms = update_hitch_log_threshold_ms();
        if total_ms < threshold_ms {
            return;
        }

        println!(
            "[hitch] render frame={} total_ms={:.2} threshold_ms={:.2} bridge_ms={:.2} draw_ms={:.2} draw_scene={} visible_chunks={} cube_instances={} ui_sprites={} surface={}x{}",
            self.timing.frame_index,
            total_ms,
            threshold_ms,
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
    chunk_mesh_commit_ms: f64,
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
    unloaded: usize,
    mesh_queued: usize,
    mesh_uploaded: usize,
    mesh_empty: usize,
    mesh_commit_pending_before: usize,
    mesh_commit_pending_after: usize,
    mesh_commit_visited: usize,
    mesh_commit_deferred: usize,
    mesh_upload_bytes: usize,
    mesh_upload_max_triangles: usize,
    mesh_upload_max_bytes: usize,
    minimap: usize,
    region_class_resolved: usize,
    ignored: usize,
    failed: usize,
    mesh_triangles: usize,
    mesh_upload_ms: f64,
    mesh_upload_max_ms: f64,
    ecs_apply_result_ms: f64,
    world_insert_ms: f64,
    minimap_queue_ms: f64,
    minimap_apply_ms: f64,
    region_apply_ms: f64,
    unload_apply_ms: f64,
}

impl JobResultApplyStats {
    fn add(&mut self, other: Self) {
        self.processed += other.processed;
        self.create_progress += other.create_progress;
        self.world_created += other.world_created;
        self.disk_loaded += other.disk_loaded;
        self.generated += other.generated;
        self.unloaded += other.unloaded;
        self.mesh_queued += other.mesh_queued;
        self.mesh_uploaded += other.mesh_uploaded;
        self.mesh_empty += other.mesh_empty;
        self.mesh_commit_pending_before = self
            .mesh_commit_pending_before
            .max(other.mesh_commit_pending_before);
        self.mesh_commit_pending_after = self
            .mesh_commit_pending_after
            .max(other.mesh_commit_pending_after);
        self.mesh_commit_visited += other.mesh_commit_visited;
        self.mesh_commit_deferred += other.mesh_commit_deferred;
        self.mesh_upload_bytes = self
            .mesh_upload_bytes
            .saturating_add(other.mesh_upload_bytes);
        if other.mesh_upload_max_ms > self.mesh_upload_max_ms {
            self.mesh_upload_max_ms = other.mesh_upload_max_ms;
            self.mesh_upload_max_triangles = other.mesh_upload_max_triangles;
            self.mesh_upload_max_bytes = other.mesh_upload_max_bytes;
        }
        self.minimap += other.minimap;
        self.region_class_resolved += other.region_class_resolved;
        self.ignored += other.ignored;
        self.failed += other.failed;
        self.mesh_triangles += other.mesh_triangles;
        self.mesh_upload_ms += other.mesh_upload_ms;
        self.ecs_apply_result_ms += other.ecs_apply_result_ms;
        self.world_insert_ms += other.world_insert_ms;
        self.minimap_queue_ms += other.minimap_queue_ms;
        self.minimap_apply_ms += other.minimap_apply_ms;
        self.region_apply_ms += other.region_apply_ms;
        self.unload_apply_ms += other.unload_apply_ms;
    }

    fn record_mesh_upload_sample(&mut self, upload_ms: f64, triangles: usize, bytes: usize) {
        if upload_ms > self.mesh_upload_max_ms {
            self.mesh_upload_max_ms = upload_ms;
            self.mesh_upload_max_triangles = triangles;
            self.mesh_upload_max_bytes = bytes;
        }
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
        "create_world={} load={} generate={} unload={} mesh={} minimap={} region={}",
        counts.create_world,
        counts.load_chunk,
        counts.generate_chunk,
        counts.unload_chunk,
        counts.build_chunk_mesh,
        counts.build_minimap_chunk_column,
        counts.resolve_region_class_area
    )
}

fn estimate_world_mesh_upload_bytes(mesh: &crate::world::CpuMesh) -> usize {
    std::mem::size_of_val(mesh.vertices.as_slice())
        .saturating_add(std::mem::size_of_val(mesh.indices.as_slice()))
}

fn slowest_update_stage(timings: &AppUpdateTimings) -> (&'static str, f64) {
    let stages = [
        ("shortcuts", timings.shortcuts_ms),
        ("fixed", timings.fixed_ms),
        ("bridge_input", timings.bridge_input_ms),
        ("ecs_pre", timings.ecs_pre_ms),
        ("ecs_update", timings.ecs_update_ms),
        ("collect_jobs_before_submit", timings.collect_jobs_ms),
        (
            "collect_jobs_after_submit",
            timings.collect_jobs_after_submit_ms,
        ),
        ("spawn_before_submit", timings.spawn_ms),
        ("spawn_after_submit", timings.spawn_after_submit_ms),
        ("player_motion", timings.player_motion_ms),
        ("ecs_post", timings.ecs_post_ms),
        ("lifecycle_plan", timings.lifecycle_plan_ms),
        ("submit_jobs", timings.submit_jobs_ms),
        ("mesh_commit", timings.chunk_mesh_commit_ms),
        ("environment", timings.environment_ms),
        ("selection", timings.selection_ms),
        ("click_log", timings.click_log_ms),
        ("command_drain", timings.command_drain_ms),
    ];

    stages
        .into_iter()
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .unwrap_or(("none", 0.0))
}

fn update_hitch_log_threshold_ms() -> f64 {
    std::env::var("NEW_WORLD_HITCH_LOG_MS")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(DEFAULT_UPDATE_HITCH_LOG_THRESHOLD_MS)
}

fn trace_frame_perf_logs_enabled() -> bool {
    std::env::var_os("NEW_WORLD_TRACE_FRAME_LOGS").is_some()
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
