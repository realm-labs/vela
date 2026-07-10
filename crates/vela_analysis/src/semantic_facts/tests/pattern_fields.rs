use vela_common::SourceId;
use vela_def::{FunctionId, script_trait_method_id};
use vela_hir::body::HirExprKind;
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};
use vela_hir::script_methods::{ScriptMethodCatalog, ScriptMethodCatalogMode};

use crate::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use crate::semantic_facts::{CallTargetFact, ConstructorTargetFact, ScriptTypeTargetFact};
use crate::type_fact::TypeFact;

#[test]
fn executable_variant_pattern_fields_preserve_script_method_identity() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(95),
        ModulePath::from_qualified("main"),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }
enum Event {
    Record { record_player: Player },
    Tuple(tuple_player: Player),
    None,
}
impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}
fn main() {
    let record_event = Event::Record { record_player: Player { level: 7 } };
    let record_bonus = match record_event {
        Event::Record { record_player } => record_player.bonus(5),
        _ => 0,
    };
    let tuple_event = Event::Tuple(Player { level: 8 });
    let tuple_bonus = match tuple_event {
        Event::Tuple(tuple_player) => tuple_player.bonus(6),
        _ => 0,
    };
    return record_bonus + tuple_bonus;
}
"#,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let main = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let player = graph
        .declarations()
        .find(|declaration| declaration.name == "Player")
        .expect("Player declaration");
    let event = graph
        .declarations()
        .find(|declaration| declaration.name == "Event")
        .expect("Event declaration");
    let body = graph.function_body(main.id).expect("main body");
    let bindings = graph.bindings_for_body(body.id).expect("main bindings");
    let [record_player] = bindings.locals_named("record_player") else {
        panic!("record pattern local");
    };
    let [tuple_player] = bindings.locals_named("tuple_player") else {
        panic!("tuple pattern local");
    };

    let function = FunctionId::new(9_701);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(function, body.id)],
    )
    .expect("pattern field analysis");
    let view = generation.view(function).expect("main view");
    let player_fact = TypeFact::record("main::Player");
    let player_target = ScriptTypeTargetFact::declaration(player.id);

    let tuple_pattern = body
        .patterns
        .values()
        .find(|pattern| {
            matches!(
                pattern.kind,
                vela_hir::body::HirPatternKind::TupleVariant { .. }
            )
        })
        .expect("tuple variant pattern");
    assert_eq!(
        view.pattern_constructor_target(tuple_pattern.id),
        Some(&ConstructorTargetFact::Variant {
            enum_declaration: event.id,
            variant: "Tuple".to_owned(),
        })
    );
    let direct_tuple = crate::semantic_facts::patterns::pattern_local_facts(
        &graph,
        None,
        body,
        tuple_pattern.id,
        &TypeFact::Unknown,
        None,
    );
    assert_eq!(direct_tuple.len(), 1);
    assert_eq!(direct_tuple[0].local, *tuple_player);
    assert_eq!(direct_tuple[0].fact, player_fact);
    assert_eq!(direct_tuple[0].script_type, Some(player_target.clone()));

    assert_eq!(
        view.local(*record_player),
        Some(&player_fact),
        "record field"
    );
    assert_eq!(
        view.local_script_type(*record_player),
        Some(&player_target),
        "record field target"
    );
    assert_eq!(view.local(*tuple_player), Some(&player_fact), "tuple field");
    assert_eq!(
        view.local_script_type(*tuple_player),
        Some(&player_target),
        "tuple field target"
    );

    let mut calls = body
        .expressions
        .values()
        .filter_map(|expression| {
            let HirExprKind::Call(call) = &expression.kind else {
                return None;
            };
            let field = body.field(call.callee)?;
            (field.name == "bonus").then_some((expression.id, field.receiver))
        })
        .collect::<Vec<_>>();
    calls.sort_by_key(|(call, _)| body.expression(*call).map(|call| call.origin.span.start));
    assert_eq!(calls.len(), 2);

    let catalog = ScriptMethodCatalog::from_graph(&graph, ScriptMethodCatalogMode::ModuleGraph)
        .expect("script method catalog");
    let expected_method = script_trait_method_id("main::BonusSource", "bonus");
    for (call, receiver) in calls {
        assert_eq!(view.expression(receiver), Some(&player_fact));
        assert_eq!(view.script_type(receiver), Some(&player_target));
        let Some(CallTargetFact::ScriptMethod { method }) = view.call_target(call) else {
            panic!("stable script method target for {call:?}");
        };
        let catalog_method = catalog
            .methods()
            .find(|catalog_method| catalog_method.node() == *method)
            .expect("target method in HIR catalog");
        assert_eq!(catalog_method.method_id(), expected_method);
    }
}
