use crate::{
    facts::AnalysisFacts,
    registry::{RegistryEffectFact, RegistryFacts},
    semantic_facts::CallTargetFact,
    type_fact::TypeFact,
};
use vela_common::SourceId;
use vela_hir::{
    body::HirExprKind,
    module_graph::{ModuleGraph, ModuleSource},
};
use vela_package::{ModulePath, PackageId};

#[test]
fn external_function_aliases_preserve_returns_effects_and_ownership_boundaries() {
    let host = TypeFact::host("HostCell");
    for (imports, parameters, callee, expected, target) in [
        ("", "", "bridge::make", Some(host.clone()), "registry"),
        (
            "use bridge as api;",
            "",
            "api::make",
            Some(host.clone()),
            "registry",
        ),
        (
            "use bridge::make;",
            "",
            "make",
            Some(host.clone()),
            "registry",
        ),
        (
            "use bridge::make as create;",
            "",
            "create",
            Some(host.clone()),
            "registry",
        ),
        (
            "use bridge as api;",
            "",
            "api::native",
            Some(host.clone()),
            "native",
        ),
        (
            "use math as calc;",
            "",
            "calc::abs",
            Some(TypeFact::I64),
            "stdlib",
        ),
        (
            "use math::abs as positive;",
            "",
            "positive",
            Some(TypeFact::I64),
            "stdlib",
        ),
        (
            "use bridge as api; use other as api;",
            "",
            "api::make",
            None,
            "unresolved",
        ),
        (
            "use bridge::make as create; use other::make as create;",
            "",
            "create",
            None,
            "unresolved",
        ),
        ("use absent as api;", "", "api::make", None, "unresolved"),
        (
            "use bridge::make as create;",
            "create",
            "create",
            None,
            "local",
        ),
        (
            "use bridge::make as create;",
            "create: Any",
            "create",
            Some(TypeFact::Any),
            "local",
        ),
        (
            "use defs as api;",
            "",
            "api::make",
            Some(TypeFact::I64),
            "source",
        ),
        ("use defs as api;", "", "api::hidden", None, "unresolved"),
        ("use defs as api;", "", "api::occupied", None, "unresolved"),
        (
            "use defs as api;",
            "",
            "api::Blocked::ghost",
            None,
            "unresolved",
        ),
        (
            "use bridge as api; fn api() -> i64 { return 1; }",
            "",
            "api",
            Some(TypeFact::I64),
            "source",
        ),
        (
            "use bridge as api; fn api() -> i64 { return 1; }",
            "",
            "api::make",
            None,
            "unresolved",
        ),
        (
            "use task as jobs;",
            "",
            "jobs::spawn_scoped",
            None,
            "unresolved",
        ),
        (
            "use task::spawn_scoped as spawn;",
            "",
            "spawn",
            None,
            "unresolved",
        ),
        (
            "use bridge as task;",
            "",
            "task::spawn_scoped",
            Some(TypeFact::UNIT),
            "stdlib",
        ),
    ] {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(251),
            PackageId::anonymous(),
            ModulePath::from_qualified("main"),
            format!("{imports} fn run({parameters}) {{ let result = {callee}(1); result.value; }}"),
        ));
        graph.add_source(ModuleSource::new(SourceId::new(252), PackageId::anonymous(),
            ModulePath::from_qualified("defs"), "pub fn make() -> i64 { return 1; } fn hidden() -> String { return \"private\"; } pub const occupied: i64 = 1; pub struct Blocked { value: i64 }"));
        graph.resolve_imports();
        let mut schema = RegistryFacts::default();
        schema.insert_type("HostCell", host.clone());
        schema.insert_field("HostCell", "value", TypeFact::BOOL);
        for name in [
            "bridge::make",
            "bridge::native",
            "other::make",
            "api::make",
            "create",
            "defs::make",
            "defs::hidden",
            "defs::occupied",
            "defs::Blocked::ghost",
        ] {
            schema.insert_function(name, TypeFact::function(vec![], host.clone()));
            schema.insert_function_effect(name, RegistryEffectFact::host_read());
        }
        schema.insert_function_origin("bridge::native", vela_reflect::modules::DeclOrigin::Host);
        let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
        let body = graph
            .bodies()
            .find(|body| {
                body.expressions
                    .values()
                    .any(|e| matches!(e.kind, HirExprKind::Call(_)))
            })
            .expect("run body");
        let call = body
            .expressions
            .values()
            .find(|e| matches!(e.kind, HirExprKind::Call(_)))
            .expect("call");
        let field = body
            .expressions
            .values()
            .find(|e| matches!(e.kind, HirExprKind::Field(_)))
            .expect("field");
        assert_eq!(
            facts.expression(call.id),
            expected.as_ref(),
            "{imports} {parameters} {callee}"
        );
        assert_eq!(
            facts.expression(field.id),
            match expected {
                Some(TypeFact::Host { .. }) => Some(&TypeFact::BOOL),
                Some(TypeFact::Any) => Some(&TypeFact::Any),
                _ => None,
            },
            "{imports} {parameters} member"
        );
        let actual = facts.call_target(call.id).expect("call target");
        assert!(
            matches!(
                (target, actual),
                ("registry", CallTargetFact::RegistryFunction { .. })
                    | ("native", CallTargetFact::NativeFunction { .. })
                    | ("stdlib", CallTargetFact::StdlibFunction { .. })
                    | ("source", CallTargetFact::Declaration(_))
                    | ("local", CallTargetFact::Local(_))
                    | ("unresolved", CallTargetFact::Unresolved)
            ),
            "{imports} {parameters}: {actual:?}"
        );
        if let CallTargetFact::RegistryFunction { path } | CallTargetFact::NativeFunction { path } =
            actual
        {
            assert_eq!(
                path,
                if callee.ends_with("native") {
                    "bridge::native"
                } else {
                    "bridge::make"
                }
            );
            assert_eq!(
                facts.effect(call.id),
                Some(&RegistryEffectFact::host_read())
            );
        }
    }
}

#[test]
fn aliased_schema_callbacks_seed_current_facts_without_guessing_missing_or_any_members() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(253),
        PackageId::anonymous(),
        ModulePath::from_qualified("main"),
        "use bridge as api; fn run() { api::walk(|value| value.value); }",
    ));
    graph.resolve_imports();
    for parameter in [
        Some(TypeFact::host("HostCell")),
        Some(TypeFact::Any),
        None,
        Some(TypeFact::host("HostCell")),
    ] {
        let mut schema = RegistryFacts::default();
        schema.insert_type("HostCell", TypeFact::host("HostCell"));
        schema.insert_field("HostCell", "value", TypeFact::BOOL);
        if let Some(parameter) = &parameter {
            schema.insert_function(
                "bridge::walk",
                TypeFact::function(
                    vec![TypeFact::function(vec![parameter.clone()], TypeFact::UNIT)],
                    TypeFact::UNIT,
                ),
            );
        }
        let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
        let (body, access) = graph
            .bodies()
            .find_map(|body| {
                body.expressions
                    .values()
                    .find(|e| matches!(e.kind, HirExprKind::Field(_)))
                    .map(|access| (body, access))
            })
            .expect("lambda field");
        let field = body.field(access.id).expect("field");
        assert_eq!(
            facts.expression(field.receiver),
            parameter.as_ref(),
            "current callback parameter {parameter:?}"
        );
        assert_eq!(
            facts.expression(access.id),
            match parameter {
                Some(TypeFact::Host { .. }) => Some(&TypeFact::BOOL),
                Some(TypeFact::Any) => Some(&TypeFact::Any),
                _ => None,
            }
        );
    }
}
