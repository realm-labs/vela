use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Arc;
use std::time::Duration;

use vela_common::CapabilitySet;
use vela_engine::native::EffectSet;
use vela_engine::task::{ScopedTask, ScopedTaskHost, TaskAdmissionError, TaskPolicy, TaskScope};
use vela_vm::budget::{CollectionLimits, ExecutionLimits};

struct DroppingTaskHost;

impl ScopedTaskHost for DroppingTaskHost {
    fn admit(&self, task: ScopedTask) -> Result<(), TaskAdmissionError> {
        drop(task);
        Ok(())
    }
}

pub fn task_policy() -> TaskPolicy {
    let limits = ExecutionLimits {
        execution_unit_limit: 100_000,
        memory_limit_bytes: 1024 * 1024,
        max_call_depth: 64,
        collection_limits: CollectionLimits {
            max_array_len: 4096,
            max_map_entries: 4096,
            max_set_len: 4096,
        },
        host_call_limit: 128,
    };
    TaskPolicy::new(
        NonZeroUsize::new(8).expect("non-zero"),
        NonZeroUsize::new(8).expect("non-zero"),
        limits,
        NonZeroU64::new(128).expect("non-zero"),
        Duration::from_secs(5),
        CapabilitySet::all(),
    )
    .expect("finite test task policy")
}

pub fn dropping_task_scope() -> TaskScope {
    TaskScope::new(Arc::new(DroppingTaskHost), task_policy())
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
