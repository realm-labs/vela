use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Arc;
use std::time::Duration;

use vela_common::CapabilitySet;
use vela_engine::task::{ScopedTask, ScopedTaskHost, TaskAdmissionError, TaskPolicy, TaskScope};
use vela_vm::budget::{CollectionLimits, ExecutionLimits};

struct DroppingTaskHost;

impl ScopedTaskHost for DroppingTaskHost {
    fn admit(&self, task: ScopedTask) -> Result<(), TaskAdmissionError> {
        drop(task);
        Ok(())
    }
}

pub fn task_scope() -> TaskScope {
    let policy = TaskPolicy::new(
        NonZeroUsize::new(4).expect("non-zero"),
        NonZeroUsize::new(4).expect("non-zero"),
        ExecutionLimits {
            execution_unit_limit: 100_000,
            memory_limit_bytes: 1024 * 1024,
            max_call_depth: 64,
            collection_limits: CollectionLimits {
                max_array_len: 4096,
                max_map_entries: 4096,
                max_set_len: 4096,
            },
            host_call_limit: 128,
        },
        NonZeroU64::new(128).expect("non-zero"),
        Duration::from_secs(5),
        CapabilitySet::all(),
    )
    .expect("finite task policy");
    TaskScope::new(Arc::new(DroppingTaskHost), policy)
}

pub fn patch_ceiling() -> vela_engine::native::EffectSet {
    vela_engine::native::EffectSet::task_spawn().union(vela_engine::native::EffectSet::host_write())
}
