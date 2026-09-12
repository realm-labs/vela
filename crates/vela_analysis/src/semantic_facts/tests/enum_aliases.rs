use crate::semantic_facts::{CallTargetFact, ConstructorTargetFact};
use crate::{facts::AnalysisFacts, registry::RegistryFacts, type_fact::TypeFact};
use vela_common::SourceId;
use vela_hir::{
    body::{HirExprKind, HirPathOwner},
    module_graph::{ModuleGraph, ModuleSource},
};
use vela_package::{ModulePath, PackageId};

#[test]
fn enum_alias_facts_keep_exact_source_and_registry_owners_and_closed_negatives() {
    for (imports, base, parameter, owner, source) in [
        (
            "use api::Choice as Alias;",
            "Alias",
            "",
            Some("api::Choice"),
            true,
        ),
        (
            "use api as Alias;",
            "Alias::Choice",
            "",
            Some("api::Choice"),
            true,
        ),
        (
            "use host::Choice as Alias;",
            "Alias",
            "",
            Some("host::Choice"),
            false,
        ),
        (
            "use host as Alias;",
            "Alias::Choice",
            "",
            Some("host::Choice"),
            false,
        ),
        ("use api::Hidden as Alias;", "Alias", "", None, false),
        ("use api as Alias;", "Alias::Mask", "", None, false),
        (
            "use api::Choice as Alias; use host::Choice as Alias;",
            "Alias",
            "",
            None,
            false,
        ),
        (
            "use api::Choice as Alias; use api::Choice as Alias;",
            "Alias",
            "",
            None,
            false,
        ),
        ("use missing::Choice as Alias;", "Alias", "", None, false),
        ("use api::Choice as Alias;", "Alias", "Alias", None, false),
    ] {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(ModuleSource::new(SourceId::new(301), PackageId::anonymous(),
            ModulePath::from_qualified("main"), format!("{imports} fn inspect({parameter}) {{ let idle = {base}::Idle; let pair = {base}::Pair(1); let data = {base}::Data {{ tag: 1 }}; }}")));
        graph.add_source(ModuleSource::new(SourceId::new(302), PackageId::anonymous(), ModulePath::from_qualified("api"),
            "pub enum Choice { Idle, Pair(value: i64), Data { tag: i64 } } enum Hidden { Idle, Pair(value: i64), Data { tag: i64 } } pub struct Mask {}"));
        graph.resolve_imports();
        let mut schema = RegistryFacts::default();
        for name in [
            "host::Choice",
            "api::Choice",
            "api::Hidden",
            "api::Mask",
            "Alias",
        ] {
            schema.insert_type(name, TypeFact::enum_type(name, None::<String>));
            for variant in ["Idle", "Pair", "Data"] {
                schema.insert_variant(name, variant, TypeFact::enum_type(name, Some(variant)));
            }
        }
        let declaration = graph
            .module(module)
            .expect("enum fixture invariant")
            .get("inspect")
            .expect("enum fixture invariant");
        let body = graph
            .function_body(declaration)
            .expect("enum fixture invariant");
        let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
        let path = body
            .paths
            .iter()
            .find(|path| path.path.last().is_some_and(|name| name == "Idle"))
            .expect("enum fixture invariant");
        let HirPathOwner::Expression(idle) = path.owner else {
            panic!("path")
        };
        let (pair, _) = body.calls().next().expect("enum fixture invariant");
        let data = body
            .expressions
            .values()
            .find(|expr| matches!(expr.kind, HirExprKind::Record { .. }))
            .expect("enum fixture invariant")
            .id;
        for (expression, variant) in [(idle, "Idle"), (pair, "Pair"), (data, "Data")] {
            let actual = facts
                .expression(expression)
                .filter(|fact| !matches!(fact, TypeFact::Unknown));
            assert_eq!(
                actual.cloned(),
                owner.map(|owner| TypeFact::enum_type(owner, Some(variant))),
                "{imports} {base} {parameter}: {variant}"
            );
            if source {
                let target = facts.script_type(expression).expect("source identity");
                assert_eq!(
                    graph
                        .qualified_declaration_name(target.declaration)
                        .as_deref(),
                    owner
                );
                assert_eq!(target.variant.as_deref(), Some(variant));
            }
        }
        if owner.is_some() {
            assert!(matches!(
                facts.call_target(pair),
                Some(CallTargetFact::Variant { .. } | CallTargetFact::RegistryVariant { .. })
            ));
            assert!(matches!(
                facts.constructor_target(idle),
                Some(
                    ConstructorTargetFact::Variant { .. }
                        | ConstructorTargetFact::RegistryVariant { .. }
                )
            ));
        }
    }
}
