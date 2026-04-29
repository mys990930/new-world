use std::collections::VecDeque;

use super::JobRequestCounts;
use super::config::JobConfig;
use super::queue::{JobEnqueueOutcome, JobQueue, JobSubmitError};
use super::request::JobRequest;
use super::result::{JobError, JobResult};
use super::worker::{AssignedJob, WorkerContext, WorkerReport};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JobSystemSnapshot {
    pub worker_count: usize,
    pub available_workers: usize,
    pub pending_requests: usize,
    pub running_requests: usize,
    pub completed_results: usize,
    pub intermediate_results: usize,
    pub pending_by_kind: JobRequestCounts,
    pub running_by_kind: JobRequestCounts,
    pub completed_by_kind: JobRequestCounts,
    pub shutdown_requested: bool,
}

pub struct JobSystem {
    config: JobConfig,
    queue: JobQueue,
    workers: WorkerContext,
    intermediate_results: VecDeque<JobResult>,
    available_workers: VecDeque<usize>,
    shutdown_requested: bool,
}

impl JobSystem {
    pub fn new(config: JobConfig) -> Self {
        let worker_count = config.effective_worker_count();
        println!(
            "[jobs] starting job system: workers={} max_pending={:?}",
            worker_count, config.max_pending_requests
        );
        let workers = WorkerContext::new(worker_count);

        Self {
            config,
            queue: JobQueue::new(&config),
            workers,
            intermediate_results: VecDeque::new(),
            available_workers: (0..worker_count).collect(),
            shutdown_requested: false,
        }
    }

    pub fn config(&self) -> &JobConfig {
        &self.config
    }

    pub fn diagnostic_snapshot(&self) -> JobSystemSnapshot {
        let queue = self.queue.diagnostic_snapshot();
        JobSystemSnapshot {
            worker_count: self.config.effective_worker_count(),
            available_workers: self.available_workers.len(),
            pending_requests: queue.pending,
            running_requests: queue.running,
            completed_results: queue.completed,
            intermediate_results: self.intermediate_results.len(),
            pending_by_kind: queue.pending_by_kind,
            running_by_kind: queue.running_by_kind,
            completed_by_kind: queue.completed_by_kind,
            shutdown_requested: self.shutdown_requested,
        }
    }

    pub fn submit(&mut self, request: JobRequest) -> Result<JobEnqueueOutcome, JobSubmitError> {
        self.collect_completed();
        let outcome = self.queue.enqueue(request)?;
        self.dispatch_pending();
        Ok(outcome)
    }

    pub fn submit_all(
        &mut self,
        requests: impl IntoIterator<Item = JobRequest>,
    ) -> Result<(), JobSubmitError> {
        for request in requests {
            let _ = self.submit(request)?;
        }
        Ok(())
    }

    pub fn drain_completed(&mut self) -> Vec<JobResult> {
        self.drain_completed_limit(usize::MAX)
    }

    pub fn drain_completed_limit(&mut self, max_results: usize) -> Vec<JobResult> {
        self.collect_completed();
        self.dispatch_pending();
        let mut drained = Vec::new();
        while drained.len() < max_results {
            let Some(result) = self.intermediate_results.pop_front() else {
                break;
            };
            drained.push(result);
        }
        if drained.len() < max_results {
            drained.extend(
                self.queue
                    .drain_completed_limit(max_results.saturating_sub(drained.len())),
            );
        }
        drained
    }

    pub fn shutdown(&mut self) {
        if self.shutdown_requested {
            return;
        }

        self.shutdown_requested = true;
        println!("[jobs] shutdown requested");
        self.queue.begin_shutdown();
        self.workers.shutdown();
        self.collect_completed();
    }

    fn collect_completed(&mut self) {
        while let Some(report) = self.workers.try_recv() {
            match report {
                WorkerReport::Progress { result } => {
                    self.intermediate_results.push_back(result);
                }
                WorkerReport::Finished {
                    worker_id,
                    sequence,
                    key,
                    result,
                } => {
                    if self.queue.finish_running(sequence, key, result) {
                        self.available_workers.push_back(worker_id);
                    }
                }
            }
        }
    }

    fn dispatch_pending(&mut self) {
        if self.shutdown_requested {
            return;
        }

        while let Some(worker_id) = self.available_workers.pop_front() {
            let Some(queued) = self.queue.take_next_pending() else {
                self.available_workers.push_front(worker_id);
                break;
            };

            let request = queued.request.clone();
            let sequence = queued.sequence;
            let key = request.coalesce_key();
            let assigned = AssignedJob {
                sequence,
                key,
                request,
            };

            if self.workers.send(worker_id, assigned).is_ok() {
                self.queue.mark_running(queued);
            } else {
                self.queue.push_immediate_failure(
                    sequence,
                    queued.request,
                    JobError::WorkerDisconnected { worker_id },
                );
            }
        }
    }
}

impl Drop for JobSystem {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::world::{BlockRegistry, ChunkCoord, LocalBlockCoord, NeighborChunks, WorldMeta};

    fn test_registry() -> Arc<BlockRegistry> {
        Arc::new(BlockRegistry::load_default().expect("default registry should load"))
    }

