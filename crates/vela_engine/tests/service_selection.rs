use vela_common::{ServiceAbiFingerprint, ServiceGenerationId, ServiceId, ServiceMethodId};
use vela_engine::service::{
    Service, ServiceMethodSelection, ServiceMethodUpdate, ServiceSchema, ServiceSelectionError,
    ServiceSelectionTable, ServiceSetSchema,
};
use vela_macros::{service, service_domain};

#[service(path = "test::inventory")]
pub trait InventoryService: Send + Sync {
    fn grant(&self, amount: i64) -> i64;
    fn remove(&self, amount: i64) -> i64;
}

pub struct RustInventoryService;

impl InventoryService for RustInventoryService {
    fn grant(&self, amount: i64) -> i64 {
        amount
    }

    fn remove(&self, amount: i64) -> i64 {
        -amount
    }
}

#[service(path = "test::reward")]
pub trait RewardService: Send + Sync {
    fn apply(&self, amount: i64) -> i64;
}

pub struct RustRewardService;

impl RewardService for RustRewardService {
    fn apply(&self, amount: i64) -> i64 {
        amount * 2
    }
}

pub struct RequestContext;

#[service_domain(context = RequestContext)]
pub struct TestServices {
    pub inventory: Service<dyn InventoryService>,
    pub reward: Service<dyn RewardService>,
}

#[test]
fn snapshot_is_complete_and_unmentioned_methods_select_rust() {
    let schema = schema();
    let inventory = service(&schema, "test::inventory");
    let grant = method(inventory, "grant");
    let remove = method(inventory, "remove");
    let reward = service(&schema, "test::reward");
    let apply = method(reward, "apply");

    let table = ServiceSelectionTable::snapshot(
        &schema,
        [ServiceMethodUpdate::vela(
            inventory.id(),
            grant.id,
            inventory.abi_fingerprint(),
            "vela::grant_v1",
        )],
    )
    .expect("valid snapshot");

    assert_eq!(table.len(), 3);
    assert_eq!(
        table.get(inventory.id(), grant.id),
        Some(&ServiceMethodSelection::Vela("vela::grant_v1"))
    );
    assert_eq!(
        table.get(inventory.id(), remove.id),
        Some(&ServiceMethodSelection::RustDefault)
    );
    assert_eq!(
        table.get(reward.id(), apply.id),
        Some(&ServiceMethodSelection::RustDefault)
    );
}

#[test]
fn delta_inherits_exact_base_and_explicit_rust_default_clears_vela() {
    let schema = schema();
    let inventory = service(&schema, "test::inventory");
    let grant = method(inventory, "grant");
    let remove = method(inventory, "remove");
    let base = ServiceSelectionTable::snapshot(
        &schema,
        [ServiceMethodUpdate::vela(
            inventory.id(),
            grant.id,
            inventory.abi_fingerprint(),
            "vela::grant_v1",
        )],
    )
    .expect("valid base");

    let second = ServiceSelectionTable::delta(
        &schema,
        ServiceGenerationId::new(7),
        ServiceGenerationId::new(7),
        &base,
        [ServiceMethodUpdate::vela(
            inventory.id(),
            remove.id,
            inventory.abi_fingerprint(),
            "vela::remove_v2",
        )],
    )
    .expect("valid exact-base delta");
    assert_eq!(
        second.get(inventory.id(), grant.id),
        Some(&ServiceMethodSelection::Vela("vela::grant_v1"))
    );
    assert_eq!(
        second.get(inventory.id(), remove.id),
        Some(&ServiceMethodSelection::Vela("vela::remove_v2"))
    );

    let restored = ServiceSelectionTable::delta(
        &schema,
        ServiceGenerationId::new(8),
        ServiceGenerationId::new(8),
        &second,
        [ServiceMethodUpdate::rust_default(
            inventory.id(),
            grant.id,
            inventory.abi_fingerprint(),
        )],
    )
    .expect("explicit RustDefault");
    assert_eq!(
        restored.get(inventory.id(), grant.id),
        Some(&ServiceMethodSelection::RustDefault)
    );
    assert_eq!(
        restored.get(inventory.id(), remove.id),
        Some(&ServiceMethodSelection::Vela("vela::remove_v2"))
    );
}

#[test]
fn invalid_sparse_claims_are_rejected_before_composition() {
    let schema = schema();
    let inventory = service(&schema, "test::inventory");
    let grant = method(inventory, "grant");
    let update = || {
        ServiceMethodUpdate::vela(
            inventory.id(),
            grant.id,
            inventory.abi_fingerprint(),
            "vela::grant",
        )
    };

    assert!(matches!(
        ServiceSelectionTable::snapshot(&schema, [update(), update()]),
        Err(ServiceSelectionError::DuplicateMethodUpdate { .. })
    ));
    assert!(matches!(
        ServiceSelectionTable::snapshot(
            &schema,
            [ServiceMethodUpdate::vela(
                ServiceId::new(u128::MAX),
                grant.id,
                inventory.abi_fingerprint(),
                "vela::unknown_service",
            )],
        ),
        Err(ServiceSelectionError::UnknownService { .. })
    ));
    assert!(matches!(
        ServiceSelectionTable::snapshot(
            &schema,
            [ServiceMethodUpdate::vela(
                inventory.id(),
                ServiceMethodId::new(u128::MAX),
                inventory.abi_fingerprint(),
                "vela::unknown_method",
            )],
        ),
        Err(ServiceSelectionError::UnknownMethod { .. })
    ));
    assert!(matches!(
        ServiceSelectionTable::snapshot(
            &schema,
            [ServiceMethodUpdate::vela(
                inventory.id(),
                grant.id,
                ServiceAbiFingerprint::new(u64::MAX),
                "vela::wrong_abi",
            )],
        ),
        Err(ServiceSelectionError::IncompatibleServiceSchema { .. })
    ));
}

#[test]
fn delta_rejects_a_non_exact_base_without_changing_the_base() {
    let schema = schema();
    let inventory = service(&schema, "test::inventory");
    let grant = method(inventory, "grant");
    let base = ServiceSelectionTable::snapshot(
        &schema,
        [ServiceMethodUpdate::vela(
            inventory.id(),
            grant.id,
            inventory.abi_fingerprint(),
            "vela::grant_v1",
        )],
    )
    .expect("valid base");

    assert!(matches!(
        ServiceSelectionTable::delta(
            &schema,
            ServiceGenerationId::new(7),
            ServiceGenerationId::new(8),
            &base,
            [ServiceMethodUpdate::rust_default(
                inventory.id(),
                grant.id,
                inventory.abi_fingerprint(),
            )],
        ),
        Err(ServiceSelectionError::BaseGenerationMismatch { .. })
    ));
    assert_eq!(
        base.get(inventory.id(), grant.id),
        Some(&ServiceMethodSelection::Vela("vela::grant_v1"))
    );
}

fn schema() -> ServiceSetSchema {
    let app = TestServices::builder(vela_engine::engine::Engine::builder())
        .inventory(RustInventoryService)
        .reward(RustRewardService)
        .build()
        .expect("generated service domain");
    app.domain().schema().clone()
}

fn service<'schema>(schema: &'schema ServiceSetSchema, path: &str) -> &'schema ServiceSchema {
    schema
        .services()
        .iter()
        .find(|service| service.path() == path)
        .expect("fixture service")
}

fn method<'schema>(
    service: &'schema ServiceSchema,
    name: &str,
) -> &'schema vela_engine::service::ServiceMethodDescriptor {
    service
        .methods()
        .iter()
        .find(|method| method.path.rsplit("::").next() == Some(name))
        .expect("fixture method")
}
