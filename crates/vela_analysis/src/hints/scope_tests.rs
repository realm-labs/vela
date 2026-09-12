use super::*;
use vela_common::{SourceId, Span};
use vela_hir::module_graph::ModuleSource;
use vela_package::{ModulePath, PackageId};

#[test]
fn module_type_hints_preserve_local_import_and_visibility_identity() {
    let mut graph = ModuleGraph::new();
    for (index, module, text) in [
        (
            1,
            "alpha",
            "pub struct Cell {} pub struct OnlyForeign {} struct Hidden {}",
        ),
        (
            2,
            "beta",
            "use alpha::Cell as Imported; struct Cell {} fn main() {}",
        ),
    ] {
        graph.add_source(ModuleSource::new(
            SourceId::new(index),
            PackageId::anonymous(),
            ModulePath::from_qualified(module),
            text,
        ));
    }
    graph.resolve_imports();
    assert!(graph.diagnostics().is_empty());
    let module = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("requesting module")
        .module;
    for (path, expected) in [
        (vec!["Cell"], TypeFact::record("beta::Cell")),
        (vec!["Imported"], TypeFact::record("alpha::Cell")),
        (vec!["alpha", "Cell"], TypeFact::record("alpha::Cell")),
        (vec!["alpha", "Hidden"], TypeFact::Unknown),
        (vec!["OnlyForeign"], TypeFact::Unknown),
        (vec!["i64"], TypeFact::I64),
    ] {
        let hint = HirTypeHint {
            path: path.iter().map(|name| (*name).to_owned()).collect(),
            args: Vec::new(),
            span: Span::new(SourceId::new(2), 0, 0),
        };
        assert_eq!(
            type_fact_from_hint_in_module(&graph, module, &hint),
            expected,
            "{path:?}"
        );
        let nested = HirTypeHint {
            path: vec!["Option".into()],
            args: vec![hint],
            span: Span::new(SourceId::new(2), 0, 0),
        };
        assert_eq!(
            type_fact_from_hint_in_module(&graph, module, &nested),
            TypeFact::option(expected),
            "nested {path:?}"
        );
    }
}

#[test]
fn imported_type_hints_preserve_exact_source_registry_and_unavailable_owners() {
    for (imports, hint, expected) in [
        ("use api::Token as Alias;", "Alias", Some("api::Token")),
        ("use api as Alias;", "Alias::Token", Some("api::Token")),
        ("use host::Token as Alias;", "Alias", Some("host::Token")),
        ("use host as Alias;", "Alias::Token", Some("host::Token")),
        ("use api::Hidden as Alias;", "Alias", None),
        ("use absent::Token as Alias;", "Alias", None),
        ("", "absent::Token", None),
        (
            "use api::Token as Alias; use host::Token as Alias;",
            "Alias",
            None,
        ),
        ("use api as Alias; use host as Alias;", "Alias::Token", None),
        ("use api::Token as Alias; fn Alias() {}", "Alias", None),
    ] {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(12),
            PackageId::anonymous(),
            ModulePath::from_qualified("main"),
            format!("{imports} fn run(value: {hint}) {{ value.tag; }}"),
        ));
        graph.add_source(ModuleSource::new(
            SourceId::new(13),
            PackageId::anonymous(),
            ModulePath::from_qualified("api"),
            "pub struct Token { tag: i64 } struct Hidden { tag: i64 }",
        ));
        graph.resolve_imports();
        let mut schema = crate::registry::RegistryFacts::default();
        for name in ["host::Token", "api::Token", "api::Hidden", "Token", "Alias"] {
            schema.insert_type(name, TypeFact::record(name));
            schema.insert_field(name, "tag", TypeFact::BOOL);
        }
        let facts = crate::facts::AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
        let body = graph
            .bodies()
            .find(|body| {
                graph
                    .bindings_for_body(body.id)
                    .is_some_and(|bindings| bindings.locals().any(|local| local.name == "value"))
            })
            .expect("run body");
        let field = body
            .expressions
            .values()
            .find(|e| matches!(e.kind, vela_hir::body::HirExprKind::Field(_)))
            .expect("field");
        let expected_field = expected.map(|owner| {
            if owner == "api::Token" {
                TypeFact::I64
            } else {
                TypeFact::BOOL
            }
        });
        assert_eq!(
            facts.expression(field.id),
            expected_field.as_ref(),
            "{imports} {hint}"
        );
    }
}
