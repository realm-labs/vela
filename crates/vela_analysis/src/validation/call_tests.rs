use vela_common::{SourceId, Span};
use vela_def::FunctionId;
use vela_hir::body::HirExprKind;
use vela_hir::ids::{HirDeclId, HirExprId};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use super::{CallParameterSlotValueFact, CallPlacementModeFact, CallSourceArgumentFact};
use crate::callable::{
    CallableParameterFact, CallableParameterRequirementFact, CallableSignatureFact,
};
use crate::executable::{
    ExecutableAnalysisGeneration, ExecutableAnalysisInput, ExecutableReceiverInput,
};
use crate::registry::RegistryFacts;
use crate::semantic_facts::ScriptTypeTargetFact;
use crate::type_fact::TypeFact;

#[test]
fn call_placement_diagnostics_freeze_codes_spans_and_labels() {
    let source = SourceId::new(106);
    let text = r#"
fn grant(base: i64, amount: i64 = 10) {}

fn main() {
    grant(base = 1, amunt = 2);
    grant(base = 1, 2);
    grant(1, 2, 3);
    grant(1, base = 2);
    grant();
}
"#;
    let (graph, main) = graph(source, text);
    let function = FunctionId::new(10_601);
    let generation = generation(&graph, None, main, function);
    let view = generation.view(function).expect("main view");
    let diagnostics = view.validation_diagnostics();

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_deref().expect("diagnostic code"))
            .collect::<Vec<_>>(),
        [
            "compiler::unknown_named_argument",
            "compiler::positional_after_named_argument",
            "compiler::too_many_arguments",
            "compiler::duplicate_argument",
            "compiler::missing_required_argument",
        ]
    );
    assert_eq!(
        span_text(text, diagnostics[0].span.expect("unknown span")),
        "amunt = 2"
    );
    assert_eq!(
        diagnostics[0]
            .labels
            .iter()
            .map(|label| label.message.as_str())
            .collect::<Vec<_>>(),
        [
            "argument name does not match any parameter",
            "available parameters: amount, base",
        ]
    );
    assert_eq!(
        span_text(text, diagnostics[1].span.expect("positional span")),
        "2"
    );
    assert_eq!(
        span_text(text, diagnostics[2].span.expect("extra span")),
        "3"
    );
    assert_eq!(
        diagnostics[2].labels[0].message,
        "call accepts 2 positional argument(s)"
    );
    assert_eq!(
        span_text(text, diagnostics[3].span.expect("duplicate span")),
        "base = 2"
    );
    assert_eq!(span_text(text, diagnostics[3].labels[0].span), "1");
    assert_eq!(span_text(text, diagnostics[3].labels[1].span), "base = 2");
    assert_eq!(
        span_text(text, diagnostics[4].span.expect("missing span")),
        "grant()"
    );
    let grant = declaration_named(&graph, "grant");
    let base_span = graph
        .function_signature(grant)
        .expect("grant signature")
        .params[0]
        .span;
    assert_eq!(diagnostics[4].labels.len(), 2);
    assert_eq!(diagnostics[4].labels[1].span, base_span);
}

#[test]
fn shared_body_placements_keep_source_order_separate_from_receiver_specific_slots() {
    let source = SourceId::new(107);
    let text = r#"
trait Probe {
    fn route(self) { self.target(second = 2, first = 1); }
}

struct Forward {}
struct Reverse {}

impl Forward {
    fn target(self, first: i64, second: i64, third: i64 = 3) {}
}
impl Reverse {
    fn target(self, second: i64, first: i64, third: i64 = 3) {}
}
impl Probe for Forward {}
impl Probe for Reverse {}
"#;
    let (graph, _) = graph(source, text);
    let probe = declaration_named(&graph, "Probe");
    let forward = declaration_named(&graph, "Forward");
    let reverse = declaration_named(&graph, "Reverse");
    let route = graph
        .trait_shape(probe)
        .expect("Probe shape")
        .methods
        .iter()
        .find(|method| method.name == "route")
        .expect("route method");
    let body = graph
        .trait_default_method_body(route.default_body_node.expect("route body node"))
        .expect("route body");
    let call = body
        .expressions
        .values()
        .find(|expression| matches!(expression.kind, HirExprKind::Call(_)))
        .expect("target call")
        .id;
    let forward_function = FunctionId::new(10_701);
    let reverse_function = FunctionId::new(10_702);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [
            ExecutableAnalysisInput::new(forward_function, body.id).with_receiver(
                ExecutableReceiverInput::new(TypeFact::record("game::Forward"))
                    .with_script_type(ScriptTypeTargetFact::declaration(forward)),
            ),
            ExecutableAnalysisInput::new(reverse_function, body.id).with_receiver(
                ExecutableReceiverInput::new(TypeFact::record("game::Reverse"))
                    .with_script_type(ScriptTypeTargetFact::declaration(reverse)),
            ),
        ],
    )
    .expect("shared-body generation");

    let forward_view = generation.view(forward_function).expect("Forward view");
    let reverse_view = generation.view(reverse_function).expect("Reverse view");
    let forward = forward_view
        .call_argument_placement(call)
        .expect("Forward placement");
    let reverse = reverse_view
        .call_argument_placement(call)
        .expect("Reverse placement");
    assert_eq!(argument_names(&forward.source_order), ["second", "first"]);
    assert_eq!(argument_names(&reverse.source_order), ["second", "first"]);
    assert_eq!(slot_sources(forward), [Some(1), Some(0), None]);
    assert_eq!(slot_sources(reverse), [Some(0), Some(1), None]);
    assert_eq!(slot_names(forward), ["first", "second", "third"]);
    assert_eq!(slot_names(reverse), ["second", "first", "third"]);
}

