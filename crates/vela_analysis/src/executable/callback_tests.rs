use vela_common::SourceId;
use vela_def::FunctionId;
use vela_hir::body::{HirBody, HirBodyOwner, HirExpr, HirExprKind};
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use super::{
    ExecutableAnalysisGeneration, ExecutableAnalysisInput, ExecutableAnalysisView,
    ExecutableReceiverInput,
};
use crate::logical_records::{LogicalRecordKind, map_entry};
use crate::registry::RegistryFacts;
use crate::semantic_facts::{CallTargetFact, MemberTargetFact, ScriptTypeTargetFact};
use crate::type_fact::TypeFact;

#[test]
fn callback_params_are_qualified_by_concrete_executable() {
    let (graph, _) = graph(
        91,
        r#"
trait Probe {
    fn inspect(self) {
        return self.values.map(|value| value);
    }
}

struct Labels { values: Array<String> }
struct Scores { values: Array<i64> }

impl Probe for Labels {}
impl Probe for Scores {}
"#,
    );
    let probe = declaration_named(&graph, "Probe");
    let labels = declaration_named(&graph, "Labels");
    let scores = declaration_named(&graph, "Scores");
    let inspect = graph
        .trait_shape(probe)
        .expect("Probe trait shape")
        .methods
        .iter()
        .find(|method| method.name == "inspect")
        .expect("inspect default method");
    let body = graph
        .trait_default_method_body(
            inspect
                .default_body_node
                .expect("inspect default body node"),
        )
        .expect("inspect default body");
    let lambda = child_lambdas(&graph, body.id)
        .into_iter()
        .next()
        .expect("inspect callback body");
    let parameter = lambda.params[0].local;
    let map = method_calls(body, "map")[0];
    let values = fields_named(body, "values")[0];

    let labels_function = FunctionId::new(9_101);
    let scores_function = FunctionId::new(9_102);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [
            ExecutableAnalysisInput::new(labels_function, body.id).with_receiver(
                ExecutableReceiverInput::new(TypeFact::record("game::Labels"))
                    .with_script_type(ScriptTypeTargetFact::declaration(labels)),
            ),
            ExecutableAnalysisInput::new(scores_function, body.id).with_receiver(
                ExecutableReceiverInput::new(TypeFact::record("game::Scores"))
                    .with_script_type(ScriptTypeTargetFact::declaration(scores)),
            ),
        ],
    )
    .expect("executable-qualified callback analysis");
    let labels_view = generation.view(labels_function).expect("Labels view");
    let scores_view = generation.view(scores_function).expect("Scores view");

    assert_eq!(labels_view.local(parameter), Some(&TypeFact::STRING));
    assert_eq!(scores_view.local(parameter), Some(&TypeFact::I64));
    assert_eq!(
        labels_view.expression(map),
        Some(&TypeFact::array(TypeFact::STRING))
    );
    assert_eq!(
        scores_view.expression(map),
        Some(&TypeFact::array(TypeFact::I64))
    );
    assert_eq!(
        labels_view.member_target(values),
        Some(&MemberTargetFact::ScriptField {
            owner: labels,
            variant: None,
            name: "values".to_owned(),
        })
    );
    assert_eq!(
        scores_view.member_target(values),
        Some(&MemberTargetFact::ScriptField {
            owner: scores,
            variant: None,
            name: "values".to_owned(),
        })
    );
}

