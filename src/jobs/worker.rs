use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};

use super::request::{JobCoalesceKey, JobRequest};
use super::result::JobResult;
use super::routing;

#[derive(Debug, Clone)]
pub(crate) struct AssignedJob {
    pub sequence: u64,
    pub key: JobCoalesceKey,
    pub request: JobRequest,
}

#[derive(Debug)]
pub(crate) struct WorkerReport {
    pub worker_id: usize,
    pub sequence: u64,
    pub key: JobCoalesceKey,
    pub result: JobResult,
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
                let result = routing::execute(job.request);
                if result_sender
                    .send(WorkerReport {
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
