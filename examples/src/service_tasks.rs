//! Bounded thread-backed host scope used by generated Service examples.

use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use vela_common::CapabilitySet;
use vela_engine::native::EffectSet;
use vela_engine::task::{
    ScopedTask, ScopedTaskCompletion, ScopedTaskHost, TaskAdmissionError,
    TaskCancellationReason, TaskContinuationOutcome, TaskPolicy, TaskScope,
};
use vela_engine::runtime::{CallArgs, CallOptions};
use vela_vm::budget::{CollectionLimits, ExecutionLimits};

struct ActorTaskHost {
    active: Arc<AtomicUsize>,
    maximum: usize,
    closed: Arc<AtomicBool>,
    completions: std::sync::mpsc::SyncSender<ScopedTaskCompletion>,
}

impl ScopedTaskHost for ActorTaskHost {
    fn admit(&self, task: ScopedTask) -> Result<(), TaskAdmissionError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(TaskAdmissionError::ScopeClosed);
        }
        self.active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < self.maximum).then_some(active + 1)
            })
            .map_err(|_| TaskAdmissionError::CapacityExceeded {
                maximum: self.maximum,
        })?;
        let active = Arc::clone(&self.active);
        let closed = Arc::clone(&self.closed);
        let completions = self.completions.clone();
        std::thread::spawn(move || {
            let completion = crate::async_executor::block_on(task.into_completion_future());
            if closed.load(Ordering::Acquire) {
                drop(completion.cancel(TaskCancellationReason::ScopeClosed));
            } else {
                match completions.try_send(completion) {
                    Ok(()) => {}
                    Err(std::sync::mpsc::TrySendError::Full(completion)) => {
                        drop(completion.cancel(TaskCancellationReason::CompletionQueueFull));
                    }
                    Err(std::sync::mpsc::TrySendError::Disconnected(completion)) => {
                        drop(completion.cancel(TaskCancellationReason::ScopeClosed));
                    }
                }
            }
            active.fetch_sub(1, Ordering::AcqRel);
        });
        Ok(())
    }
}

/// Minimal actor-style integration: worker threads only publish owned
/// completions; an actor turn explicitly calls `resume_one` with freshly
/// acquired handler arguments.
pub struct ActorTaskAdapter {
    scope: TaskScope,
    closed: Arc<AtomicBool>,
    completions: Mutex<std::sync::mpsc::Receiver<ScopedTaskCompletion>>,
}

impl ActorTaskAdapter {
    #[must_use]
    pub fn new() -> Self {
        let policy = policy();
        let maximum = policy.max_active_tasks().get();
        let closed = Arc::new(AtomicBool::new(false));
        let (sender, receiver) =
            std::sync::mpsc::sync_channel(policy.max_queued_completions().get());
        let host = Arc::new(ActorTaskHost {
            active: Arc::new(AtomicUsize::new(0)),
            maximum,
            closed: Arc::clone(&closed),
            completions: sender,
        });
        Self {
            scope: TaskScope::new(host, policy),
            closed,
            completions: Mutex::new(receiver),
        }
    }

    #[must_use]
    pub fn task_scope(&self) -> TaskScope {
        self.scope.clone()
    }

    pub fn resume_one<'host>(
        &self,
        args: CallArgs<'host>,
        options: CallOptions,
    ) -> Option<TaskContinuationOutcome> {
        let completion = self
            .completions
            .lock()
            .expect("actor completion queue lock")
            .try_recv()
            .ok()?;
        Some(completion.resume(args, options))
    }

    pub fn shutdown(&self) {
        self.closed.store(true, Ordering::Release);
        let completions = self
            .completions
            .lock()
            .expect("actor completion queue lock");
        while let Ok(completion) = completions.try_recv() {
            drop(completion.cancel(TaskCancellationReason::ScopeClosed));
        }
    }
}

impl Default for ActorTaskAdapter {
    fn default() -> Self {
        Self::new()
    }
}

fn policy() -> TaskPolicy {
    const MAXIMUM: usize = 8;
    let limits = ExecutionLimits {
        execution_unit_limit: 250_000,
        memory_limit_bytes: 4 * 1024 * 1024,
        max_call_depth: 128,
        collection_limits: CollectionLimits {
            max_array_len: 16_384,
            max_map_entries: 16_384,
            max_set_len: 16_384,
        },
        host_call_limit: 256,
    };
    TaskPolicy::new(
        NonZeroUsize::new(MAXIMUM).expect("non-zero"),
        NonZeroUsize::new(MAXIMUM).expect("non-zero"),
        limits,
        NonZeroU64::new(256).expect("non-zero"),
        Duration::from_secs(10),
        CapabilitySet::all(),
    )
    .expect("finite example task policy")
}

pub fn scope() -> TaskScope {
    ActorTaskAdapter::new().task_scope()
}

pub fn emergency_patch_effect_ceiling() -> EffectSet {
    EffectSet::task_spawn()
        .union(EffectSet::host_write())
        .union(EffectSet::event_emit())
        .union(EffectSet::time())
        .union(EffectSet::random())
        .union(EffectSet::io_read())
        .union(EffectSet::io_write())
        .union(EffectSet::reflection_read())
        .union(EffectSet::reflection_write())
        .union(EffectSet::reflection_call())
}
