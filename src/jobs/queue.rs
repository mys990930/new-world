use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use super::config::JobConfig;
use super::request::{JobCoalesceKey, JobRequest, JobRequestCounts};
use super::result::{JobError, JobResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobEnqueueOutcome {
    Enqueued,
    Coalesced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobSubmitError {
    Shutdown,
    PendingQueueFull { limit: usize },
}

#[derive(Debug, Clone)]
pub(crate) struct QueuedRequest {
    pub sequence: u64,
    pub request: JobRequest,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct JobQueueSnapshot {
    pub pending: usize,
    pub running: usize,
    pub completed: usize,
    pub pending_by_kind: JobRequestCounts,
    pub running_by_kind: JobRequestCounts,
    pub completed_by_kind: JobRequestCounts,
}

#[derive(Debug, Clone)]
struct RunningRequest {
    sequence: u64,
}

pub(crate) struct JobQueue {
    max_pending_requests: Option<usize>,
    next_sequence: u64,
    next_drain_sequence: u64,
    pending: VecDeque<QueuedRequest>,
    pending_keys: HashSet<JobCoalesceKey>,
    running: HashMap<JobCoalesceKey, RunningRequest>,
    completed: BTreeMap<u64, JobResult>,
    completed_keys: HashSet<JobCoalesceKey>,
    shutting_down: bool,
}

impl JobQueue {
    pub fn new(config: &JobConfig) -> Self {
        Self {
            max_pending_requests: config.max_pending_requests,
            next_sequence: 0,
            next_drain_sequence: 0,
            pending: VecDeque::new(),
            pending_keys: HashSet::new(),
            running: HashMap::new(),
            completed: BTreeMap::new(),
            completed_keys: HashSet::new(),
            shutting_down: false,
        }
    }

    pub fn enqueue(&mut self, request: JobRequest) -> Result<JobEnqueueOutcome, JobSubmitError> {
        if self.shutting_down {
            return Err(JobSubmitError::Shutdown);
        }

        let key = request.coalesce_key();
        if self.pending_keys.contains(&key)
            || self.running.contains_key(&key)
            || self.completed_keys.contains(&key)
        {
            return Ok(JobEnqueueOutcome::Coalesced);
        }

        if let Some(limit) = self.max_pending_requests {
            if self.pending.len() >= limit {
                return Err(JobSubmitError::PendingQueueFull { limit });
            }
        }

        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.pending.push_back(QueuedRequest {
            sequence,
            request: request.clone(),
        });
        self.pending_keys.insert(key);
        Ok(JobEnqueueOutcome::Enqueued)
    }

    pub fn take_next_pending(&mut self) -> Option<QueuedRequest> {
        let queued = self.pending.pop_front()?;
        self.pending_keys.remove(&queued.request.coalesce_key());
        Some(queued)
    }

    pub fn mark_running(&mut self, queued: QueuedRequest) {
        let key = queued.request.coalesce_key();
        self.running.insert(
            key,
            RunningRequest {
                sequence: queued.sequence,
            },
        );
    }

    pub fn push_immediate_failure(&mut self, sequence: u64, request: JobRequest, error: JobError) {
        self.completed_keys.insert(request.coalesce_key());
        self.completed
            .insert(sequence, JobResult::JobFailed { request, error });
    }

    pub fn finish_running(
        &mut self,
        sequence: u64,
        key: JobCoalesceKey,
        result: JobResult,
    ) -> bool {
        let Some(running) = self.running.remove(&key) else {
            return false;
        };
        debug_assert_eq!(running.sequence, sequence);
        self.completed_keys.insert(key);
        self.completed.insert(sequence, result);
        true
    }

    pub fn begin_shutdown(&mut self) {
        if self.shutting_down {
            return;
        }

        self.shutting_down = true;
        while let Some(queued) = self.pending.pop_front() {
            let key = queued.request.coalesce_key();
            self.pending_keys.remove(&key);
            self.completed_keys.insert(key);
            self.completed.insert(
                queued.sequence,
                JobResult::JobFailed {
                    request: queued.request,
                    error: JobError::Shutdown,
                },
            );
        }
    }

    pub fn drain_completed_limit(&mut self, max_results: usize) -> Vec<JobResult> {
        let mut drained = Vec::new();
        while drained.len() < max_results {
            let Some(result) = self.completed.remove(&self.next_drain_sequence) else {
                break;
            };
            if let Some(key) = result_coalesce_key(&result) {
                self.completed_keys.remove(&key);
            }
            drained.push(result);
            self.next_drain_sequence = self.next_drain_sequence.saturating_add(1);
        }
        drained
    }

    pub fn diagnostic_snapshot(&self) -> JobQueueSnapshot {
        let mut pending_by_kind = JobRequestCounts::default();
        for queued in &self.pending {
            pending_by_kind.add_request(&queued.request);
        }

        let mut running_by_kind = JobRequestCounts::default();
        for key in self.running.keys() {
            running_by_kind.add_coalesce_key(key);
        }

        let mut completed_by_kind = JobRequestCounts::default();
        for result in self.completed.values() {
            add_result_kind(&mut completed_by_kind, result);
        }

        JobQueueSnapshot {
            pending: self.pending.len(),
            running: self.running.len(),
            completed: self.completed.len(),
            pending_by_kind,
            running_by_kind,
            completed_by_kind,
        }
    }
}

fn result_coalesce_key(result: &JobResult) -> Option<JobCoalesceKey> {
    match result {
        JobResult::CreateWorldProgress { root, .. } | JobResult::WorldCreated { root, .. } => {
            Some(JobCoalesceKey::CreateWorld(root.clone()))
        }
        JobResult::ChunkLoaded { coord, .. } => Some(JobCoalesceKey::LoadChunk(*coord)),
        JobResult::ChunkGenerated { coord, .. } => Some(JobCoalesceKey::GenerateChunk(*coord)),
        JobResult::ChunkMeshBuilt { coord, .. } => Some(JobCoalesceKey::BuildChunkMesh(*coord)),
        JobResult::MinimapChunkColumnBuilt { coord, .. } => {
            Some(JobCoalesceKey::BuildMinimapChunkColumn(*coord))
        }
        JobResult::RegionClassResolved { area, .. } => {
            Some(JobCoalesceKey::ResolveRegionClassArea(*area))
        }
        JobResult::JobFailed { request, .. } => Some(request.coalesce_key()),
    }
}

fn add_result_kind(counts: &mut JobRequestCounts, result: &JobResult) {
    match result {
        JobResult::CreateWorldProgress { .. } | JobResult::WorldCreated { .. } => {
            counts.create_world += 1;
        }
        JobResult::ChunkLoaded { .. } => counts.load_chunk += 1,
        JobResult::ChunkGenerated { .. } => counts.generate_chunk += 1,
        JobResult::ChunkMeshBuilt { .. } => counts.build_chunk_mesh += 1,
        JobResult::MinimapChunkColumnBuilt { .. } => {
            counts.build_minimap_chunk_column += 1;
        }
        JobResult::RegionClassResolved { .. } => counts.resolve_region_class_area += 1,
        JobResult::JobFailed { request, .. } => counts.add_request(request),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::world::{BlockRegistry, ChunkCoord, WorldMeta};

    fn test_registry() -> Arc<BlockRegistry> {
        Arc::new(BlockRegistry::load_default().expect("default registry should load"))
    }

    #[test]
    fn queue_coalesces_duplicate_generate_requests() {
        let mut queue = JobQueue::new(&JobConfig::default());
        let request = JobRequest::GenerateChunk {
            coord: ChunkCoord(0, 0, 0),
            meta: WorldMeta::default(),
            registry: test_registry(),
        };

        assert_eq!(
            queue.enqueue(request.clone()).unwrap(),
            JobEnqueueOutcome::Enqueued
        );
        assert_eq!(
            queue.enqueue(request).unwrap(),
            JobEnqueueOutcome::Coalesced
        );
    }

    #[test]
    fn queue_drains_completed_in_submit_order_even_if_workers_finish_out_of_order() {
        let mut queue = JobQueue::new(&JobConfig::default());
        let request_a = JobRequest::GenerateChunk {
            coord: ChunkCoord(0, 0, 0),
            meta: WorldMeta::default(),
            registry: test_registry(),
        };
        let request_b = JobRequest::GenerateChunk {
            coord: ChunkCoord(1, 0, 0),
            meta: WorldMeta::default(),
            registry: test_registry(),
        };

        queue.enqueue(request_a.clone()).unwrap();
        queue.enqueue(request_b.clone()).unwrap();

        let queued_a = queue.take_next_pending().unwrap();
        let queued_b = queue.take_next_pending().unwrap();
        queue.mark_running(queued_a.clone());
        queue.mark_running(queued_b.clone());

        queue.finish_running(
            queued_b.sequence,
            queued_b.request.coalesce_key(),
            JobResult::JobFailed {
                request: request_b.clone(),
                error: JobError::Shutdown,
            },
        );
        assert!(queue.drain_completed_limit(usize::MAX).is_empty());

        queue.finish_running(
            queued_a.sequence,
            queued_a.request.coalesce_key(),
            JobResult::JobFailed {
                request: request_a.clone(),
                error: JobError::Shutdown,
            },
        );

        assert_eq!(
            queue.drain_completed_limit(usize::MAX),
            vec![
                JobResult::JobFailed {
                    request: request_a,
                    error: JobError::Shutdown,
                },
                JobResult::JobFailed {
                    request: request_b,
                    error: JobError::Shutdown,
                },
            ]
        );
    }

    #[test]
    fn queue_drain_completed_limit_keeps_later_results_buffered() {
        let mut queue = JobQueue::new(&JobConfig::default());
        let request_a = JobRequest::GenerateChunk {
            coord: ChunkCoord(0, 0, 0),
            meta: WorldMeta::default(),
            registry: test_registry(),
        };
        let request_b = JobRequest::GenerateChunk {
            coord: ChunkCoord(1, 0, 0),
            meta: WorldMeta::default(),
            registry: test_registry(),
        };

        queue.enqueue(request_a.clone()).unwrap();
        queue.enqueue(request_b.clone()).unwrap();

        let queued_a = queue.take_next_pending().unwrap();
        let queued_b = queue.take_next_pending().unwrap();
        queue.mark_running(queued_a.clone());
        queue.mark_running(queued_b.clone());

        queue.finish_running(
            queued_a.sequence,
            queued_a.request.coalesce_key(),
            JobResult::JobFailed {
                request: request_a.clone(),
                error: JobError::Shutdown,
            },
        );
        queue.finish_running(
            queued_b.sequence,
            queued_b.request.coalesce_key(),
            JobResult::JobFailed {
                request: request_b.clone(),
                error: JobError::Shutdown,
            },
        );

        assert_eq!(
            queue.drain_completed_limit(1),
            vec![JobResult::JobFailed {
                request: request_a,
                error: JobError::Shutdown,
            }]
        );
        assert_eq!(
            queue.drain_completed_limit(usize::MAX),
            vec![JobResult::JobFailed {
                request: request_b,
                error: JobError::Shutdown,
            }]
        );
    }

    #[test]
    fn queue_coalesces_duplicate_work_until_completed_result_is_drained() {
        let mut queue = JobQueue::new(&JobConfig::default());
        let request = JobRequest::GenerateChunk {
            coord: ChunkCoord(3, 0, -2),
            meta: WorldMeta::default(),
            registry: test_registry(),
        };

        assert_eq!(
            queue.enqueue(request.clone()).unwrap(),
            JobEnqueueOutcome::Enqueued
        );
        let queued = queue.take_next_pending().unwrap();
        queue.mark_running(queued.clone());
        queue.finish_running(
            queued.sequence,
            queued.request.coalesce_key(),
            JobResult::JobFailed {
                request: request.clone(),
                error: JobError::Shutdown,
            },
        );

        assert_eq!(
            queue.enqueue(request.clone()).unwrap(),
            JobEnqueueOutcome::Coalesced
        );
        assert_eq!(queue.drain_completed_limit(usize::MAX).len(), 1);
        assert_eq!(queue.enqueue(request).unwrap(), JobEnqueueOutcome::Enqueued);
    }
}