#[test]
fn external_calls_only_place_named_arguments_and_keep_parameter_spans_absent() {
    let source = SourceId::new(108);
    let text = r#"
fn main() {
    game::add();
    game::add(1, 2, 3);
    game::add(rhs = 2, lhs = 1);
    game::add(rhs = 2);
}
"#;
    let (graph, main) = graph(source, text);
    let mut schema = RegistryFacts::default();
    schema.insert_function(
        "game::add",
        TypeFact::function(vec![TypeFact::I64, TypeFact::I64], TypeFact::I64),
    );
    schema.insert_function_signature(
        "game::add",
        CallableSignatureFact::new(
            [
                CallableParameterFact::new(
                    "lhs",
                    TypeFact::I64,
                    CallableParameterRequirementFact::Required,
                ),
                CallableParameterFact::new(
                    "rhs",
                    TypeFact::I64,
                    CallableParameterRequirementFact::Defaulted,
                ),
            ],
            TypeFact::I64,
        ),
    );
    let function = FunctionId::new(10_801);
    let generation = generation(&graph, Some(&schema), main, function);
    let view = generation.view(function).expect("main view");
    let empty = expression_exact(&graph, source, text, "game::add()");
    let extra = expression_exact(&graph, source, text, "game::add(1, 2, 3)");
    let named = expression_exact(&graph, source, text, "game::add(rhs = 2, lhs = 1)");
    let missing = expression_exact(&graph, source, text, "game::add(rhs = 2)");

    assert_eq!(
        placement_mode(view, empty),
        CallPlacementModeFact::ExternalPositional
    );
    assert_eq!(
        placement_mode(view, extra),
        CallPlacementModeFact::ExternalPositional
    );
    let named = view
        .call_argument_placement(named)
        .expect("named placement");
    assert_eq!(named.mode, CallPlacementModeFact::ExternalNamed);
    assert_eq!(argument_names(&named.source_order), ["rhs", "lhs"]);
    assert_eq!(slot_sources(named), [Some(1), Some(0)]);
    assert_eq!(
        placement_mode(view, missing),
        CallPlacementModeFact::ExternalNamed
    );
    let diagnostics = view.validation_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code.as_deref(),
        Some("compiler::missing_required_argument")
    );
    assert_eq!(diagnostics[0].labels.len(), 1);
}

#[test]
fn set_from_array_uses_exact_external_arity_and_named_placement() {
    let source = SourceId::new(111);
    let text = r#"
fn main() {
    set::from_array([1]);
    set::from_array(values = [2]);
    set::from_array();
    set::from_array([3], [4]);
}
"#;
    let (graph, main) = graph(source, text);
    let mut schema = RegistryFacts::default();
    schema.insert_function(
        "set::from_array",
        TypeFact::function(
            vec![TypeFact::array(TypeFact::Any)],
            TypeFact::set(TypeFact::Any),
        ),
    );
    schema.insert_function_signature(
        "set::from_array",
        CallableSignatureFact::new(
            [CallableParameterFact::new(
                "values",
                TypeFact::array(TypeFact::Any),
                CallableParameterRequirementFact::Required,
            )],
            TypeFact::set(TypeFact::Any),
        ),
    );
    let function = FunctionId::new(11_101);
    let generation = generation(&graph, Some(&schema), main, function);
    let view = generation.view(function).expect("main view");

    let positional = expression_exact(&graph, source, text, "set::from_array([1])");
    assert_eq!(
        placement_mode(view, positional),
        CallPlacementModeFact::ExternalPositional
    );
    assert_eq!(
        slot_sources(
            view.call_argument_placement(positional)
                .expect("positional placement")
        ),
        [Some(0)]
    );

    let named = expression_exact(&graph, source, text, "set::from_array(values = [2])");
    let named = view
        .call_argument_placement(named)
        .expect("named placement");
    assert_eq!(named.mode, CallPlacementModeFact::ExternalNamed);
    assert_eq!(argument_names(&named.source_order), ["values"]);
    assert_eq!(slot_sources(named), [Some(0)]);

    let diagnostics = view.validation_diagnostics();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics[0].code.as_deref(),
        Some("compiler::missing_required_argument")
    );
    assert_eq!(
        span_text(text, diagnostics[0].span.expect("missing span")),
        "set::from_array()"
    );
    assert_eq!(
        diagnostics[1].code.as_deref(),
        Some("compiler::too_many_arguments")
    );
    assert_eq!(
        span_text(text, diagnostics[1].span.expect("extra span")),
        "[4]"
    );
}

