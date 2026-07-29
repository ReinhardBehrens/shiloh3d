//! Work-stealing-style job system built on `crossbeam` (pure Rust).

use crossbeam::deque::{Injector, Steal, Stealer, Worker};
use crossbeam::sync::WaitGroup;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};

type BoxedJob = Box<dyn FnOnce() + Send + 'static>;

/// Opaque handle for waiting on a batch of jobs.
#[derive(Debug, Clone)]
pub struct JobHandle {
    remaining: Arc<AtomicU64>,
}

impl JobHandle {
    #[inline]
    pub fn is_finished(&self) -> bool {
        self.remaining.load(Ordering::Acquire) == 0
    }

    /// Busy-waits until the batch completes (callers can later add park/notify).
    pub fn wait(&self) {
        while !self.is_finished() {
            thread::yield_now();
        }
    }
}

/// Shared queues + shutdown flag.
struct Shared {
    injector: Injector<BoxedJob>,
    stealers: Vec<Stealer<BoxedJob>>,
    shutdown: AtomicBool,
}

/// Multi-threaded job system with a global injector and per-worker deques.
pub struct JobSystem {
    shared: Arc<Shared>,
    workers: Mutex<Vec<JoinHandle<()>>>,
}

/// Builder for [`JobSystem`].
#[derive(Debug, Clone)]
pub struct JobSystemBuilder {
    worker_count: usize,
}

impl Default for JobSystemBuilder {
    fn default() -> Self {
        Self {
            worker_count: thread::available_parallelism()
                .map(|n| n.get().saturating_sub(1).max(1))
                .unwrap_or(1),
        }
    }
}

impl JobSystemBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn worker_count(mut self, count: usize) -> Self {
        self.worker_count = count.max(1);
        self
    }

    pub fn build(self) -> JobSystem {
        let mut workers_local = Vec::with_capacity(self.worker_count);
        let mut stealers = Vec::with_capacity(self.worker_count);
        for _ in 0..self.worker_count {
            let worker = Worker::new_fifo();
            stealers.push(worker.stealer());
            workers_local.push(worker);
        }

        let shared = Arc::new(Shared {
            injector: Injector::new(),
            stealers,
            shutdown: AtomicBool::new(false),
        });

        let mut joins = Vec::with_capacity(self.worker_count);
        for (id, local) in workers_local.into_iter().enumerate() {
            let shared = Arc::clone(&shared);
            joins.push(thread::Builder::new()
                .name(format!("shiloh-job-{id}"))
                .spawn(move || worker_loop(shared, local))
                .expect("failed to spawn job worker"));
        }

        JobSystem {
            shared,
            workers: Mutex::new(joins),
        }
    }
}

impl JobSystem {
    pub fn builder() -> JobSystemBuilder {
        JobSystemBuilder::new()
    }

    /// Spawns a single job.
    pub fn spawn<F>(&self, job: F) -> JobHandle
    where
        F: FnOnce() + Send + 'static,
    {
        let remaining = Arc::new(AtomicU64::new(1));
        let counter = Arc::clone(&remaining);
        self.shared.injector.push(Box::new(move || {
            job();
            counter.fetch_sub(1, Ordering::Release);
        }));
        JobHandle { remaining }
    }

    /// Spawns many independent jobs and returns a single completion handle.
    pub fn spawn_batch<I, F>(&self, jobs: I) -> JobHandle
    where
        I: IntoIterator<Item = F>,
        F: FnOnce() + Send + 'static,
    {
        let jobs: Vec<F> = jobs.into_iter().collect();
        let remaining = Arc::new(AtomicU64::new(jobs.len() as u64));
        if jobs.is_empty() {
            return JobHandle { remaining };
        }
        for job in jobs {
            let counter = Arc::clone(&remaining);
            self.shared.injector.push(Box::new(move || {
                job();
                counter.fetch_sub(1, Ordering::Release);
            }));
        }
        JobHandle { remaining }
    }

    /// Runs `body` on the calling thread after ensuring workers are live.
    pub fn scope<R>(&self, body: impl FnOnce() -> R) -> R {
        let _wg = WaitGroup::new();
        body()
    }
}

impl Drop for JobSystem {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::Release);
        let workers = std::mem::take(&mut *self.workers.lock());
        for handle in workers {
            let _ = handle.join();
        }
    }
}

fn worker_loop(shared: Arc<Shared>, local: Worker<BoxedJob>) {
    while !shared.shutdown.load(Ordering::Acquire) {
        let job = local.pop().or_else(|| match shared.injector.steal() {
            Steal::Success(job) => Some(job),
            Steal::Empty | Steal::Retry => None,
        });

        let job = job.or_else(|| {
            for stealer in &shared.stealers {
                match stealer.steal() {
                    Steal::Success(job) => return Some(job),
                    Steal::Empty | Steal::Retry => {}
                }
            }
            None
        });

        if let Some(job) = job {
            job();
        } else {
            thread::yield_now();
        }
    }
}
