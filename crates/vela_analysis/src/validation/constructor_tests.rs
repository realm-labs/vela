use vela_common::{SourceId, Span};
use vela_def::{FieldId, FunctionId, TypeId};
use vela_hir::body::HirExprKind;
use vela_hir::ids::{HirDeclId, HirExprId};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use super::{
    CallParameterSlotValueFact, ConstructorInputKindFact, ConstructorSlotValueFact,
    ConstructorSourceValueFact,
};
use crate::executable::{
    ExecutableAnalysisGeneration, ExecutableAnalysisInput, ExecutableReceiverInput,
};
use crate::registry::{RegistryFacts, RegistryFieldAccessFact, RegistryFieldTargetFact};
use crate::semantic_facts::{ConstructorTargetFact, ScriptTypeTargetFact};
use crate::type_fact::TypeFact;

#[test]
fn source_constructor_placements_separate_evaluation_order_from_schema_slots() {
    let source = SourceId::new(201);
    let text = r#"
struct Reward { first: i64, second: String = "default" }
enum Event {
    Named { first: i64, second: String = "default" },
    Tuple(first: i64, second: String = "default"),
}
fn main() {
    Reward { second: "two", first: 1 };
    Event::Named { second: "two", first: 1 };
    Event::Tuple(second = "two", first = 1);
    Reward { first: 2 };
    Event::Tuple(first = 2);
}
"#;
    let (graph, main) = graph(source, text);
    let reward = declaration_named(&graph, "Reward");
    let event = declaration_named(&graph, "Event");
    let function = FunctionId::new(20_101);
    let generation = generation(&graph, None, main, function);
    let view = generation.view(function).expect("main view");

    let reward_expression =
        expression_exact(&graph, source, text, "Reward { second: \"two\", first: 1 }");
    let reward_placement = view
        .constructor_placement(reward_expression)
        .expect("Reward placement");
    assert_eq!(
        reward_placement.target,
        ConstructorTargetFact::Declaration(reward)
    );
    assert_eq!(
        reward_placement.input_kind,
        ConstructorInputKindFact::RecordFields
    );
    assert_eq!(
        source_names(&reward_placement.source_order),
        ["second", "first"]
    );
    assert_eq!(slot_names(reward_placement), ["first", "second"]);
    assert_eq!(slot_sources(reward_placement), [Some(1), Some(0)]);
    assert_eq!(
        slot_expected(reward_placement),
        [TypeFact::I64, TypeFact::STRING]
    );

    let named = expression_exact(
        &graph,
        source,
        text,
        "Event::Named { second: \"two\", first: 1 }",
    );
    let named = view
        .constructor_placement(named)
        .expect("record variant placement");
    assert_eq!(
        named.target,
        ConstructorTargetFact::Variant {
            enum_declaration: event,
            variant: "Named".to_owned(),
        }
    );
    assert_eq!(source_names(&named.source_order), ["second", "first"]);
    assert_eq!(slot_names(named), ["first", "second"]);
    assert_eq!(slot_sources(named), [Some(1), Some(0)]);

    let tuple_expression = expression_exact(
        &graph,
        source,
        text,
        "Event::Tuple(second = \"two\", first = 1)",
    );
    let tuple = view
        .constructor_placement(tuple_expression)
        .expect("tuple variant placement");
    assert_eq!(tuple.input_kind, ConstructorInputKindFact::TupleArguments);
    assert_eq!(source_names(&tuple.source_order), ["second", "first"]);
    assert_eq!(slot_names(tuple), ["0", "1"]);
    assert_eq!(parameter_names(tuple), ["first", "second"]);
    assert_eq!(slot_sources(tuple), [Some(1), Some(0)]);
    let call = view
        .call_argument_placement(tuple_expression)
        .expect("tuple call placement");
    assert_eq!(
        call.source_order
            .iter()
            .map(|argument| argument.name.as_deref().expect("named argument"))
            .collect::<Vec<_>>(),
        ["second", "first"]
    );
    assert_eq!(
        call.parameter_slots
            .as_ref()
            .expect("tuple call slots")
            .iter()
            .map(|slot| match slot.value {
                CallParameterSlotValueFact::Explicit { source_index, .. } => Some(source_index),
                CallParameterSlotValueFact::MissingDefault => None,
            })
            .collect::<Vec<_>>(),
        [Some(1), Some(0)]
    );

    let reward_default = expression_exact(&graph, source, text, "Reward { first: 2 }");
    let reward_default = view
        .constructor_placement(reward_default)
        .expect("record default placement");
    let reward_default_body = graph.struct_shape(reward).expect("Reward shape").fields[1]
        .default_body
        .expect("Reward.second default body");
    assert!(matches!(
        slot_value(reward_default, 1),
        ConstructorSlotValueFact::SourceDefault { body }
            if *body == reward_default_body
    ));

    let tuple_default = expression_exact(&graph, source, text, "Event::Tuple(first = 2)");
    let tuple_default = view
        .constructor_placement(tuple_default)
        .expect("tuple default placement");
    let tuple_default_body = match &graph.enum_shape(event).expect("Event shape").variants[1].fields
    {
        vela_hir::type_hint::EnumVariantFieldsHint::Tuple(fields) => {
            fields[1].default_body.expect("Tuple.second default body")
        }
        other => panic!("expected tuple fields, got {other:?}"),
    };
    assert!(matches!(
        slot_value(tuple_default, 1),
        ConstructorSlotValueFact::SourceDefault { body }
            if *body == tuple_default_body
    ));
    assert_eq!(view.validation_diagnostics(), &[]);
}

