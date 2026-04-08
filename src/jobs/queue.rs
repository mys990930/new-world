use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use super::config::JobConfig;
use super::request::{JobCoalesceKey, JobRequest};
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
            shutting_down: false,
        }
    }

    pub fn enqueue(&mut self, request: JobRequest) -> Result<JobEnqueueOutcome, JobSubmitError> {
        if self.shutting_down {
            return Err(JobSubmitError::Shutdown);
        }

        let key = request.coalesce_key();
        if self.pending_keys.contains(&key) || self.running.contains_key(&key) {
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
        self.completed.insert(sequence, result);
        true
    }

    pub fn begin_shutdown(&mut self) {
        if self.shutting_down {
            return;
        }

        self.shutting_down = true;
        while let Some(queued) = self.pending.pop_front() {
            self.pending_keys.remove(&queued.request.coalesce_key());
            self.completed.insert(
                queued.sequence,
                JobResult::JobFailed {
                    request: queued.request,
                    error: JobError::Shutdown,
                },
            );
        }
    }

    pub fn drain_completed(&mut self) -> Vec<JobResult> {
        let mut drained = Vec::new();
        while let Some(result) = self.completed.remove(&self.next_drain_sequence) {
            drained.push(result);
            self.next_drain_sequence = self.next_drain_sequence.saturating_add(1);
        }
        drained
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, WorldMeta};

    #[test]
    fn queue_coalesces_duplicate_generate_requests() {
        let mut queue = JobQueue::new(&JobConfig::default());
        let request = JobRequest::GenerateChunk {
            coord: ChunkCoord(0, 0, 0),
            meta: WorldMeta::default(),
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
        };
        let request_b = JobRequest::GenerateChunk {
            coord: ChunkCoord(1, 0, 0),
            meta: WorldMeta::default(),
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
        assert!(queue.drain_completed().is_empty());

        queue.finish_running(
            queued_a.sequence,
            queued_a.request.coalesce_key(),
            JobResult::JobFailed {
                request: request_a.clone(),
                error: JobError::Shutdown,
            },
        );

        assert_eq!(
            queue.drain_completed(),
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
}