#[test]
fn source_record_and_iterator_callbacks_converge_to_stable_member_facts() {
    let (graph, main) = graph(
        92,
        r#"
struct Bag { tags: Array<String> }

fn main() {
    let bags = [Bag { tags: ["daily", "quest"] }];
    let mapped = bags.map(|bag| bag.tags.join(","));
    let iter = mapped.iter().map(|text| text.to_upper());
    let tags = set::from_array(["daily", "raid"]);
    let selected = tags.filter(|tag| tag.starts_with("d"));
    return iter.collect_array().join("|") + selected.values().collect_array().join("|");
}
"#,
    );
    let bag = declaration_named(&graph, "Bag");
    let body = graph.function_body(main).expect("main body");
    let lambdas = child_lambdas(&graph, body.id);
    assert_eq!(lambdas.len(), 3);
    let bag_param = lambdas[0].params[0].local;
    let text_param = lambdas[1].params[0].local;
    let tag_param = lambdas[2].params[0].local;
    let map_calls = method_calls(body, "map");
    let iter_call = method_calls(body, "iter")[0];
    let collect_call = method_calls(body, "collect_array")[0];
    let bag_tags = fields_named(lambdas[0], "tags")[0];
    let bag_join = method_calls(lambdas[0], "join")[0];
    let text_upper = method_calls(lambdas[1], "to_upper")[0];
    let tag_starts_with = method_calls(lambdas[2], "starts_with")[0];

    let function = FunctionId::new(9_201);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(function, body.id)],
    )
    .expect("callback analysis");
    let view = generation.view(function).expect("main view");

    assert_eq!(view.local(bag_param), Some(&TypeFact::record("game::Bag")));
    assert_eq!(
        view.local_script_type(bag_param),
        Some(&ScriptTypeTargetFact::declaration(bag))
    );
    assert_eq!(view.local(text_param), Some(&TypeFact::STRING));
    assert_eq!(view.local(tag_param), Some(&TypeFact::STRING));
    assert_eq!(
        view.member_target(bag_tags),
        Some(&MemberTargetFact::ScriptField {
            owner: bag,
            variant: None,
            name: "tags".to_owned(),
        })
    );
    assert_stdlib_call(&view, bag_join, "join");
    assert_stdlib_call(&view, text_upper, "to_upper");
    assert_stdlib_call(&view, tag_starts_with, "starts_with");
    assert_eq!(
        view.expression(map_calls[0]),
        Some(&TypeFact::array(TypeFact::STRING))
    );
    assert_eq!(
        view.expression(iter_call),
        Some(&TypeFact::iterator(TypeFact::STRING))
    );
    assert_eq!(
        view.expression(map_calls[1]),
        Some(&TypeFact::iterator(TypeFact::STRING))
    );
    assert_eq!(
        view.expression(collect_call),
        Some(&TypeFact::array(TypeFact::STRING))
    );
}

#[test]
fn map_callbacks_use_value_or_key_value_facts_from_lambda_arity() {
    let (graph, main) = graph(
        93,
        r#"
fn main() {
    let rewards = {"gold": 5, "gem": 6};
    let mapped = rewards.map_values(|value| value + 1);
    let filtered = rewards.filter(|key, value| key.len() >= 3 && value > 0);
    let found = rewards.find(|value| value == 6);
    return mapped.len() + filtered.len() + found.unwrap_or(MapEntry { key: "", value: 0 }).value;
}
"#,
    );
    let body = graph.function_body(main).expect("main body");
    let lambdas = child_lambdas(&graph, body.id);
    assert_eq!(lambdas.len(), 3);
    let map_values = method_calls(body, "map_values")[0];
    let filter = method_calls(body, "filter")[0];
    let find = method_calls(body, "find")[0];
    let key_len = method_calls(lambdas[1], "len")[0];
    let collection_lengths = method_calls(body, "len");

    let function = FunctionId::new(9_301);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(function, body.id)],
    )
    .expect("map callback analysis");
    let view = generation.view(function).expect("main view");

    assert_eq!(view.local(lambdas[0].params[0].local), Some(&TypeFact::I64));
    assert_eq!(
        view.local(lambdas[1].params[0].local),
        Some(&TypeFact::STRING)
    );
    assert_eq!(view.local(lambdas[1].params[1].local), Some(&TypeFact::I64));
    assert_eq!(view.local(lambdas[2].params[0].local), Some(&TypeFact::I64));
    assert_eq!(
        view.expression(map_values),
        Some(&TypeFact::map(TypeFact::STRING, TypeFact::I64))
    );
    assert_eq!(
        view.expression(filter),
        Some(&TypeFact::map(TypeFact::STRING, TypeFact::I64))
    );
    assert_eq!(
        view.expression(find),
        Some(&TypeFact::option(map_entry(
            TypeFact::STRING,
            TypeFact::I64
        )))
    );
    assert_stdlib_call(&view, key_len, "len");
    assert_eq!(collection_lengths.len(), 2);
    for call in collection_lengths {
        assert_stdlib_call(&view, call, "len");
    }
}