#[test]
fn constructor_diagnostics_precede_generic_tuple_call_diagnostics() {
    let source = SourceId::new(202);
    let text = r#"
struct Reward { first: i64, second: i64 }
enum Item { Pair(first: i64), Known { amount: i64 } }
fn main() {
    Reward { extra: 1, extra: 2 };
    Item::Pair();
    Item::Pair(1, 2);
    Item::Missing { amount: 1 };
}
"#;
    let (graph, main) = graph(source, text);
    let function = FunctionId::new(20_201);
    let generation = generation(&graph, None, main, function);
    let view = generation.view(function).expect("main view");
    let diagnostics = view.validation_diagnostics();
    let codes = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_deref().expect("diagnostic code"))
        .collect::<Vec<_>>();

    assert_eq!(
        codes,
        [
            "compiler::duplicate_constructor_field",
            "compiler::unknown_constructor_field",
            "compiler::unknown_constructor_field",
            "compiler::missing_constructor_field",
            "compiler::missing_constructor_field",
            "compiler::missing_constructor_field",
            "compiler::unknown_constructor_field",
            "compiler::unknown_constructor_variant",
        ]
    );
    assert!(!codes.contains(&"compiler::missing_required_argument"));
    assert!(!codes.contains(&"compiler::too_many_arguments"));
    assert_eq!(
        diagnostics[0]
            .labels
            .iter()
            .map(|label| span_text(text, label.span))
            .collect::<Vec<_>>(),
        ["extra", "extra"]
    );
    let unknown = &diagnostics[1];
    assert_eq!(
        unknown.message,
        "unknown constructor field `extra` for `Reward`"
    );
    assert_eq!(
        span_text(text, unknown.span.expect("unknown span")),
        "extra"
    );
    assert_eq!(
        unknown
            .labels
            .iter()
            .map(|label| (span_text(text, label.span), label.message.as_str()))
            .collect::<Vec<_>>(),
        [
            ("extra", "field is not declared by the constructor schema"),
            ("extra", "available fields: first, second"),
        ]
    );
    assert!(unknown.candidates.is_empty());
    assert!(unknown.repairs.is_empty());

    let missing_tuple = &diagnostics[5];
    assert_eq!(
        missing_tuple.message,
        "missing constructor field `0` for `Item::Pair`"
    );
    assert_eq!(
        span_text(text, missing_tuple.span.expect("missing tuple span")),
        "Item::Pair()"
    );
    assert_eq!(missing_tuple.labels.len(), 1);
    assert_eq!(
        span_text(text, missing_tuple.labels[0].span),
        "Item::Pair()"
    );
    assert_eq!(
        missing_tuple.labels[0].message,
        "required field is not provided and has no default"
    );

    let unknown_variant = diagnostics.last().expect("unknown variant");
    assert_eq!(
        unknown_variant.message,
        "unknown enum variant `Item::Missing`"
    );
    assert_eq!(
        span_text(text, unknown_variant.span.expect("unknown variant span")),
        "Item::Missing { amount: 1 }"
    );
    assert_eq!(unknown_variant.labels.len(), 1);
    assert_eq!(
        unknown_variant.labels[0].message,
        "variant is not declared on this enum"
    );

    let missing = expression_exact(&graph, source, text, "Item::Pair()");
    let extra = expression_exact(&graph, source, text, "Item::Pair(1, 2)");
    assert!(
        view.constructor_placement(missing)
            .expect("missing tuple placement")
            .declaration_slots
            .is_none()
    );
    assert!(
        view.constructor_placement(extra)
            .expect("extra tuple placement")
            .declaration_slots
            .is_none()
    );
}

