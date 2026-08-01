//! Bounded thread-backed host scope used by generated Service examples.

use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use vela_common::CapabilitySet;
use vela_engine::native::EffectSet;
use vela_engine::task::{
    ScopedTask, ScopedTaskHost, TaskAdmissionError, TaskPolicy, TaskScope,
};
use vela_vm::budget::{CollectionLimits, ExecutionLimits};

struct ThreadTaskHost {
    active: Arc<AtomicUsize>,
    maximum: usize,
}

impl ScopedTaskHost for ThreadTaskHost {
    fn admit(&self, task: ScopedTask) -> Result<(), TaskAdmissionError> {
        self.active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < self.maximum).then_some(active + 1)
            })
            .map_err(|_| TaskAdmissionError::CapacityExceeded {
                maximum: self.maximum,
            })?;
        let active = Arc::clone(&self.active);
        std::thread::spawn(move || {
            let (_, _, future) = task.into_parts();
            let _ = crate::async_executor::block_on(future);
            active.fetch_sub(1, Ordering::AcqRel);
        });
        Ok(())
    }
}

pub fn scope() -> TaskScope {
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
    let policy = TaskPolicy::new(
        NonZeroUsize::new(MAXIMUM).expect("non-zero"),
        NonZeroUsize::new(MAXIMUM).expect("non-zero"),
        limits,
        NonZeroU64::new(256).expect("non-zero"),
        Duration::from_secs(10),
        CapabilitySet::all(),
    )
    .expect("finite example task policy");
    TaskScope::new(
        Arc::new(ThreadTaskHost {
            active: Arc::new(AtomicUsize::new(0)),
            maximum: MAXIMUM,
        }),
        policy,
    )
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
