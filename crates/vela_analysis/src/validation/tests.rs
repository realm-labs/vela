use vela_common::{SourceId, Span};
use vela_def::FunctionId;
use vela_hir::body::{HirBinaryOp, HirBody, HirExprKind, HirStmtKind};
use vela_hir::ids::{HirDeclId, HirExprId};
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::ModulePath;

use crate::executable::{
    ExecutableAnalysisGeneration, ExecutableAnalysisInput, ExecutableReceiverInput,
};
use crate::semantic_facts::ScriptTypeTargetFact;
use crate::type_fact::TypeFact;

use super::{
    ArrayOrderingValueKind, BuiltinOperatorTrait, CapabilityFact, LoopControlKind,
    LoopControlPlacement, OperatorCapabilityFact,
};

#[test]
fn identity_validation_distinguishes_static_values_from_dynamic_references() {
    let source = SourceId::new(101);
    let text = r#"
struct Token { value: i64 }
enum State { Ready }

fn main(dynamic) {
    let token = Token { value: 1 };
    token === token;
    State::Ready() === State::Ready();
    1 === 1;
    (1..2) === (1..2);
    dynamic === dynamic;
    return [1] === [1];
}
"#;
    let (graph, main) = graph(source, text);
    let body = graph.function_body(main).expect("main body");
    let function = FunctionId::new(10_101);
    let generation = generation(&graph, main, function);
    let view = generation.view(function).expect("main view");

    let record = expression_exact(&graph, source, text, "token === token");
    let enum_value = expression_exact(&graph, source, text, "State::Ready() === State::Ready()");
    let scalar = expression_exact(&graph, source, text, "1 === 1");
    let range = expression_exact(&graph, source, text, "(1..2) === (1..2)");
    let dynamic = expression_exact(&graph, source, text, "dynamic === dynamic");
    let array = expression_exact(&graph, source, text, "[1] === [1]");

    assert_identity_state(body, &view, record, CapabilityFact::is_supported);
    assert_identity_state(body, &view, enum_value, CapabilityFact::is_supported);
    assert_identity_state(body, &view, scalar, |fact| {
        fact == &CapabilityFact::Unsupported {
            type_name: "i64".to_owned(),
        }
    });
    assert_identity_state(body, &view, range, |fact| {
        fact == &CapabilityFact::Unsupported {
            type_name: "Range".to_owned(),
        }
    });
    assert_identity_state(body, &view, dynamic, CapabilityFact::is_dynamic);
    assert_identity_state(body, &view, array, CapabilityFact::is_supported);

    let diagnostics = view.validation_diagnostics();
    assert_eq!(
        diagnostic_codes(diagnostics),
        [
            "compiler::invalid_identity_comparison",
            "compiler::invalid_identity_comparison"
        ]
    );
    let scalar_diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("type `i64`"))
        .expect("scalar identity diagnostic");
    assert_eq!(scalar_diagnostic.span, graph.expression_span(scalar));
    assert_eq!(scalar_diagnostic.labels.len(), 2);
}

#[test]
fn comparison_validation_uses_authoritative_derives_and_impl_metadata() {
    let source = SourceId::new(102);
    let text = r#"
struct Plain { value: i64 }

#[derive(PartialEq)]
struct DerivedEq { value: i64 }

#[derive(PartialEq, PartialOrd)]
struct DerivedOrd { value: i64 }

struct ExplicitEq { value: i64 }
impl PartialEq for ExplicitEq {
    fn eq(self, other: ExplicitEq) -> bool { return self.value == other.value; }
}

fn main() {
    Plain { value: 1 } == Plain { value: 2 };
    DerivedEq { value: 1 } == DerivedEq { value: 2 };
    ExplicitEq { value: 1 } == ExplicitEq { value: 2 };
    Plain { value: 1 } < Plain { value: 2 };
    return DerivedOrd { value: 1 } < DerivedOrd { value: 2 };
}
"#;
    let (graph, main) = graph(source, text);
    let function = FunctionId::new(10_201);
    let generation = generation(&graph, main, function);
    let view = generation.view(function).expect("main view");

    let plain_eq = expression_exact(
        &graph,
        source,
        text,
        "Plain { value: 1 } == Plain { value: 2 }",
    );
    let derived_eq = expression_exact(
        &graph,
        source,
        text,
        "DerivedEq { value: 1 } == DerivedEq { value: 2 }",
    );
    let explicit_eq = expression_exact(
        &graph,
        source,
        text,
        "ExplicitEq { value: 1 } == ExplicitEq { value: 2 }",
    );
    let plain_ord = expression_exact(
        &graph,
        source,
        text,
        "Plain { value: 1 } < Plain { value: 2 }",
    );
    let derived_ord = expression_exact(
        &graph,
        source,
        text,
        "DerivedOrd { value: 1 } < DerivedOrd { value: 2 }",
    );

    assert_comparison(&view, plain_eq, BuiltinOperatorTrait::PartialEq, false);
    assert_comparison(&view, derived_eq, BuiltinOperatorTrait::PartialEq, true);
    assert_comparison(&view, explicit_eq, BuiltinOperatorTrait::PartialEq, true);
    assert_comparison(&view, plain_ord, BuiltinOperatorTrait::PartialOrd, false);
    assert_comparison(&view, derived_ord, BuiltinOperatorTrait::PartialOrd, true);
    assert_eq!(
        diagnostic_codes(view.validation_diagnostics()),
        [
            "compiler::missing_comparison_trait",
            "compiler::missing_comparison_trait"
        ]
    );
}

