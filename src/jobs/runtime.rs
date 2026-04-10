use std::collections::VecDeque;

use super::config::JobConfig;
use super::queue::{JobEnqueueOutcome, JobQueue, JobSubmitError};
use super::request::JobRequest;
use super::result::{JobError, JobResult};
use super::worker::{AssignedJob, WorkerContext};

pub struct JobSystem {
    config: JobConfig,
    queue: JobQueue,
    workers: WorkerContext,
    available_workers: VecDeque<usize>,
    shutdown_requested: bool,
}

impl JobSystem {
    pub fn new(config: JobConfig) -> Self {
        let worker_count = config.effective_worker_count();
        let workers = WorkerContext::new(worker_count);

        Self {
            config,
            queue: JobQueue::new(&config),
            workers,
            available_workers: (0..worker_count).collect(),
            shutdown_requested: false,
        }
    }

    pub fn config(&self) -> &JobConfig {
        &self.config
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
        self.collect_completed();
        self.dispatch_pending();
        self.queue.drain_completed()
    }

    pub fn shutdown(&mut self) {
        if self.shutdown_requested {
            return;
        }

        self.shutdown_requested = true;
        self.queue.begin_shutdown();
        self.workers.shutdown();
        self.collect_completed();
    }

    fn collect_completed(&mut self) {
        while let Some(report) = self.workers.try_recv() {
            if self
                .queue
                .finish_running(report.sequence, report.key, report.result)
            {
                self.available_workers.push_back(report.worker_id);
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
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::world::{BlockRegistry, ChunkCoord, LocalBlockCoord, NeighborChunks, WorldMeta};

    fn test_registry() -> Arc<BlockRegistry> {
        Arc::new(BlockRegistry::load_default().expect("default registry should load"))
    }

    #[test]
    fn generate_then_mesh_jobs_produce_chunk_outputs() {
        let mut jobs = JobSystem::new(JobConfig::default());
        let coord = ChunkCoord(0, -8, 0);
        let registry = test_registry();
        let terrain = registry
            .block_id("terrain_debug")
            .expect("terrain_debug block should exist");

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
            JobResult::ChunkGenerated { coord: found, chunk } => {
                assert_eq!(found, coord);
                assert_eq!(
                    chunk.get_block(LocalBlockCoord::new(3, 0, 5).unwrap()),
                    Some(terrain)
                );
                assert_eq!(
                    chunk.get_block(LocalBlockCoord::new(3, 31, 5).unwrap()),
                    Some(terrain)
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