#[test]
fn collection_fallbacks_preserve_nested_receiver_targets() {
    let (graph, main) = graph(
        96,
        r#"
fn main() {
    let groups = {"w": ["wolf", "wisp"], "b": ["bat"]};
    let none = option::none();
    let joined = none.unwrap_or(["fallback"]).join(".");
    return groups.get_or("w", []).len() + groups.get("b").unwrap_or([]).len() + joined.len();
}
"#,
    );
    let body = graph.function_body(main).expect("main body");
    let get_or = method_calls(body, "get_or")[0];
    let get = method_calls(body, "get")[0];
    let unwrap_or = method_calls(body, "unwrap_or")[0];
    let option_unwrap_or = method_calls(body, "unwrap_or")[1];
    let join = method_calls(body, "join")[0];
    let lengths = method_calls(body, "len");

    let function = FunctionId::new(9_601);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(function, body.id)],
    )
    .expect("map fallback analysis");
    let view = generation.view(function).expect("main view");
    let arrays = TypeFact::array(TypeFact::STRING);

    assert_eq!(view.expression(get_or), Some(&arrays));
    assert_eq!(
        view.expression(get),
        Some(&TypeFact::option(arrays.clone()))
    );
    assert_eq!(view.expression(unwrap_or), Some(&arrays));
    assert_stdlib_call(&view, get_or, "get_or");
    assert_stdlib_call(&view, get, "get");
    assert_stdlib_call(&view, unwrap_or, "unwrap_or");
    assert_eq!(view.expression(option_unwrap_or), Some(&arrays));
    assert_stdlib_call(&view, option_unwrap_or, "unwrap_or");
    assert_stdlib_call(&view, join, "join");
    assert_eq!(lengths.len(), 3);
    for call in &lengths[..2] {
        let call = *call;
        assert_eq!(call_receiver_fact(body, &view, call), Some(&arrays));
        assert_stdlib_call(&view, call, "len");
    }
    assert_eq!(
        call_receiver_fact(body, &view, lengths[2]),
        Some(&TypeFact::STRING)
    );
    assert_stdlib_call(&view, lengths[2], "len");
}

#[test]
fn logical_map_entries_survive_option_iterator_array_local_and_callback_flow() {
    let (graph, main) = graph(
        95,
        r#"
fn main() {
    let rewards = {"gold": 5, "gem": 6};
    let found = rewards.find(|value| value == 6);
    let entry = found?;
    let entries = rewards.entries().collect_array();
    let first = entries[0];
    let mapped = rewards.entries().map(|entry| entry.key.len() + entry.value).collect_array();
    let rebuilt = rewards.iter().collect_map();
    return entry.value + first.value + mapped[0] + rebuilt.len();
}
"#,
    );
    let body = graph.function_body(main).expect("main body");
    let lambdas = child_lambdas(&graph, body.id);
    assert_eq!(lambdas.len(), 2);

    let entry = map_entry(TypeFact::STRING, TypeFact::I64);
    let find = method_calls(body, "find")[0];
    let try_entry = body
        .expressions
        .values()
        .find(|expression| matches!(expression.kind, HirExprKind::Try { .. }))
        .expect("Map.find try expression")
        .id;
    let collect_entries = method_calls(body, "collect_array")[0];
    let collect_map = method_calls(body, "collect_map")[0];
    let entry_fields = fields_named(body, "value");
    let callback_key = fields_named(lambdas[1], "key")[0];
    let callback_value = fields_named(lambdas[1], "value")[0];

    let function = FunctionId::new(9_501);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(function, body.id)],
    )
    .expect("logical MapEntry flow analysis");
    let view = generation.view(function).expect("main view");

    assert_eq!(
        view.expression(find),
        Some(&TypeFact::option(entry.clone()))
    );
    assert_eq!(view.expression(try_entry), Some(&entry));
    assert_eq!(
        view.expression(collect_entries),
        Some(&TypeFact::array(entry.clone()))
    );
    assert_eq!(view.local(lambdas[1].params[0].local), Some(&entry));
    assert_eq!(
        view.expression(collect_map),
        Some(&TypeFact::map(TypeFact::STRING, TypeFact::I64))
    );

    for field in entry_fields
        .into_iter()
        .chain([callback_key, callback_value])
    {
        let HirExprKind::Field(member) = &body_or_lambda_expression(body, &lambdas, field).kind
        else {
            panic!("member expression");
        };
        assert_eq!(view.expression(member.receiver), Some(&entry));
        let record = entry.as_logical_record().expect("logical MapEntry fact");
        assert_eq!(record.kind(), LogicalRecordKind::MapEntry);
        assert_eq!(
            view.member_target(field),
            Some(&MemberTargetFact::LogicalRecordField(
                record
                    .field_target(&member.name)
                    .expect("stable MapEntry field target")
            ))
        );
    }
}