#[test]
fn imported_and_qualified_constructor_paths_share_declaration_identities() {
    let main_source = SourceId::new(203);
    let main_text = r#"
use game::schema::Reward as Prize
use game::schema::State as ImportedState
fn main() {
    Prize { amount: 1 };
    game::schema::Reward { amount: 2 };
    ImportedState::Pair(amount = 3);
    game::schema::State::Pair(amount = 4);
}
"#;
    let mut graph = ModuleGraph::new();
    let main_module = graph.add_source(ModuleSource::new(
        main_source,
        ModulePath::from_qualified("game::main"),
        main_text,
    ));
    graph.add_source(ModuleSource::new(
        SourceId::new(204),
        ModulePath::from_qualified("game::schema"),
        "pub struct Reward { amount: i64 }\npub enum State { Pair(amount: i64) }",
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let main = graph
        .module(main_module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let reward = declaration_named(&graph, "Reward");
    let state = declaration_named(&graph, "State");
    let function = FunctionId::new(20_301);
    let generation = generation(&graph, None, main, function);
    let view = generation.view(function).expect("main view");

    for expression_text in ["Prize { amount: 1 }", "game::schema::Reward { amount: 2 }"] {
        let expression = expression_exact(&graph, main_source, main_text, expression_text);
        assert_eq!(
            view.constructor_placement(expression)
                .expect("record placement")
                .target,
            ConstructorTargetFact::Declaration(reward)
        );
    }
    for expression_text in [
        "ImportedState::Pair(amount = 3)",
        "game::schema::State::Pair(amount = 4)",
    ] {
        let expression = expression_exact(&graph, main_source, main_text, expression_text);
        let placement = view.constructor_placement(expression).unwrap_or_else(|| {
            panic!(
                "variant placement for {expression_text}: {:?}",
                view.call_target(expression)
            )
        });
        assert_eq!(
            placement.target,
            ConstructorTargetFact::Variant {
                enum_declaration: state,
                variant: "Pair".to_owned(),
            }
        );
        assert_eq!(slot_names(placement), ["0"]);
        assert_eq!(parameter_names(placement), ["amount"]);
    }
}

#[test]
fn dynamic_constructors_retain_names_and_order_but_only_validate_duplicates() {
    let source = SourceId::new(205);
    let text = r#"
fn main() {
    Missing { second: 2, first: 1 };
    Missing::Pair { label: "x", amount: 3 };
    Duplicate { value: 1, value: 2 };
}
"#;
    let (graph, main) = graph(source, text);
    let function = FunctionId::new(20_501);
    let generation = generation(&graph, None, main, function);
    let view = generation.view(function).expect("main view");

    for (expression_text, names) in [
        ("Missing { second: 2, first: 1 }", vec!["second", "first"]),
        (
            "Missing::Pair { label: \"x\", amount: 3 }",
            vec!["label", "amount"],
        ),
    ] {
        let expression = expression_exact(&graph, source, text, expression_text);
        let placement = view
            .constructor_placement(expression)
            .expect("dynamic placement");
        assert_eq!(placement.target, ConstructorTargetFact::Dynamic);
        assert_eq!(source_names(&placement.source_order), names);
        assert!(placement.declaration_slots.is_none());
    }
    assert_eq!(
        view.validation_diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code.as_deref())
            .collect::<Vec<_>>(),
        [Some("compiler::duplicate_constructor_field")]
    );
}

#[test]
fn registered_defaults_remain_explicitly_value_unavailable() {
    let source = SourceId::new(206);
    let text = "fn main() { ExternalReward { second: 2 }; ExternalReward {}; }";
    let (graph, main) = graph(source, text);
    let mut schema = RegistryFacts::default();
    schema.insert_type("ExternalReward", TypeFact::record("ExternalReward"));
    insert_registered_field(
        &mut schema,
        TypeId::new(601),
        "ExternalReward",
        "first",
        FieldId::new(6_001),
        0,
        true,
    );
    insert_registered_field(
        &mut schema,
        TypeId::new(601),
        "ExternalReward",
        "second",
        FieldId::new(6_002),
        1,
        false,
    );
    let function = FunctionId::new(20_601);
    let generation = generation(&graph, Some(&schema), main, function);
    let view = generation.view(function).expect("main view");
    let expression = expression_exact(&graph, source, text, "ExternalReward { second: 2 }");
    let placement = view
        .constructor_placement(expression)
        .expect("registered placement");

    assert_eq!(slot_names(placement), ["first", "second"]);
    assert!(matches!(
        slot_value(placement, 0),
        ConstructorSlotValueFact::RegisteredDefaultUnavailable
    ));
    assert!(matches!(
        slot_value(placement, 1),
        ConstructorSlotValueFact::Explicit {
            source_index: 0,
            value: Some(_),
        }
    ));
    assert_eq!(
        view.validation_diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code.as_deref())
            .collect::<Vec<_>>(),
        [Some("compiler::missing_constructor_field")]
    );
}

#[test]
fn registered_variant_short_name_resolves_one_canonical_schema_owner() {
    let source = SourceId::new(208);
    let text = "fn main() { State::Ready { amount: 1 }; }";
    let (graph, main) = graph(source, text);
    let mut schema = RegistryFacts::default();
    schema.insert_type("State", TypeFact::enum_type("game::State", None::<String>));
    schema.insert_variant(
        "game::State",
        "Ready",
        TypeFact::enum_type("game::State", Some("Ready")),
    );
    schema.insert_field("game::State::Ready", "amount", TypeFact::I64);
    let access = RegistryFieldAccessFact {
        owner: "game::State::Ready".to_owned(),
        name: "amount".to_owned(),
        readable: true,
        writable: true,
        reflect_readable: true,
        reflect_writable: true,
        required_permissions: Vec::new(),
    };
    schema.insert_field_target(RegistryFieldTargetFact::new(
        TypeId::new(801),
        "game::State::Ready",
        "amount",
        FieldId::new(8_001),
        None,
        true,
        access,
    ));
    let function = FunctionId::new(20_801);
    let generation = generation(&graph, Some(&schema), main, function);
    let view = generation.view(function).expect("main view");
    let expression = expression_exact(&graph, source, text, "State::Ready { amount: 1 }");
    let placement = view
        .constructor_placement(expression)
        .expect("registered variant placement");

    assert_eq!(
        placement.target,
        ConstructorTargetFact::RegistryVariant {
            owner: "game::State".to_owned(),
            variant: "Ready".to_owned(),
        }
    );
    assert_eq!(slot_names(placement), ["amount"]);
    assert_eq!(slot_sources(placement), [Some(0)]);
    assert_eq!(view.validation_diagnostics(), &[]);
}

#[test]
fn ambiguous_registry_short_names_do_not_merge_constructor_schemas() {
    let mut schema = RegistryFacts::default();
    for (owner, type_id, field_id) in [
        ("alpha::Reward", TypeId::new(701), FieldId::new(7_001)),
        ("beta::Reward", TypeId::new(702), FieldId::new(7_002)),
    ] {
        insert_registered_field(&mut schema, type_id, owner, "amount", field_id, 0, false);
    }
    assert!(
        schema
            .field_targets_for_owner_or_short_name("Reward")
            .is_empty()
    );
    assert_eq!(
        schema
            .field_targets_for_owner_or_short_name("alpha::Reward")
            .iter()
            .map(|target| target.owner_name.as_str())
            .collect::<Vec<_>>(),
        ["alpha::Reward"]
    );
}

#[test]
fn shared_hir_body_constructor_facts_remain_executable_qualified() {
    let source = SourceId::new(207);
    let text = r#"
struct Reward { amount: i64 = 1 }
trait Build { fn build(self) { Reward {}; } }
struct Alpha {}
struct Beta {}
impl Build for Alpha {}
impl Build for Beta {}
"#;
    let (graph, _) = graph(source, text);
    let build_trait = declaration_named(&graph, "Build");
    let alpha = declaration_named(&graph, "Alpha");
    let beta = declaration_named(&graph, "Beta");
    let method = graph
        .trait_shape(build_trait)
        .expect("Build shape")
        .methods
        .iter()
        .find(|method| method.name == "build")
        .expect("build method");
    let body = graph
        .trait_default_method_body(method.default_body_node.expect("build body node"))
        .expect("build body");
    let constructor = body
        .expressions
        .values()
        .find(|expression| matches!(expression.kind, HirExprKind::Record { .. }))
        .expect("Reward constructor")
        .id;
    let alpha_function = FunctionId::new(20_701);
    let beta_function = FunctionId::new(20_702);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [
            ExecutableAnalysisInput::new(alpha_function, body.id).with_receiver(
                ExecutableReceiverInput::new(TypeFact::record("game::Alpha"))
                    .with_script_type(ScriptTypeTargetFact::declaration(alpha)),
            ),
            ExecutableAnalysisInput::new(beta_function, body.id).with_receiver(
                ExecutableReceiverInput::new(TypeFact::record("game::Beta"))
                    .with_script_type(ScriptTypeTargetFact::declaration(beta)),
            ),
        ],
    )
    .expect("shared executable generation");
    let alpha_view = generation.view(alpha_function).expect("Alpha view");
    let beta_view = generation.view(beta_function).expect("Beta view");
    let alpha = alpha_view
        .constructor_placement(constructor)
        .expect("Alpha constructor");
    let beta = beta_view
        .constructor_placement(constructor)
        .expect("Beta constructor");
    assert_eq!(alpha, beta);
    assert!(matches!(
        slot_value(alpha, 0),
        ConstructorSlotValueFact::SourceDefault { .. }
    ));
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

fn source_names(arguments: &[ConstructorSourceValueFact]) -> Vec<&str> {
    arguments
        .iter()
        .map(|argument| argument.name.as_deref().expect("named constructor input"))
        .collect()
}

fn slot_names(placement: &super::ConstructorPlacementFact) -> Vec<&str> {
    placement
        .declaration_slots
        .as_ref()
        .expect("declaration slots")
        .iter()
        .map(|slot| slot.field_name.as_str())
        .collect()
}

fn parameter_names(placement: &super::ConstructorPlacementFact) -> Vec<&str> {
    placement
        .declaration_slots
        .as_ref()
        .expect("declaration slots")
        .iter()
        .map(|slot| slot.parameter_name.as_str())
        .collect()
}

fn slot_sources(placement: &super::ConstructorPlacementFact) -> Vec<Option<usize>> {
    placement
        .declaration_slots
        .as_ref()
        .expect("declaration slots")
        .iter()
        .map(|slot| match slot.value {
            ConstructorSlotValueFact::Explicit { source_index, .. } => Some(source_index),
            ConstructorSlotValueFact::SourceDefault { .. }
            | ConstructorSlotValueFact::SourceDefaultUnavailable { .. }
            | ConstructorSlotValueFact::RegisteredDefaultUnavailable => None,
        })
        .collect()
}

fn slot_expected(placement: &super::ConstructorPlacementFact) -> Vec<TypeFact> {
    placement
        .declaration_slots
        .as_ref()
        .expect("declaration slots")
        .iter()
        .map(|slot| slot.expected.clone())
        .collect()
}

fn slot_value(
    placement: &super::ConstructorPlacementFact,
    index: usize,
) -> &ConstructorSlotValueFact {
    &placement
        .declaration_slots
        .as_ref()
        .expect("declaration slots")[index]
        .value
}

fn span_text(text: &str, span: Span) -> &str {
    &text[usize::try_from(span.start).expect("span start")
        ..usize::try_from(span.end).expect("span end")]
}

fn insert_registered_field(
    schema: &mut RegistryFacts,
    owner: TypeId,
    owner_name: &str,
    name: &str,
    field: FieldId,
    declaration_order: u32,
    has_default: bool,
) {
    schema.insert_field(owner_name, name, TypeFact::I64);
    let access = RegistryFieldAccessFact {
        owner: owner_name.to_owned(),
        name: name.to_owned(),
        readable: true,
        writable: true,
        reflect_readable: true,
        reflect_writable: true,
        required_permissions: Vec::new(),
    };
    schema.insert_field_target(
        RegistryFieldTargetFact::new(owner, owner_name, name, field, None, false, access)
            .declaration_order(declaration_order)
            .defaulted(has_default),
    );
}