    #[test]
    #[ignore = "slow job runtime smoke test that runs generation"]
    fn generate_then_mesh_jobs_produce_chunk_outputs() {
        let mut jobs = JobSystem::new(JobConfig::default());
        let coord = ChunkCoord(0, -8, 0);
        let registry = test_registry();
        let stone = registry
            .block_id("stone")
            .expect("stone block should exist");

        assert_eq!(
            jobs.submit(JobRequest::GenerateChunk {
                coord,
                meta: WorldMeta::new(7),
                registry: registry.clone(),
            })
            .unwrap(),
            JobEnqueueOutcome::Enqueued
        );

        let generated = wait_for_single_result(&mut jobs);
        let chunk = match generated {
            JobResult::ChunkGenerated {
                coord: found,
                chunk,
            } => {
                assert_eq!(found, coord);
                assert_eq!(
                    chunk.get_block(LocalBlockCoord::new(3, 0, 5).unwrap()),
                    Some(stone)
                );
                assert_eq!(
                    chunk.get_block(LocalBlockCoord::new(3, 31, 5).unwrap()),
                    Some(stone)
                );
                chunk
            }
            other => panic!("expected generated chunk result, got {other:?}"),
        };

        assert_eq!(
            jobs.submit(JobRequest::BuildChunkMesh {
                center: chunk.snapshot(),
                neighbors: NeighborChunks::default(),
                registry,
            })
            .unwrap(),
            JobEnqueueOutcome::Enqueued
        );

        let meshed = wait_for_single_result(&mut jobs);
        match meshed {
            JobResult::ChunkMeshBuilt { coord: found, mesh } => {
                assert_eq!(found, coord);
                assert!(!mesh.is_empty());
                assert!(mesh.triangle_count() > 0);
            }
            other => panic!("expected meshed chunk result, got {other:?}"),
        }

        jobs.shutdown();
    }

    #[test]
    #[ignore = "slow job runtime smoke test that runs generation"]
    fn duplicate_chunk_requests_are_coalesced() {
        let mut jobs = JobSystem::new(JobConfig {
            worker_count: 1,
            max_pending_requests: Some(4),
        });
        let request = JobRequest::GenerateChunk {
            coord: ChunkCoord(0, 0, 0),
            meta: WorldMeta::default(),
            registry: test_registry(),
        };

        let first = jobs.submit(request.clone()).unwrap();
        let second = jobs.submit(request.clone()).unwrap();

        assert_eq!(first, JobEnqueueOutcome::Enqueued);
        assert!(matches!(
            second,
            JobEnqueueOutcome::Coalesced | JobEnqueueOutcome::Enqueued
        ));

        let result = wait_for_single_result(&mut jobs);
        assert_eq!(result.coord(), ChunkCoord(0, 0, 0));
        let remaining = jobs.drain_completed();
        if second == JobEnqueueOutcome::Coalesced {
            assert!(remaining.is_empty());
        } else {
            assert_eq!(remaining.len(), 1);
            assert_eq!(remaining[0].coord(), ChunkCoord(0, 0, 0));
        }

        jobs.shutdown();
    }

    #[test]
    #[ignore = "slow job runtime smoke test that runs generation"]
    fn submit_all_accepts_multiple_requests() {
        let mut jobs = JobSystem::new(JobConfig {
            worker_count: 1,
            max_pending_requests: Some(4),
        });

        jobs.submit_all([
            JobRequest::GenerateChunk {
                coord: ChunkCoord(0, 0, 0),
                meta: WorldMeta::default(),
                registry: test_registry(),
            },
            JobRequest::GenerateChunk {
                coord: ChunkCoord(1, 0, 0),
                meta: WorldMeta::default(),
                registry: test_registry(),
            },
        ])
        .unwrap();

        let first = wait_for_single_result(&mut jobs);
        let second = wait_for_single_result(&mut jobs);
        assert_ne!(first.coord(), second.coord());

        jobs.shutdown();
    }

    #[test]
    #[ignore = "slow job runtime smoke test that runs current generation world creation"]
    fn create_world_job_produces_manifest_result() {
        let mut jobs = JobSystem::new(JobConfig {
            worker_count: 1,
            max_pending_requests: Some(4),
        });
        let root = std::env::temp_dir().join(format!(
            "new-world-create-job-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));

        let request = JobRequest::CreateWorld {
            root: root.clone(),
            config: crate::world::CreateWorldConfig {
                seed: 42,
                center_x: 0,
                center_z: 0,
                radius: 1,
                min_y_chunk: -1,
                max_y_chunk: 1,
            },
            registry: test_registry(),
        };

        assert_eq!(jobs.submit(request).unwrap(), JobEnqueueOutcome::Enqueued);

        let mut final_result = None;
        let mut saw_progress = false;
        for _ in 0..3_000 {
            for result in jobs.drain_completed() {
                match result {
                    JobResult::CreateWorldProgress {
                        root: found,
                        completed_chunks,
                        total_chunks,
                    } => {
                        assert_eq!(found, root);
                        assert!(total_chunks > 0);
                        assert!(completed_chunks <= total_chunks);
                        saw_progress = true;
                    }
                    JobResult::WorldCreated { .. } => {
                        final_result = Some(result);
                    }
                    other => panic!("expected create-world progress or result, got {other:?}"),
                }
            }

            if final_result.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(saw_progress);
        match final_result.expect("timed out waiting for created world result") {
            JobResult::WorldCreated {
                root: found,
                manifest,
            } => {
                assert_eq!(found, root);
                assert_eq!(manifest.seed, 42);
                assert!(found.exists());
                assert!(found.join("manifest.toml").exists());
            }
            other => panic!("expected created world result, got {other:?}"),
        }

        jobs.shutdown();
        let _ = fs::remove_dir_all(root);
    }

    fn wait_for_single_result(jobs: &mut JobSystem) -> JobResult {
        for _ in 0..100 {
            let mut results = jobs.drain_completed();
            if !results.is_empty() {
                assert_eq!(results.len(), 1);
                return results.remove(0);
            }
            thread::sleep(Duration::from_millis(10));
        }

        panic!("timed out waiting for jobs result");
    }
}
