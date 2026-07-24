use vela_common::SourceId;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_engine::service::{
    ServiceMethodSelection, ServiceSchema, ServiceSetSchema, ServiceSourceErrorKind,
    ServiceSourceManifest,
};
use vela_hir::source_ingestion::build_single_source;
use vela_macros::{service, service_set};
use vela_vm::owned_value::OwnedValue;

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

pub struct RequestContext;

#[service_set(context = RequestContext)]
pub struct TestServices {
    #[vela::default(RustInventoryService)]
    pub inventory: dyn InventoryService,
}

#[test]
fn source_manifest_imports_schema_and_keeps_adjacent_methods_on_rust() {
    let schema = schema();
    let text = r#"
#[service_impl(test::inventory)]
impl InventoryHotfix {
    fn grant(value) {
        return value + 1;
    }
}
"#;
    let sources = source(text);
    let manifest =
        ServiceSourceManifest::link(sources.graph(), &schema).expect("schema-linked manifest");
    assert_eq!(manifest.len(), 1);
    let table = manifest.into_snapshot(&schema).expect("complete snapshot");
    let inventory = service(&schema);
    let grant = method(inventory, "grant");
    let remove = method(inventory, "remove");

    let ServiceMethodSelection::Vela(target) = table
        .get(inventory.id(), grant.id)
        .expect("grant selection")
    else {
        panic!("grant should select Vela");
    };
    assert_eq!(target.implementation(), "InventoryHotfix");
    assert_eq!(target.signature().params[0].name, "value");
    assert_eq!(target.symbol(), "__service_impl.test.inventory.grant");
    let engine = engine();
    let compiled = engine
        .compile_source(text)
        .expect("service source should compile into hidden bytecode");
    let artifact = engine
        .link_compiled_program(compiled)
        .expect("service bytecode should link");
    assert!(
        artifact
            .program()
            .entry_point_by_id(target.function())
            .is_some(),
        "schema-linked target must name compiled bytecode"
    );
    let mut runtime =
        Runtime::from_linked_artifact(engine, artifact).expect("service execution runtime");
    let output = target
        .call(
            &mut runtime,
            CallArgs::from_positional([OwnedValue::i64(6)]),
            CallOptions::unbounded(),
        )
        .expect("selected Vela service method");
    assert_eq!(
        runtime.value_to_owned(&output).expect("owned result"),
        OwnedValue::i64(7)
    );
    assert_eq!(
        table.get(inventory.id(), remove.id),
        Some(&ServiceMethodSelection::RustDefault)
    );
}

#[test]
fn source_manifest_rejects_unknown_duplicate_and_incompatible_claims() {
    let schema = schema();
    let unknown_service = source(
        r#"
#[service_impl(test::missing)]
impl MissingHotfix {
    fn grant(value) { return value; }
}
"#,
    );
    assert!(matches!(
        ServiceSourceManifest::link(unknown_service.graph(), &schema)
            .expect_err("unknown service")
            .kind(),
        ServiceSourceErrorKind::UnknownService { .. }
    ));

    let unknown_method = source(
        r#"
#[service_impl(test::inventory)]
impl InventoryHotfix {
    fn missing(value) { return value; }
}
"#,
    );
    assert!(matches!(
        ServiceSourceManifest::link(unknown_method.graph(), &schema)
            .expect_err("unknown method")
            .kind(),
        ServiceSourceErrorKind::UnknownMethod { .. }
    ));

    let duplicate = source(
        r#"
#[service_impl(test::inventory)]
impl FirstHotfix {
    fn grant(value) { return value; }
}
#[service_impl(test::inventory)]
impl SecondHotfix {
    fn grant(value) { return value + 1; }
}
"#,
    );
    assert!(matches!(
        ServiceSourceManifest::link(duplicate.graph(), &schema)
            .expect_err("duplicate method")
            .kind(),
        ServiceSourceErrorKind::DuplicateMethodClaim { .. }
    ));

    let wrong_arity = source(
        r#"
#[service_impl(test::inventory)]
impl InventoryHotfix {
    fn grant(first, second) { return first + second; }
}
"#,
    );
    assert!(matches!(
        ServiceSourceManifest::link(wrong_arity.graph(), &schema)
            .expect_err("wrong arity")
            .kind(),
        ServiceSourceErrorKind::ParameterCountMismatch {
            expected: 1,
            actual: 2,
            ..
        }
    ));

    let wrong_asyncness = source(
        r#"
#[service_impl(test::inventory)]
impl InventoryHotfix {
    async fn grant(value) { return value; }
}
"#,
    );
    assert!(matches!(
        ServiceSourceManifest::link(wrong_asyncness.graph(), &schema)
            .expect_err("wrong asyncness")
            .kind(),
        ServiceSourceErrorKind::AsyncnessMismatch { .. }
    ));
}

fn source(text: &str) -> vela_hir::source_ingestion::HirSourceSet {
    build_single_source(SourceId::new(1), text).expect("valid Vela source")
}

fn schema() -> ServiceSetSchema {
    let engine = engine();
    TestServices::new(&engine.type_bindings())
        .expect("generated service schema")
        .schema()
        .clone()
}

fn engine() -> vela_engine::engine::Engine {
    TestServices::register_types(vela_engine::engine::Engine::builder())
        .build()
        .expect("generated registrations")
}

fn service(schema: &ServiceSetSchema) -> &ServiceSchema {
    schema
        .services()
        .iter()
        .find(|service| service.path() == "test::inventory")
        .expect("inventory schema")
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
