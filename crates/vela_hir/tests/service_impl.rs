use vela_common::SourceId;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_hir::script_methods::{ScriptMethodCatalog, ScriptMethodCatalogMode};
use vela_hir::source_ingestion::build_single_source;
use vela_package::{ModulePath, PackageId};

#[test]
fn rejects_removed_declarations_at_the_attribute_span() {
    for attribute in [
        "#[service_impl(app::Inventory)]",
        "#[service_impl(service = app::Inventory)]",
        "#[service_impl]",
        "#[provider(id = \"inventory\")]",
        "#[provider]",
        "#[provider(id = \"one\")] #[provider(id = \"two\")]",
    ] {
        for declaration in [
            "impl Patch { fn grant(value) { return value; } }",
            "impl Grant for Patch { fn grant(self, value) { return value; } }",
            "fn main() { return 1; }",
        ] {
            let source = format!("{attribute}\n{declaration}");
            let error = build_single_source(SourceId::new(1), &source)
                .expect_err("removed declarations must not become ordinary declarations");
            let diagnostic = error
                .diagnostics()
                .iter()
                .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::removed_declaration"))
                .expect("deliberate removal diagnostic");
            let span = diagnostic.span.expect("attribute source span");
            assert_eq!(span.source, SourceId::new(1));
            assert!(source[span.start as usize..span.end as usize].starts_with("#["));
            assert!(diagnostic.message.contains("no longer supported"));
        }
    }
}

#[test]
fn removed_impls_never_enter_the_ordinary_method_catalog() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(2),
        PackageId::anonymous(),
        ModulePath::root(),
        r#"
struct Patch {}
trait Grant { fn grant(self); }
#[service_impl(app::Inventory)]
impl Patch { fn grant(self) {} }
#[provider(id = "inventory")]
impl Grant for Patch { fn grant(self) {} }
"#,
    ));
    assert!(!graph.diagnostics().is_empty());
    let catalog = ScriptMethodCatalog::from_graph(&graph, ScriptMethodCatalogMode::ModuleGraph)
        .expect("rejected declarations cannot register ordinary methods");
    assert!(catalog.is_empty());
}

#[test]
fn rejects_removed_service_paths_in_calls_values_and_nested_functions() {
    for expression in [
        "service::base::grant(value)",
        "service::pinned::audit::record(value)",
        "service::pinned::missing::record(value)",
        "service::base::grant",
        "service::base",
        "service::pinned",
        "service::base::grant::extra(value)",
        "(|| service::base::grant(value))()",
        "(|| service::pinned::audit::record(value))()",
    ] {
        let source = format!("fn main(value) {{ return {expression}; }}");
        let error = build_single_source(SourceId::new(3), &source)
            .expect_err("removed paths cannot dispatch, escape, or be captured");
        let diagnostic = error
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::removed_service_call"))
            .expect("deliberate removed-path diagnostic");
        let span = diagnostic.span.expect("path source span");
        assert_eq!(span.source, SourceId::new(3));
        assert!(source[span.start as usize..span.end as usize].starts_with("service::"));
        assert!(diagnostic.message.contains("no longer supported"));
    }
}

#[test]
fn ordinary_traits_methods_and_base_services_names_remain_available() {
    let sources = build_single_source(
        SourceId::new(4),
        include_str!("fixtures/methods/ordinary_inventory.vela"),
    )
    .expect("ordinary traits and local names are retained");
    let catalog = ScriptMethodCatalog::from_graph(
        sources.graph(),
        ScriptMethodCatalogMode::single_source(sources.modules()[0], "test"),
    )
    .expect("ordinary method catalog");
    assert_eq!(catalog.len(), 2);
}
