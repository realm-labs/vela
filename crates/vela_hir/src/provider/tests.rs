use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_def::script_trait_id;
use vela_package::{ModulePath, PackageAlias, PackageId};

use super::*;
use crate::module_graph::ModuleSource;

#[test]
fn provider_service_is_inferred_from_resolved_impl_trait() {
    let app = PackageId::new("dev.vela.provider").expect("app package");
    let service = PackageId::new("dev.vela.service").expect("service package");
    let mut graph = ModuleGraph::with_package_dependencies(BTreeMap::from([(
        app.clone(),
        BTreeMap::from([(
            PackageAlias::new("service").expect("dependency alias"),
            service.clone(),
        )]),
    )]));
    graph.add_source(ModuleSource::new(
        SourceId::new(1),
        service,
        ModulePath::from_qualified("api"),
        r#"pub trait CommandProvider { fn run(self, value: i64) -> i64; }"#,
    ));
    graph.add_source(ModuleSource::new(
        SourceId::new(2),
        app,
        ModulePath::from_qualified("service"),
        r#"
pub struct SortInventory {}
#[provider(id = "sort_inventory")]
impl service::api::CommandProvider for SortInventory {
    pub fn run(self, value: i64) -> i64 { return value; }
}
"#,
    ));
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());

    let providers = discover_providers(&graph).expect("provider catalog");
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].key.provider().as_str(), "sort_inventory");
    assert_eq!(
        providers[0].key.service(),
        script_trait_id("dev.vela.service", "api::CommandProvider")
    );
    assert_eq!(providers[0].methods[0].name, "run");
}

#[test]
fn provider_rejects_non_trait_impl_and_nonzero_field_target() {
    let inherent = graph(
        r#"
pub struct SortInventory {}
#[provider(id = "sort_inventory")]
impl SortInventory {}
"#,
    );
    assert_code(&inherent, "hir::provider_requires_trait_impl");

    let fields = graph(
        r#"
pub trait CommandProvider { fn run(self) -> i64; }
pub struct SortInventory { count: i64 }
#[provider(id = "sort_inventory")]
impl CommandProvider for SortInventory { pub fn run(self) -> i64 { return 1; } }
"#,
    );
    assert_code(&fields, "hir::invalid_provider_target");
}

#[test]
fn provider_rejects_redundant_unknown_duplicate_or_missing_id() {
    for (attribute, code) in [
        ("#[provider]", "hir::invalid_provider_arguments"),
        (
            "#[provider(\"sort_inventory\")]",
            "hir::invalid_provider_argument_name",
        ),
        (
            "#[provider(service = \"CommandProvider\")]",
            "hir::invalid_provider_argument_name",
        ),
        (
            "#[provider(id = \"sort_inventory\", id = \"other\")]",
            "hir::invalid_provider_arguments",
        ),
    ] {
        let source = format!(
            r#"
pub trait CommandProvider {{ fn run(self) -> i64; }}
pub struct SortInventory {{}}
{attribute}
impl CommandProvider for SortInventory {{ pub fn run(self) -> i64 {{ return 1; }} }}
"#
        );
        assert_code(&graph(&source), code);
    }
}

#[test]
fn provider_rejects_method_signature_and_effect_mismatch() {
    let signature = graph(
        r#"
pub trait CommandProvider { fn run(self, value: i64) -> i64; }
pub struct SortInventory {}
#[provider(id = "sort_inventory")]
impl CommandProvider for SortInventory {
    pub fn run(self, value: String) -> i64 { return 1; }
}
"#,
    );
    assert_code(&signature, "hir::provider_method_contract_mismatch");

    let effect = graph(
        r#"
pub trait CommandProvider { #[effect("host_read")] fn run(self) -> i64; }
pub struct SortInventory {}
#[provider(id = "sort_inventory")]
impl CommandProvider for SortInventory {
    #[effect("host_write")]
    pub fn run(self) -> i64 { return 1; }
}
"#,
    );
    assert_code(&effect, "hir::provider_method_contract_mismatch");
}

#[test]
fn duplicate_provider_key_is_rejected() {
    let graph = graph(
        r#"
pub trait CommandProvider { fn run(self) -> i64; }
pub struct First {}
pub struct Second {}
#[provider(id = "sort_inventory")]
impl CommandProvider for First { pub fn run(self) -> i64 { return 1; } }
#[provider(id = "sort_inventory")]
impl CommandProvider for Second { pub fn run(self) -> i64 { return 2; } }
"#,
    );
    assert_code(&graph, "hir::duplicate_provider_key");
}

fn graph(source: &str) -> ModuleGraph {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(1),
        PackageId::new("dev.vela.provider").expect("package id"),
        ModulePath::from_qualified("service"),
        source,
    ));
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    graph
}

fn assert_code(graph: &ModuleGraph, expected: &str) {
    let error = discover_providers(graph).expect_err("provider discovery must fail");
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some(expected)),
        "missing diagnostic {expected}: {:?}",
        error.diagnostics()
    );
}
