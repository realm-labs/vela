use vela_common::SourceId;
use vela_def::{DefPath, FunctionId};
use vela_hir::body::HirExprKind;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::ModulePath;
use vela_registry::{DefinitionRegistry, FieldDef, TypeDef, VariantDef};

use crate::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use crate::facts::AnalysisFacts;
use crate::registry::RegistryFacts;
use crate::semantic_facts::{HostPathSegmentFact, MemberTargetFact};
use crate::type_fact::TypeFact;
use crate::validation::HostAccessUseKind;

#[test]
fn unique_host_variant_field_matches_definition_registry_fallback() {
    let registry = quest_registry(false);
    let schema = RegistryFacts::from_compile_view(registry.compile_view())
        .expect("registry declaration slots");
    let source = SourceId::new(91);
    let text = r#"
fn main(player: Player) {
    player.quest_progress.count += 1;
    return player.quest_progress.count;
}
"#;
    let (graph, main) = graph(source, text);
    let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
    let body = graph.function_body(main).expect("main body");
    let count = body
        .expressions
        .values()
        .find(|expression| {
            matches!(&expression.kind, HirExprKind::Field(field) if field.name == "count")
        })
        .expect("count field");
    let target = match facts.member_target(count.id) {
        Some(MemberTargetFact::HostField(target)) => target,
        other => panic!("unique variant host field target: {other:?}"),
    };
    assert!(target.variant_field);
    assert_eq!(target.name, "count");
    assert_eq!(facts.expression(count.id), Some(&TypeFact::I64));
    let path = facts.host_path_target(count.id).expect("host variant path");
    assert!(matches!(
        path.segments.as_slice(),
        [HostPathSegmentFact::Field(_), HostPathSegmentFact::Field(field)]
            if field.variant_field && field.semantic == target.semantic
    ));

    let function = FunctionId::new(91_001);
    let generation = ExecutableAnalysisGeneration::from_module_graph_and_schema(
        &graph,
        &schema,
        [ExecutableAnalysisInput::new(function, body.id)],
    )
    .expect("host variant executable analysis");
    let view = generation.view(function).expect("main view");
    let assignment = body
        .expressions
        .values()
        .find(|expression| matches!(expression.kind, HirExprKind::Assign { .. }))
        .expect("count mutation");
    assert_eq!(
        view.host_access_use(assignment.id).map(|fact| fact.kind),
        Some(HostAccessUseKind::Mutate)
    );
    // Host variant fields retain the direct compiler's writable exemption.
    assert_eq!(view.validation_diagnostics(), &[]);
}

#[test]
fn ambiguous_host_variant_field_remains_unresolved() {
    let registry = quest_registry(true);
    let schema = RegistryFacts::from_compile_view(registry.compile_view())
        .expect("registry declaration slots");
    assert!(
        schema
            .host_field_target_fact("QuestProgress", "count")
            .is_none()
    );
    assert!(schema.host_field_fact("QuestProgress", "count").is_none());

    let source = SourceId::new(92);
    let text = "fn main(progress: QuestProgress) { return progress.count; }";
    let (graph, main) = graph(source, text);
    let facts = AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
    let count = graph
        .function_body(main)
        .expect("main body")
        .expressions
        .values()
        .find(|expression| {
            matches!(&expression.kind, HirExprKind::Field(field) if field.name == "count")
        })
        .expect("count field");
    assert_eq!(facts.expression(count.id), None);
    assert_eq!(
        facts.member_target(count.id),
        Some(&MemberTargetFact::Unresolved)
    );
    assert!(facts.host_path_target(count.id).is_none());
}

fn quest_registry(ambiguous_count: bool) -> DefinitionRegistry {
    let mut registry = DefinitionRegistry::new();
    let player = registry
        .register_type(
            TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
                .host_runtime_id(91),
        )
        .expect("Player type");
    let progress = registry
        .register_type(
            TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "QuestProgress",
            ))
            .host_runtime_id(92),
        )
        .expect("QuestProgress type");
    registry
        .register_field(
            FieldDef::new(
                DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "Player",
                    "quest_progress",
                ),
                player,
            )
            .type_hint(Some("QuestProgress"))
            .host_runtime_id(93),
        )
        .expect("Player::quest_progress");
    let active = registry
        .register_variant(VariantDef::new(
            DefPath::variant(
                "host",
                std::iter::empty::<&str>(),
                "QuestProgress",
                "Active",
            ),
            progress,
        ))
        .expect("Active variant");
    registry
        .register_field(
            FieldDef::new(
                DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "QuestProgress::Active",
                    "count",
                ),
                progress,
            )
            .variant_owner(active)
            .type_hint(Some("i64"))
            .host_runtime_id(94),
        )
        .expect("Active::count");
    if ambiguous_count {
        let complete = registry
            .register_variant(VariantDef::new(
                DefPath::variant(
                    "host",
                    std::iter::empty::<&str>(),
                    "QuestProgress",
                    "Complete",
                ),
                progress,
            ))
            .expect("Complete variant");
        registry
            .register_field(
                FieldDef::new(
                    DefPath::field(
                        "host",
                        std::iter::empty::<&str>(),
                        "QuestProgress::Complete",
                        "count",
                    ),
                    progress,
                )
                .variant_owner(complete)
                .type_hint(Some("i64"))
                .host_runtime_id(95),
            )
            .expect("Complete::count");
    }
    registry
}

fn graph(source: SourceId, text: &str) -> (ModuleGraph, vela_hir::ids::HirDeclId) {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        source,
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("game"),
        text,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let main = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .map(|declaration| declaration.id)
        .expect("main declaration");
    (graph, main)
}