#[test]
fn option_result_and_typed_registry_callbacks_seed_nested_params() {
    let (graph, main) = graph(
        94,
        r#"
fn main() {
    let option_value = option::some("quest").map(|value| value.to_upper());
    let option_fallback = option::none().or_else(| | option::some("fallback"));
    let result_value = result::ok(["gold", "xp"]).map(|values| values.join("+"));
    let result_error = result::err(["bad", "level"]).map_err(|errors| errors.join("."));
    let result_chained = result::ok("quest").and_then(|text| result::ok(text.to_upper()));
    let checked = fixture::apply((|text| text.starts_with("Q")));
    return option_value.unwrap_or("") + result_value.unwrap_or("")
        + result_error.to_error_option().unwrap_or("") + option_fallback.unwrap_or("")
        + result_chained.unwrap_or("");
}
"#,
    );
    let body = graph.function_body(main).expect("main body");
    let lambdas = child_lambdas(&graph, body.id);
    assert_eq!(lambdas.len(), 6);
    let mut schema = RegistryFacts::default();
    schema.insert_function(
        "fixture::apply",
        TypeFact::function(
            vec![TypeFact::function(vec![TypeFact::STRING], TypeFact::BOOL)],
            TypeFact::BOOL,
        ),
    );

    let function = FunctionId::new(9_401);
    let generation = ExecutableAnalysisGeneration::from_module_graph_and_schema(
        &graph,
        &schema,
        [ExecutableAnalysisInput::new(function, body.id)],
    )
    .expect("Option, Result, and registry callback analysis");
    let view = generation.view(function).expect("main view");

    assert_eq!(
        view.local(lambdas[0].params[0].local),
        Some(&TypeFact::STRING)
    );
    assert!(lambdas[1].params.is_empty());
    assert_eq!(
        view.local(lambdas[2].params[0].local),
        Some(&TypeFact::array(TypeFact::STRING))
    );
    assert_eq!(
        view.local(lambdas[3].params[0].local),
        Some(&TypeFact::array(TypeFact::STRING))
    );
    assert_eq!(
        view.local(lambdas[4].params[0].local),
        Some(&TypeFact::STRING)
    );
    assert_eq!(
        view.local(lambdas[5].params[0].local),
        Some(&TypeFact::STRING)
    );
    let starts_with = method_calls(lambdas[5], "starts_with")[0];
    assert_stdlib_call(&view, starts_with, "starts_with");
}

fn graph(source: u32, text: &str) -> (ModuleGraph, HirDeclId) {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(source),
        ModulePath::from_qualified("game"),
        text,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let main = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .map(|declaration| declaration.id)
        .unwrap_or_else(|| declaration_named(&graph, "Probe"));
    (graph, main)
}

fn declaration_named(graph: &ModuleGraph, name: &str) -> HirDeclId {
    graph
        .declarations()
        .find(|declaration| declaration.name == name)
        .map(|declaration| declaration.id)
        .unwrap_or_else(|| panic!("{name} declaration"))
}

fn child_lambdas(graph: &ModuleGraph, parent: HirBodyId) -> Vec<&HirBody> {
    let mut bodies = graph
        .bodies()
        .filter(|body| matches!(body.owner, HirBodyOwner::Lambda { parent: owner, .. } if owner == parent))
        .collect::<Vec<_>>();
    bodies.sort_by_key(|body| body.origin.span.start);
    bodies
}

fn method_calls(body: &HirBody, method: &str) -> Vec<HirExprId> {
    let mut calls = body
        .expressions
        .values()
        .filter_map(|expression| {
            let HirExprKind::Call(call) = &expression.kind else {
                return None;
            };
            body.field(call.callee)
                .is_some_and(|field| field.name == method)
                .then_some(expression.id)
        })
        .collect::<Vec<_>>();
    calls.sort_by_key(|expression| {
        body.expression(*expression)
            .map(|value| value.origin.span.start)
    });
    calls
}

fn fields_named(body: &HirBody, name: &str) -> Vec<HirExprId> {
    let mut fields = body
        .expressions
        .values()
        .filter_map(|expression| {
            let HirExprKind::Field(field) = &expression.kind else {
                return None;
            };
            (field.name == name).then_some(expression.id)
        })
        .collect::<Vec<_>>();
    fields.sort_by_key(|expression| {
        body.expression(*expression)
            .map(|value| value.origin.span.start)
    });
    fields
}

fn body_or_lambda_expression<'a>(
    body: &'a HirBody,
    lambdas: &[&'a HirBody],
    expression: HirExprId,
) -> &'a HirExpr {
    body.expression(expression)
        .or_else(|| lambdas.iter().find_map(|body| body.expression(expression)))
        .expect("expression in body or child lambda")
}

fn assert_stdlib_call(view: &ExecutableAnalysisView<'_>, call: HirExprId, method: &str) {
    assert_eq!(
        view.call_target(call),
        Some(&CallTargetFact::StdlibMethod {
            name: method.to_owned(),
        })
    );
}

fn call_receiver_fact<'a>(
    body: &HirBody,
    view: &'a ExecutableAnalysisView<'_>,
    call: HirExprId,
) -> Option<&'a TypeFact> {
    let call = body.expression(call)?;
    let HirExprKind::Call(call) = &call.kind else {
        return None;
    };
    let field = body.field(call.callee)?;
    view.expression(field.receiver)
}