#[test]
fn dynamic_calls_retain_names_while_positional_callables_reject_them() {
    let source = SourceId::new(109);
    let text = r#"
fn main(value: Any, callback) {
    value.missing(second = 2, first = 1);
    callback(value = 1);
}
"#;
    let (graph, main) = graph(source, text);
    let function = FunctionId::new(10_901);
    let generation = generation(&graph, None, main, function);
    let view = generation.view(function).expect("main view");
    let dynamic = expression_exact(&graph, source, text, "value.missing(second = 2, first = 1)");
    let positional = expression_exact(&graph, source, text, "callback(value = 1)");
    let dynamic = view
        .call_argument_placement(dynamic)
        .expect("dynamic placement");
    assert_eq!(dynamic.mode, CallPlacementModeFact::Dynamic);
    assert_eq!(argument_names(&dynamic.source_order), ["second", "first"]);
    assert!(dynamic.parameter_slots.is_none());
    assert_eq!(
        placement_mode(view, positional),
        CallPlacementModeFact::Positional
    );
    assert_eq!(
        view.validation_diagnostics()[0].code.as_deref(),
        Some("compiler::unknown_named_argument")
    );
    assert_eq!(view.validation_diagnostics()[0].labels.len(), 1);
}

#[test]
fn tuple_variant_calls_use_strict_parameter_slots() {
    let source = SourceId::new(110);
    let text = r#"
enum Pair { Values(first: i64, second: i64, third: i64 = 3) }
fn main() { Pair::Values(second = 2, first = 1); }
"#;
    let (graph, main) = graph(source, text);
    let function = FunctionId::new(11_001);
    let generation = generation(&graph, None, main, function);
    let view = generation.view(function).expect("main view");
    let call = expression_exact(&graph, source, text, "Pair::Values(second = 2, first = 1)");
    let placement = view
        .call_argument_placement(call)
        .expect("variant placement");

    assert_eq!(placement.mode, CallPlacementModeFact::Strict);
    assert_eq!(argument_names(&placement.source_order), ["second", "first"]);
    assert_eq!(slot_names(placement), ["first", "second", "third"]);
    assert_eq!(slot_sources(placement), [Some(1), Some(0), None]);
}

fn graph(source: SourceId, text: &str) -> (ModuleGraph, HirDeclId) {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        source,
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
    schema: Option<&RegistryFacts>,
    declaration: HirDeclId,
    function: FunctionId,
) -> ExecutableAnalysisGeneration {
    let body = graph.function_body(declaration).expect("function body");
    match schema {
        Some(schema) => ExecutableAnalysisGeneration::from_module_graph_and_schema(
            graph,
            schema,
            [ExecutableAnalysisInput::new(function, body.id)],
        ),
        None => ExecutableAnalysisGeneration::from_module_graph(
            graph,
            [ExecutableAnalysisInput::new(function, body.id)],
        ),
    }
    .expect("executable analysis")
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
            u32::try_from(start).expect("test source offset"),
            u32::try_from(start + expression.len()).expect("test source end"),
        ))
        .expect("HIR expression at source span")
}

fn argument_names(arguments: &[CallSourceArgumentFact]) -> Vec<&str> {
    arguments
        .iter()
        .map(|argument| argument.name.as_deref().expect("named argument"))
        .collect()
}

fn slot_sources(placement: &super::CallArgumentPlacementFact) -> Vec<Option<usize>> {
    placement
        .parameter_slots
        .as_ref()
        .expect("parameter slots")
        .iter()
        .map(|slot| match slot.value {
            CallParameterSlotValueFact::Explicit { source_index, .. } => Some(source_index),
            CallParameterSlotValueFact::MissingDefault => None,
        })
        .collect()
}

fn slot_names(placement: &super::CallArgumentPlacementFact) -> Vec<&str> {
    placement
        .parameter_slots
        .as_ref()
        .expect("parameter slots")
        .iter()
        .map(|slot| slot.name.as_str())
        .collect()
}

fn placement_mode(
    view: crate::executable::ExecutableAnalysisView<'_>,
    expression: HirExprId,
) -> CallPlacementModeFact {
    view.call_argument_placement(expression)
        .expect("call placement")
        .mode
}

fn span_text(text: &str, span: Span) -> &str {
    &text[usize::try_from(span.start).expect("span start")
        ..usize::try_from(span.end).expect("span end")]
}
