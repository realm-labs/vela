use vela_common::SourceId;
use vela_hir::binding::ServiceLexicalCapability;
use vela_hir::script_methods::{ScriptMethodCatalog, ScriptMethodCatalogMode};
use vela_hir::service_impl::{ServiceImplCatalog, ServiceImplCatalogErrorKind};
use vela_hir::source_ingestion::build_single_source;

#[test]
fn catalogs_sparse_service_impl_methods_without_dynamic_method_registration() {
    let sources = build_single_source(
        SourceId::new(1),
        r#"
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(turn, player, items) {
        return items;
    }
}
"#,
    )
    .expect("service implementation should enter HIR");
    let catalog =
        ServiceImplCatalog::from_graph(sources.graph()).expect("valid service implementation");
    let implementation = catalog
        .implementations()
        .next()
        .expect("one service implementation");
    let method = implementation.methods().next().expect("one method");

    assert_eq!(
        implementation.service_path_text(),
        "game::inventory::InventoryService"
    );
    assert_eq!(implementation.implementation_path(), ["InventoryHotfix"]);
    assert_eq!(method.name(), "grant");
    assert_eq!(
        method
            .signature()
            .params
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<Vec<_>>(),
        ["turn", "player", "items"]
    );
    assert_eq!(
        sources
            .graph()
            .impl_method_body(method.node())
            .expect("method body")
            .id,
        method.body()
    );

    let ordinary = ScriptMethodCatalog::from_graph(
        sources.graph(),
        ScriptMethodCatalogMode::single_source(sources.modules()[0], "test"),
    )
    .expect("ordinary method catalog");
    assert!(
        ordinary.is_empty(),
        "service bodies must not become dynamic methods on InventoryHotfix"
    );
}

#[test]
fn binds_service_capabilities_only_as_direct_call_receivers() {
    let sources = build_single_source(
        SourceId::new(3),
        r#"
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(value) {
        let granted = base.grant(value);
        return services.audit.record(granted);
    }
}
"#,
    )
    .expect("direct service capability calls should enter HIR");
    let catalog =
        ServiceImplCatalog::from_graph(sources.graph()).expect("valid service implementation");
    let method = catalog
        .implementations()
        .next()
        .and_then(|implementation| implementation.methods().next())
        .expect("one service method");
    let bindings = sources
        .graph()
        .impl_method_bindings(method.node())
        .expect("service method bindings");

    let capabilities = bindings
        .service_capabilities()
        .map(|(_, capability)| capability)
        .collect::<Vec<_>>();
    assert_eq!(
        capabilities,
        [
            ServiceLexicalCapability::Base,
            ServiceLexicalCapability::Services
        ]
    );
}

#[test]
fn rejects_service_capabilities_outside_their_non_escaping_call_shapes() {
    let cases = [
        (
            r#"
fn grant(value) {
    return base.grant(value);
}
"#,
            "hir::service_capability_outside_impl",
        ),
        (
            r#"
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(value) {
        let callable = base.grant;
        return callable(value);
    }
}
"#,
            "hir::invalid_service_capability_use",
        ),
        (
            r#"
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(value) {
        let deferred = || base.grant(value);
        return deferred();
    }
}
"#,
            "hir::service_capability_capture",
        ),
        (
            r#"
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(base) {
        return base;
    }
}
"#,
            "hir::reserved_service_capability",
        ),
    ];

    for (source, expected_code) in cases {
        let error = build_single_source(SourceId::new(4), source)
            .expect_err("invalid service capability use should fail HIR ingestion");
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some(expected_code)),
            "expected {expected_code}, got {:?}",
            error.diagnostics()
        );
    }
}

#[test]
fn rejects_malformed_or_trait_style_service_impl_declarations() {
    let malformed = build_single_source(
        SourceId::new(1),
        r#"
#[service_impl(service = game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(value) { return value; }
}
"#,
    )
    .expect("attribute syntax remains valid");
    let error =
        ServiceImplCatalog::from_graph(malformed.graph()).expect_err("named argument is invalid");
    assert_eq!(error.kind(), &ServiceImplCatalogErrorKind::InvalidAttribute);

    let trait_style = build_single_source(
        SourceId::new(2),
        r#"
#[service_impl(game::inventory::InventoryService)]
impl PatchTrait for InventoryHotfix {
    fn grant(value) { return value; }
}
"#,
    )
    .expect("trait impl syntax remains valid");
    let error = ServiceImplCatalog::from_graph(trait_style.graph())
        .expect_err("service_impl is its own declaration form");
    assert_eq!(
        error.kind(),
        &ServiceImplCatalogErrorKind::TraitImplUnsupported
    );
}
