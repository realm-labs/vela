use std::task::{Context, Poll, Waker};

use vela_engine::service::Service;
use vela_macros::{service, service_domain};

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    fn apply(&self, amount: i64) -> i64;
}

pub struct RustRewardService {
    offset: i64,
}

impl RewardService for RustRewardService {
    fn apply(&self, amount: i64) -> i64 {
        amount + self.offset
    }
}

#[service(path = "game::inventory")]
pub trait InventoryService: Send + Sync {
    fn capacity(&self) -> i64;
}

pub struct RustInventoryService {
    capacity: i64,
}

impl InventoryService for RustInventoryService {
    fn capacity(&self) -> i64 {
        self.capacity
    }
}

pub struct RequestContext;

#[service_domain(context = RequestContext)]
pub struct GameLogic {
    pub reward: Service<dyn RewardService>,
    pub inventory: Service<dyn InventoryService>,
}

#[service(path = "game::async_reward")]
pub trait AsyncRewardService: Send + Sync {
    async fn apply(&self, amount: i64) -> i64;
}

pub struct RustAsyncRewardService {
    offset: i64,
}

impl AsyncRewardService for RustAsyncRewardService {
    async fn apply(&self, amount: i64) -> i64 {
        amount + self.offset
    }
}

#[service_domain(context = RequestContext)]
pub struct AsyncGameLogic {
    pub reward: Service<dyn AsyncRewardService>,
}

#[test]
fn generated_domain_builds_engine_and_instance_defaults_together() {
    let app = GameLogic::builder(vela_engine::engine::Engine::builder())
        .task_scope(crate::support::task_scope())
        .emergency_patch_effect_ceiling(crate::support::patch_ceiling())
        .reward(RustRewardService { offset: 4 })
        .inventory(RustInventoryService { capacity: 23 })
        .build()
        .expect("service domain");
    let root = app.domain().pin();

    assert_eq!(root.reward().apply(1), 5);
    assert_eq!(root.inventory().capacity(), 23);
    assert_eq!(app.domain().schema().services().len(), 2);
    assert_eq!(
        app.engine()
            .service_set_schema()
            .expect("engine service schema")
            .id(),
        root.service_set_id()
    );
}

#[test]
fn generated_domain_requires_every_default_instance() {
    let error = match GameLogic::builder(vela_engine::engine::Engine::builder())
        .task_scope(crate::support::task_scope())
        .emergency_patch_effect_ceiling(crate::support::patch_ceiling())
        .reward(RustRewardService { offset: 1 })
        .build()
    {
        Ok(_) => panic!("missing inventory default must fail"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        vela_engine::service::ServiceDomainBuildError::MissingDefault {
            service: "inventory",
            ..
        }
    ));
}

#[test]
fn generated_domain_requires_explicit_task_scope_and_patch_ceiling() {
    let missing_scope = GameLogic::builder(vela_engine::engine::Engine::builder())
        .emergency_patch_effect_ceiling(crate::support::patch_ceiling())
        .reward(RustRewardService { offset: 1 })
        .inventory(RustInventoryService { capacity: 2 })
        .build()
        .err()
        .expect("scope is a required Service application authority");
    assert!(matches!(
        missing_scope,
        vela_engine::service::ServiceDomainBuildError::MissingTaskScope { .. }
    ));

    let missing_ceiling = GameLogic::builder(vela_engine::engine::Engine::builder())
        .task_scope(crate::support::task_scope())
        .reward(RustRewardService { offset: 1 })
        .inventory(RustInventoryService { capacity: 2 })
        .build()
        .err()
        .expect("patch ceiling is a required Service schema authority");
    assert!(matches!(
        missing_ceiling,
        vela_engine::service::ServiceDomainBuildError::MissingPatchEffectCeiling { .. }
    ));

    let missing_spawn = GameLogic::builder(vela_engine::engine::Engine::builder())
        .task_scope(crate::support::task_scope())
        .emergency_patch_effect_ceiling(vela_engine::native::EffectSet::host_write())
        .reward(RustRewardService { offset: 1 })
        .inventory(RustInventoryService { capacity: 2 })
        .build()
        .err()
        .expect("every emergency Service ceiling includes TaskSpawn");
    assert!(matches!(
        missing_spawn,
        vela_engine::service::ServiceDomainBuildError::PatchEffectCeilingMissingTaskSpawn { .. }
    ));
}

#[test]
fn generated_async_default_uses_object_safe_send_dispatch() {
    let app = AsyncGameLogic::builder(vela_engine::engine::Engine::builder())
        .task_scope(crate::support::task_scope())
        .emergency_patch_effect_ceiling(crate::support::patch_ceiling())
        .reward(RustAsyncRewardService { offset: 2 })
        .build()
        .expect("async service domain");
    let root = app.domain().pin();
    let mut future = root.reward().apply(40);
    assert_send(&future);
    let mut context = Context::from_waker(Waker::noop());

    assert_eq!(future.as_mut().poll(&mut context), Poll::Ready(42));
    assert!(
        app.domain().schema().services()[0].methods()[0]
            .callable
            .asyncness
            .is_async()
    );
}

fn assert_send<T: Send>(_: &T) {}