#[test]
fn array_ordering_validation_covers_proven_elements_and_callback_keys() {
    let source = SourceId::new(103);
    let text = r#"
struct Plain { value: i64 }

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Ranked { value: i64 }

fn main(dynamic_callback) {
    let plain = [Plain { value: 2 }, Plain { value: 1 }];
    let ranked = [Ranked { value: 2 }, Ranked { value: 1 }];
    let floats: Array<f64> = [2.0, 1.0];
    plain.sort();
    ranked.max();
    floats.min();
    plain.sort_by(|item| 1.0);
    plain.sort_by(|item| item);
    return plain.sort_by(dynamic_callback);
}
"#;
    let (graph, main) = graph(source, text);
    let function = FunctionId::new(10_301);
    let generation = generation(&graph, main, function);
    let view = generation.view(function).expect("main view");

    assert_array_ordering(
        &graph,
        source,
        text,
        &view,
        "plain.sort()",
        false,
        "element",
    );
    assert_array_ordering(&graph, source, text, &view, "ranked.max()", true, "element");
    assert_array_ordering(
        &graph,
        source,
        text,
        &view,
        "floats.min()",
        false,
        "element",
    );
    assert_array_ordering(
        &graph,
        source,
        text,
        &view,
        "plain.sort_by(|item| 1.0)",
        false,
        "key",
    );
    assert_array_ordering(
        &graph,
        source,
        text,
        &view,
        "plain.sort_by(|item| item)",
        false,
        "key",
    );
    let dynamic = expression_exact(&graph, source, text, "plain.sort_by(dynamic_callback)");
    assert!(
        view.array_ordering_capability(dynamic)
            .is_some_and(|fact| fact.capability.is_dynamic())
    );

    assert_eq!(
        diagnostic_codes(view.validation_diagnostics()),
        [
            "compiler::missing_ord_for_array_ordering",
            "compiler::missing_ord_for_array_ordering",
            "compiler::missing_ord_for_array_ordering",
            "compiler::missing_ord_for_array_ordering"
        ]
    );
}

#[test]
fn loop_control_validation_stays_within_each_hir_body() {
    let source = SourceId::new(104);
    let text = r#"
fn main() {
    break;
    continue;
    for value in [1] {
        if value == 1 { break; }
        { continue; }
        let nested = || {
            break;
            continue;
        };
        nested();
    }
}
"#;
    let (graph, main) = graph(source, text);
    let function = FunctionId::new(10_401);
    let generation = generation(&graph, main, function);
    let view = generation.view(function).expect("main view");
    let mut placements = Vec::new();
    for body in graph.bodies() {
        for statement in body.statements.values() {
            if matches!(statement.kind, HirStmtKind::Break | HirStmtKind::Continue) {
                placements.push(view.loop_control(statement.id).expect("loop control fact"));
            }
        }
    }

    assert_eq!(
        placements
            .iter()
            .filter(|fact| {
                fact.kind == LoopControlKind::Break
                    && fact.placement == LoopControlPlacement::OutsideLoop
            })
            .count(),
        2
    );
    assert_eq!(
        placements
            .iter()
            .filter(|fact| {
                fact.kind == LoopControlKind::Continue
                    && fact.placement == LoopControlPlacement::OutsideLoop
            })
            .count(),
        2
    );
    assert_eq!(
        placements
            .iter()
            .filter(|fact| fact.placement == LoopControlPlacement::InsideLoop)
            .count(),
        2
    );
    assert_eq!(
        diagnostic_codes(view.validation_diagnostics()),
        [
            "analysis::break_outside_loop",
            "analysis::continue_outside_loop",
            "analysis::break_outside_loop",
            "analysis::continue_outside_loop"
        ]
    );
    assert!(view.validation_diagnostics().iter().all(|diagnostic| {
        diagnostic
            .span
            .is_some_and(|span| span.source == source && !span.is_empty())
    }));
}

