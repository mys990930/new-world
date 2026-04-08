#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobConfig {
    pub worker_count: usize,
    pub max_pending_requests: Option<usize>,
}

impl JobConfig {
    pub fn effective_worker_count(self) -> usize {
        self.worker_count.max(1)
    }
}

impl Default for JobConfig {
    fn default() -> Self {
        Self {
            worker_count: 1,
            max_pending_requests: None,
        }
    }
}
