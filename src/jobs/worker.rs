use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::request::{JobCoalesceKey, JobRequest};
use super::result::JobResult;
use super::routing;

const SLOW_JOB_LOG_THRESHOLD: Duration = Duration::from_millis(25);

#[derive(Debug, Clone)]
pub(crate) struct AssignedJob {
    pub sequence: u64,
    pub key: JobCoalesceKey,
    pub request: JobRequest,
}

#[derive(Debug)]
pub(crate) enum WorkerReport {
    Progress {
        result: JobResult,
    },
    Finished {
        worker_id: usize,
        sequence: u64,
        key: JobCoalesceKey,
        result: JobResult,
    },
}

#[derive(Debug)]
enum WorkerCommand {
    Execute(AssignedJob),
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkerSendError {
    pub worker_id: usize,
}

pub(crate) struct WorkerContext {
    workers: Vec<WorkerHandle>,
    result_receiver: Receiver<WorkerReport>,
}

struct WorkerHandle {
    sender: Sender<WorkerCommand>,
    join_handle: Option<JoinHandle<()>>,
}

impl WorkerContext {
    pub fn new(worker_count: usize) -> Self {
        let (result_sender, result_receiver) = mpsc::channel();
        let mut workers = Vec::with_capacity(worker_count);

        for worker_id in 0..worker_count {
            let (command_sender, command_receiver) = mpsc::channel();
            let result_sender = result_sender.clone();
            let join_handle = thread::Builder::new()
                .name(format!("jobs-worker-{worker_id}"))
                .spawn(move || worker_loop(worker_id, command_receiver, result_sender))
                .expect("failed to spawn jobs worker thread");

            workers.push(WorkerHandle {
                sender: command_sender,
                join_handle: Some(join_handle),
            });
        }

        Self {
            workers,
            result_receiver,
        }
    }

    pub fn send(&self, worker_id: usize, job: AssignedJob) -> Result<(), WorkerSendError> {
        let Some(worker) = self.workers.get(worker_id) else {
            return Err(WorkerSendError { worker_id });
        };

        worker
            .sender
            .send(WorkerCommand::Execute(job))
            .map_err(|_| WorkerSendError { worker_id })
    }

    pub fn try_recv(&self) -> Option<WorkerReport> {
        match self.result_receiver.try_recv() {
            Ok(report) => Some(report),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }

    pub fn shutdown(&mut self) {
        for worker in &self.workers {
            let _ = worker.sender.send(WorkerCommand::Shutdown);
        }

        for worker in &mut self.workers {
            if let Some(handle) = worker.join_handle.take() {
                let _ = handle.join();
            }
        }
    }
}

fn worker_loop(
    worker_id: usize,
    command_receiver: Receiver<WorkerCommand>,
    result_sender: Sender<WorkerReport>,
) {
    while let Ok(command) = command_receiver.recv() {
        match command {
            WorkerCommand::Execute(job) => {
                let trace_jobs = trace_jobs_enabled();
                let request_label = job.request.diagnostic_label();
                if trace_jobs {
                    println!(
                        "[perf] job start: worker={} seq={} request={}",
                        worker_id, job.sequence, request_label
                    );
                }
                let started = Instant::now();
                let mut emit_progress = |result| {
                    let _ = result_sender.send(WorkerReport::Progress { result });
                };
                let result = routing::execute(job.request, &mut emit_progress);
                let elapsed = started.elapsed();
                if trace_jobs || elapsed >= SLOW_JOB_LOG_THRESHOLD {
                    println!(
                        "[perf] job finish: worker={} seq={} request={} result={} elapsed_ms={:.2}",
                        worker_id,
                        job.sequence,
                        request_label,
                        result.diagnostic_label(),
                        duration_ms(elapsed)
                    );
                }
                if result_sender
                    .send(WorkerReport::Finished {
                        worker_id,
                        sequence: job.sequence,
                        key: job.key,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
            }
            WorkerCommand::Shutdown => break,
        }
    }
}

fn trace_jobs_enabled() -> bool {
    std::env::var_os("NEW_WORLD_TRACE_JOBS").is_some()
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
