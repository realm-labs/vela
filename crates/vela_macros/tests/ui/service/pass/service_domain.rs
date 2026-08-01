use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Arc;
use std::time::Duration;

use vela_common::{Capability, CapabilitySet};
use vela_engine::service::Service;
use vela_engine::task::{
    ScopedTask, ScopedTaskHost, TaskAdmissionError, TaskPolicy, TaskScope,
};
use vela_macros::{service, service_domain};
use vela_vm::budget::{CollectionLimits, ExecutionLimits};

struct DroppingTaskHost;

impl ScopedTaskHost for DroppingTaskHost {
    fn admit(&self, task: ScopedTask) -> Result<(), TaskAdmissionError> {
        drop(task);
        Ok(())
    }
}

fn task_scope() -> TaskScope {
    TaskScope::new(
        Arc::new(DroppingTaskHost),
        TaskPolicy::new(
            NonZeroUsize::MIN,
            NonZeroUsize::MIN,
            ExecutionLimits::new(1_000, 64 * 1024, 16).with_collection_limits(
                CollectionLimits {
                    max_array_len: 128,
                    max_map_entries: 128,
                    max_set_len: 128,
                },
            ),
            NonZeroU64::MIN,
            Duration::from_secs(1),
            CapabilitySet::new().with(Capability::TaskSpawn),
        )
        .unwrap(),
    )
}

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    fn apply(&self, amount: i64) -> i64;
}

pub struct RustRewardService;

impl RewardService for RustRewardService {
    fn apply(&self, amount: i64) -> i64 {
        amount
    }
}

pub struct RequestContext;

#[service_domain(context = RequestContext)]
pub struct GameLogic {
    pub reward: Service<dyn RewardService>,
}

fn main() {
    let app = GameLogic::builder(
        vela_engine::engine::Engine::builder().capability(Capability::TaskSpawn),
    )
        .task_scope(task_scope())
        .emergency_patch_effect_ceiling(vela_engine::native::EffectSet::task_spawn())
        .reward(RustRewardService)
        .build()
        .unwrap();
    app.domain().pin().reward().apply(1);
}
