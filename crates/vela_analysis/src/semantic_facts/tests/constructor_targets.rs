use std::collections::BTreeMap;
use vela_package::ModulePath;

use vela_common::SourceId;
use vela_def::FunctionId;
use vela_hir::body::{HirExprKind, HirPatternKind};
use vela_hir::module_graph::{ModuleGraph, ModuleSource};

use crate::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use crate::semantic_facts::ConstructorTargetFact;

#[test]
fn executable_constructor_targets_follow_imported_and_qualified_hir_resolutions() {
    let mut graph = ModuleGraph::new();
    let main = graph.add_source(ModuleSource::new(
        SourceId::new(91),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("game::main"),
        r#"
use game::schema::Reward as Prize
use game::schema::State as ImportedState

fn main() {
    let imported_record = Prize { amount: 1 };
    let qualified_record = game::schema::Reward { amount: 2 };
    let imported_variant = ImportedState::Ready { amount: 3 };
    let qualified_variant = game::schema::State::Ready { amount: 4 };
    let imported_unit_variant = ImportedState::Idle;
    let qualified_unit_variant = game::schema::State::Idle;
    let dynamic_record = Missing { amount: 5 };
    match imported_variant {
        ImportedState::Ready { amount } => { amount; },
        game::schema::State::Idle => {},
        Missing::Ready { amount } => { amount; },
    }
}
"#,
    ));
    graph.add_source(ModuleSource::new(
        SourceId::new(92),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("game::schema"),
        r#"
pub struct Reward { amount: i64 }
pub enum State { Ready { amount: i64 }, Idle }
"#,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let main_declaration = graph
        .module(main)
        .and_then(|declarations| declarations.get("main"))
        .expect("main declaration");
    let reward = declaration_named(&graph, "Reward");
    let state = declaration_named(&graph, "State");
    let body = graph
        .function_body(main_declaration)
        .expect("main function body");
    let function = FunctionId::new(9_101);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(function, body.id)],
    )
    .expect("executable analysis");
    let analysis = generation.view(function).expect("main analysis");

    let constructors = body
        .expressions
        .values()
        .filter(|expression| matches!(expression.kind, HirExprKind::Record { .. }))
        .map(|expression| {
            let path = constructor_path(body, expression.id);
            (path, analysis.constructor_target(expression.id).cloned())
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        constructors["Prize"],
        Some(ConstructorTargetFact::Declaration(reward))
    );
    assert_eq!(
        constructors["game::schema::Reward"],
        Some(ConstructorTargetFact::Declaration(reward))
    );
    assert_eq!(
        constructors["ImportedState::Ready"],
        Some(ConstructorTargetFact::Variant {
            enum_declaration: state,
            variant: "Ready".to_owned(),
        })
    );
    assert_eq!(
        constructors["game::schema::State::Ready"],
        Some(ConstructorTargetFact::Variant {
            enum_declaration: state,
            variant: "Ready".to_owned(),
        })
    );
    for path in ["ImportedState::Idle", "game::schema::State::Idle"] {
        let expression = body
            .expressions
            .values()
            .find(|expression| {
                body.paths.iter().any(|candidate| {
                    candidate.owner == vela_hir::body::HirPathOwner::Expression(expression.id)
                        && candidate.path.join("::") == path
                })
            })
            .unwrap_or_else(|| panic!("unit variant expression `{path}`"));
        assert_eq!(
            analysis.constructor_target(expression.id),
            Some(&ConstructorTargetFact::Variant {
                enum_declaration: state,
                variant: "Idle".to_owned(),
            })
        );
    }
    assert_eq!(
        constructors["Missing"],
        Some(ConstructorTargetFact::Dynamic)
    );

    let patterns = body
        .patterns
        .values()
        .filter_map(|pattern| match &pattern.kind {
            HirPatternKind::Path { path }
            | HirPatternKind::TupleVariant { path, .. }
            | HirPatternKind::RecordVariant { path, .. } => path.map(|path| {
                let path = body.paths.get(&path).expect("pattern path");
                (
                    path.path.join("::"),
                    analysis.pattern_constructor_target(pattern.id).cloned(),
                )
            }),
            HirPatternKind::Wildcard
            | HirPatternKind::Binding { .. }
            | HirPatternKind::Literal(_)
            | HirPatternKind::Missing => None,
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        patterns["ImportedState::Ready"],
        Some(ConstructorTargetFact::Variant {
            enum_declaration: state,
            variant: "Ready".to_owned(),
        })
    );
    assert_eq!(
        patterns["game::schema::State::Idle"],
        Some(ConstructorTargetFact::Variant {
            enum_declaration: state,
            variant: "Idle".to_owned(),
        })
    );
    assert_eq!(
        patterns["Missing::Ready"],
        Some(ConstructorTargetFact::Dynamic)
    );
}

fn declaration_named(graph: &ModuleGraph, name: &str) -> vela_hir::ids::HirDeclId {
    graph
        .declarations()
        .find(|declaration| declaration.name == name)
        .map(|declaration| declaration.id)
        .unwrap_or_else(|| panic!("missing declaration `{name}`"))
}

fn constructor_path(
    body: &vela_hir::body::HirBody,
    expression: vela_hir::ids::HirExprId,
) -> String {
    body.paths
        .iter()
        .find(|path| {
            path.kind == vela_hir::body::HirPathKind::Constructor
                && path.owner == vela_hir::body::HirPathOwner::Expression(expression)
        })
        .map(|path| path.path.join("::"))
        .expect("constructor path")
}