#[test]
fn shared_body_operator_validation_is_qualified_by_function_id() {
    let source = SourceId::new(105);
    let text = r#"
trait Probe {
    fn same(self) { self.value === self.value; }
}

struct ReferenceOwner { value: Array<i64> }
struct ScalarOwner { value: i64 }

impl Probe for ReferenceOwner {}
impl Probe for ScalarOwner {}
"#;
    let (graph, _) = graph(source, text);
    let probe = declaration_named(&graph, "Probe");
    let reference_owner = declaration_named(&graph, "ReferenceOwner");
    let scalar_owner = declaration_named(&graph, "ScalarOwner");
    let method = graph
        .trait_shape(probe)
        .expect("Probe trait")
        .methods
        .iter()
        .find(|method| method.name == "same")
        .expect("same method");
    let body = graph
        .trait_default_method_body(method.default_body_node.expect("default method body"))
        .expect("same body");
    let comparison = body
        .expressions
        .values()
        .find(|expression| {
            matches!(
                expression.kind,
                HirExprKind::Binary {
                    op: Some(HirBinaryOp::IdentityEqual),
                    ..
                }
            )
        })
        .expect("same comparison")
        .id;
    let reference_function = FunctionId::new(10_501);
    let scalar_function = FunctionId::new(10_502);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [
            ExecutableAnalysisInput::new(reference_function, body.id).with_receiver(
                ExecutableReceiverInput::new(TypeFact::record("game::ReferenceOwner"))
                    .with_script_type(ScriptTypeTargetFact::declaration(reference_owner)),
            ),
            ExecutableAnalysisInput::new(scalar_function, body.id).with_receiver(
                ExecutableReceiverInput::new(TypeFact::record("game::ScalarOwner"))
                    .with_script_type(ScriptTypeTargetFact::declaration(scalar_owner)),
            ),
        ],
    )
    .expect("function-qualified validation");
    let reference = generation.view(reference_function).expect("reference view");
    let scalar = generation.view(scalar_function).expect("scalar view");

    assert_identity_state(body, &reference, comparison, CapabilityFact::is_supported);
    assert_identity_state(
        body,
        &scalar,
        comparison,
        |fact| matches!(fact, CapabilityFact::Unsupported { type_name } if type_name == "i64"),
    );
    assert!(reference.validation_diagnostics().is_empty());
    assert_eq!(
        diagnostic_codes(scalar.validation_diagnostics()),
        ["compiler::invalid_identity_comparison"]
    );
}

fn assert_identity_state(
    _body: &HirBody,
    view: &crate::executable::ExecutableAnalysisView<'_>,
    expression: HirExprId,
    predicate: impl FnOnce(&CapabilityFact) -> bool,
) {
    let Some(OperatorCapabilityFact::ReferenceIdentity { lhs, rhs, .. }) =
        view.operator_capability(expression)
    else {
        panic!("reference identity capability fact");
    };
    assert!(predicate(lhs));
    assert_eq!(lhs, rhs);
}

fn assert_comparison(
    view: &crate::executable::ExecutableAnalysisView<'_>,
    expression: HirExprId,
    expected_trait: BuiltinOperatorTrait,
    supported: bool,
) {
    let Some(OperatorCapabilityFact::ComparisonTrait {
        required,
        capability,
        ..
    }) = view.operator_capability(expression)
    else {
        panic!("comparison capability fact");
    };
    assert_eq!(*required, expected_trait);
    assert_eq!(capability.is_supported(), supported);
    assert_eq!(
        matches!(capability, CapabilityFact::Unsupported { .. }),
        !supported
    );
}

fn assert_array_ordering(
    graph: &ModuleGraph,
    source: SourceId,
    text: &str,
    view: &crate::executable::ExecutableAnalysisView<'_>,
    expression: &str,
    supported: bool,
    value_kind: &str,
) {
    let expression = expression_exact(graph, source, text, expression);
    let fact = view
        .array_ordering_capability(expression)
        .expect("Array ordering capability");
    assert_eq!(fact.capability.is_supported(), supported);
    assert_eq!(
        fact.value_kind,
        if value_kind == "element" {
            ArrayOrderingValueKind::Element
        } else {
            ArrayOrderingValueKind::Key
        }
    );
}

fn graph(source: SourceId, text: &str) -> (ModuleGraph, HirDeclId) {
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
        .unwrap_or(HirDeclId::new(u32::MAX));
    (graph, main)
}

fn generation(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    function: FunctionId,
) -> ExecutableAnalysisGeneration {
    let body = graph.function_body(declaration).expect("function body");
    ExecutableAnalysisGeneration::from_module_graph(
        graph,
        [ExecutableAnalysisInput::new(function, body.id)],
    )
    .expect("executable validation facts")
}

fn declaration_named(graph: &ModuleGraph, name: &str) -> HirDeclId {
    graph
        .declarations()
        .find(|declaration| declaration.name == name)
        .map(|declaration| declaration.id)
        .unwrap_or_else(|| panic!("{name} declaration"))
}

fn expression_exact(
    graph: &ModuleGraph,
    source: SourceId,
    text: &str,
    expression: &str,
) -> HirExprId {
    let start = text.find(expression).expect("expression source offset");
    graph
        .expression_at_span(Span::new(
            source,
            u32::try_from(start).expect("source start"),
            u32::try_from(start + expression.len()).expect("source end"),
        ))
        .expect("HIR expression at exact span")
}

fn diagnostic_codes(diagnostics: &[vela_common::Diagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.code.as_deref())
        .collect()
}
