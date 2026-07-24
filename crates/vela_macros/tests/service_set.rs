use std::sync::Arc;

use vela_macros::{service, service_set};

#[service(path = "game::reward")]
pub trait RewardService: Send + Sync {
    fn apply(&self, amount: i64) -> i64;
}

pub struct RustRewardService;

impl RewardService for RustRewardService {
    fn apply(&self, amount: i64) -> i64 {
        amount + 1
    }
}

#[service(path = "game::inventory")]
pub trait InventoryService: Send + Sync {
    fn capacity(&self) -> i64;
}

pub struct RustInventoryService;

impl InventoryService for RustInventoryService {
    fn capacity(&self) -> i64 {
        10
    }
}

pub struct RequestContext;

#[service_set(context = RequestContext)]
pub struct GameServices {
    #[vela::default(RustRewardService)]
    pub reward: dyn RewardService,
    #[vela::default(RustInventoryService)]
    pub inventory: dyn InventoryService,
}

struct PatchedRewardService;

impl RewardService for PatchedRewardService {
    fn apply(&self, amount: i64) -> i64 {
        amount + 100
    }
}

struct PatchedInventoryService;

impl InventoryService for PatchedInventoryService {
    fn capacity(&self) -> i64 {
        20
    }
}

fn patched_generation() -> GameServicesGeneration {
    GameServicesGeneration::new(
        Arc::new(PatchedRewardService),
        Arc::new(PatchedInventoryService),
    )
}

#[test]
fn generated_set_publishes_and_pins_one_complete_rust_generation() {
    let engine = GameServices::register_types(vela_engine::engine::Engine::builder())
        .build()
        .expect("service registration bundle");
    let services = GameServices::new(&engine.type_bindings()).expect("service schema");
    let old = services.pin();

    assert_eq!(old.reward().apply(1), 2);
    assert_eq!(old.inventory().capacity(), 10);

    let candidate = services
        .stage_rust(&old, patched_generation())
        .expect("stage complete replacement generation");
    let installed = candidate.generation_id();
    let rollback = services
        .activate_if_current(candidate)
        .expect("activate exact base");
    let new = services.pin();

    assert_eq!(old.reward().apply(1), 2);
    assert_eq!(old.inventory().capacity(), 10);
    assert_eq!(new.generation_id(), installed);
    assert_eq!(new.reward().apply(1), 101);
    assert_eq!(new.inventory().capacity(), 20);

    services
        .rollback_if_current(rollback)
        .expect("rollback exact installed generation");
    let restored = services.pin();
    assert_eq!(restored.reward().apply(1), 2);
    assert_eq!(restored.inventory().capacity(), 10);
    assert_eq!(old.service_set_id(), restored.service_set_id());
    assert_eq!(services.schema().services().len(), 2);
}

#[test]
fn generated_set_rejects_stale_activation_and_rollback() {
    let engine = GameServices::register_types(vela_engine::engine::Engine::builder())
        .build()
        .expect("service registration bundle");
    let services = GameServices::new(&engine.type_bindings()).expect("service schema");
    let base = services.pin();
    let first = services
        .stage_rust(&base, patched_generation())
        .expect("first candidate");
    let stale = services
        .stage_rust(&base, GameServicesGeneration::defaults())
        .expect("concurrent candidate");
    let first_rollback = services
        .activate_if_current(first)
        .expect("first activation");

    assert!(matches!(
        services.activate_if_current(stale),
        Err(vela_engine::service::ServicePublicationError::StaleBaseGeneration { .. })
    ));

    let active = services.pin();
    let second = services
        .stage_rust(&active, GameServicesGeneration::defaults())
        .expect("second candidate");
    services
        .activate_if_current(second)
        .expect("second activation");

    assert!(matches!(
        services.rollback_if_current(first_rollback),
        Err(vela_engine::service::ServicePublicationError::StaleRollback { .. })
    ));
    assert_eq!(services.pin().reward().apply(1), 2);
}
