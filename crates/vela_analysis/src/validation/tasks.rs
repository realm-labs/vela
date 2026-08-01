use std::collections::BTreeSet;

use vela_common::{Diagnostic, NonDetachableValueKind, Span};
use vela_hir::binding::{BindingResolution, TaskLexicalCapability};
use vela_hir::body::HirBody;
use vela_hir::ids::{HirDeclId, HirExprId};
use vela_hir::module_graph::ModuleGraph;

use super::{CallParameterSlotValueFact, ExecutableValidationFacts};
use crate::callable::CallableParameterRequirementFact;
use crate::facts::AnalysisFacts;
use crate::registry::{RegistryEffectFact, RegistryFacts};
use crate::semantic_facts::CallTargetFact;
use crate::type_fact::TypeFact;

pub(super) fn record_body(
    validation: &mut ExecutableValidationFacts,
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    facts: &AnalysisFacts,
    body: &HirBody,
) {
    let Some(bindings) = graph.bindings_for_body(body.id) else {
        return;
    };
    for (task_callee, capability) in bindings.task_capabilities() {
        let Some((task_expression, task_call)) = body
            .calls()
            .find(|(_, candidate)| candidate.callee == task_callee)
        else {
            continue;
        };
        let Some(worker_call) = task_call
            .arguments
            .first()
            .and_then(|argument| argument.value)
            .and_then(|expression| body.call(expression).map(|call| (expression, call)))
        else {
            continue;
        };
        let Some(worker) = declaration_target(bindings, worker_call.1.callee) else {
            continue;
        };
        validate_worker(validation, graph, facts, body, worker_call.0, worker);
        let continuation = if capability == TaskLexicalCapability::SpawnScopedThen
            && let Some(continuation_expression) = task_call
                .arguments
                .get(1)
                .and_then(|argument| argument.value)
            && let Some(continuation) = declaration_target(bindings, continuation_expression)
        {
            validate_continuation(
                validation,
                graph,
                worker,
                continuation,
                continuation_expression,
                expression_span(body, task_expression),
            );
            Some(continuation)
        } else {
            None
        };
        validate_effect_ceiling(
            validation,
            graph,
            schema,
            facts,
            worker,
            continuation,
            expression_span(body, task_expression),
        );
    }
}

fn validate_effect_ceiling(
    validation: &mut ExecutableValidationFacts,
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    facts: &AnalysisFacts,
    worker: HirDeclId,
    continuation: Option<HirDeclId>,
    span: Span,
) {
    let Some(schema) = schema else {
        return;
    };
    let Some(ceiling) = schema.execution_effect_ceiling() else {
        return;
    };
    let mut required = RegistryEffectFact {
        spawns_tasks: true,
        ..RegistryEffectFact::pure()
    };
    let mut visited = BTreeSet::new();
    collect_source_effects(graph, schema, facts, worker, &mut visited, &mut required);
    if let Some(continuation) = continuation {
        collect_source_effects(
            graph,
            schema,
            facts,
            continuation,
            &mut visited,
            &mut required,
        );
    }
    let denied = required.denied_by(ceiling);
    if denied.is_empty() {
        return;
    }
    let target = graph
        .declaration(worker)
        .map_or("<unknown>", |declaration| declaration.name.as_str());
    let denied = denied
        .into_iter()
        .map(|effect| format!("`{effect}` / `{}`", effect_capability(effect)))
        .collect::<Vec<_>>()
        .join(", ");
    validation.diagnostics.push(
        Diagnostic::error(format!(
            "detached target `{target}` requires denied effect/capability {denied}"
        ))
        .with_code("analysis::task_effect_denied")
        .with_span(span)
        .with_label(
            span,
            "task admission also requires an explicit host TaskScope at runtime",
        ),
    );
}

fn collect_source_effects(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    facts: &AnalysisFacts,
    declaration: HirDeclId,
    visited: &mut BTreeSet<HirDeclId>,
    effects: &mut RegistryEffectFact,
) {
    if !visited.insert(declaration) {
        return;
    }
    let Some(body) = graph.function_body(declaration) else {
        return;
    };
    if graph
        .bindings_for_body(body.id)
        .is_some_and(|bindings| bindings.task_capabilities().next().is_some())
    {
        effects.spawns_tasks = true;
    }
    for (expression, _) in body.calls() {
        match facts.call_target(expression) {
            Some(CallTargetFact::Declaration(callee)) => {
                collect_source_effects(graph, schema, facts, *callee, visited, effects);
            }
            Some(
                CallTargetFact::RegistryFunction { path }
                | CallTargetFact::NativeFunction { path }
                | CallTargetFact::StdlibFunction { path },
            ) => {
                if let Some(effect) = schema.function_effect_fact(path) {
                    effects.union_with(effect);
                }
            }
            Some(
                CallTargetFact::HostMethod { owner, name }
                | CallTargetFact::RegistryMethod { owner, name },
            ) => {
                if let Some(effect) = schema.method_effect_fact(owner, name) {
                    effects.union_with(effect);
                }
            }
            _ => {}
        }
    }
}

fn effect_capability(effect: &str) -> &'static str {
    match effect {
        "reads_host" => "host_read",
        "writes_host" => "host_write",
        "emits_events" => "event_emit",
        "reads_time" => "time",
        "uses_random" => "random",
        "reads_io" => "io_read",
        "writes_io" => "io_write",
        "reads_reflection" => "reflection_read",
        "writes_reflection" => "reflection_write",
        "calls_reflection" => "reflection_call",
        "spawns_tasks" => "task_spawn",
        _ => "unknown",
    }
}

fn declaration_target(
    bindings: &vela_hir::binding::BindingMap,
    expression: HirExprId,
) -> Option<HirDeclId> {
    match bindings.resolution(expression) {
        Some(BindingResolution::Declaration(declaration)) => Some(*declaration),
        _ => None,
    }
}

fn validate_worker(
    validation: &mut ExecutableValidationFacts,
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    body: &HirBody,
    worker_call: HirExprId,
    worker: HirDeclId,
) {
    let Some(declaration) = graph.declaration(worker) else {
        return;
    };
    let Some(signature) = super::calls::source_function_signature(graph, worker) else {
        return;
    };
    let call_span = expression_span(body, worker_call);
    for parameter in &signature.parameters {
        if let Some((path, kind)) = rejection_path(&parameter.type_fact, "parameter") {
            validation.diagnostics.push(value_diagnostic(
                &declaration.name,
                format!("{path} `{}`", parameter.name),
                kind,
                parameter.declaration_span.unwrap_or(call_span),
            ));
        }
    }
    if let Some(placement) = validation.calls.get(&worker_call)
        && let Some(slots) = &placement.parameter_slots
    {
        for slot in slots {
            let CallParameterSlotValueFact::Explicit {
                value: Some(value), ..
            } = &slot.value
            else {
                continue;
            };
            let Some(fact) = facts.expression(*value) else {
                continue;
            };
            if let Some((path, kind)) = rejection_path(fact, "argument") {
                validation.diagnostics.push(value_diagnostic(
                    &declaration.name,
                    format!("{path} for parameter `{}`", slot.name),
                    kind,
                    expression_span(body, *value),
                ));
            }
        }
    }
    if let Some((path, kind)) = rejection_path(&signature.returns, "return value") {
        validation
            .diagnostics
            .push(value_diagnostic(&declaration.name, path, kind, call_span));
    }
}

fn validate_continuation(
    validation: &mut ExecutableValidationFacts,
    graph: &ModuleGraph,
    worker: HirDeclId,
    continuation: HirDeclId,
    continuation_expression: HirExprId,
    fallback_span: Span,
) {
    let Some(worker_signature) = graph.function_signature(worker) else {
        return;
    };
    let Some(worker_declaration) = graph.declaration(worker) else {
        return;
    };
    let Some(continuation_declaration) = graph.declaration(continuation) else {
        return;
    };
    let Some(continuation_signature) = super::calls::source_function_signature(graph, continuation)
    else {
        return;
    };
    let (worker_return, worker_return_name) = worker_signature.return_type.as_ref().map_or_else(
        || (TypeFact::Any, "Any".to_owned()),
        |hint| {
            (
                crate::hints::type_fact_from_hint_in_module(graph, worker_declaration.module, hint),
                hint.display(),
            )
        },
    );
    let expected = TypeFact::result(worker_return, TypeFact::record("task::Error"));
    let expected_name = format!("Result<{worker_return_name}, task::Error>");
    let first = continuation_signature.parameters.first();
    let valid = first.is_some_and(|parameter| {
        parameter.requirement == CallableParameterRequirementFact::Required
            && parameter.type_fact == expected
    });
    if valid {
        return;
    }
    let span = graph
        .expression_span(continuation_expression)
        .unwrap_or(fallback_span);
    let mut diagnostic = Diagnostic::error(format!(
        "task continuation `{}` must accept `{}` as its first required parameter",
        continuation_declaration.name, expected_name
    ))
    .with_code("analysis::task_continuation_invalid")
    .with_span(span)
    .with_label(
        continuation_declaration.name_span,
        "continuation is declared here",
    );
    if let Some(parameter) = first
        && let Some(parameter_span) = parameter.declaration_span
    {
        diagnostic = diagnostic.with_label(
            parameter_span,
            format!(
                "found first parameter type `{}`",
                parameter.type_fact.display_name()
            ),
        );
    }
    validation.diagnostics.push(diagnostic);
}

fn value_diagnostic(
    target: &str,
    path: String,
    kind: NonDetachableValueKind,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(format!(
        "detached target `{target}` rejects {path}: {} is not detachable",
        kind.as_str()
    ))
    .with_code("analysis::task_value_not_detachable")
    .with_span(span)
    .with_label(span, format!("rejected value path: {path}"))
}

fn expression_span(body: &HirBody, expression: HirExprId) -> Span {
    body.expression(expression)
        .map_or(body.origin.span, |expression| expression.origin.span)
}

fn rejection_path(fact: &TypeFact, root: &'static str) -> Option<(String, NonDetachableValueKind)> {
    let mut path = root.to_owned();
    let kind = rejection_path_into(fact, &mut path)?;
    Some((path, kind))
}

fn rejection_path_into(fact: &TypeFact, path: &mut String) -> Option<NonDetachableValueKind> {
    match fact {
        TypeFact::Array { element } | TypeFact::Set { element } => {
            descend(element, path, ".element")
        }
        TypeFact::Option { some } | TypeFact::OptionSome { some } => descend(some, path, ".some"),
        TypeFact::ResultOk { ok } => descend(ok, path, ".ok"),
        TypeFact::ResultErr { err } => descend(err, path, ".err"),
        TypeFact::Map { key, value } => {
            descend(key, path, ".key").or_else(|| descend(value, path, ".value"))
        }
        TypeFact::Result { ok, err } => {
            descend(ok, path, ".ok").or_else(|| descend(err, path, ".err"))
        }
        TypeFact::Tuple { elements } | TypeFact::Union(elements) => elements
            .iter()
            .enumerate()
            .find_map(|(index, element)| descend(element, path, &format!("[{index}]"))),
        _ => fact.detachability().rejection(),
    }
}

fn descend(fact: &TypeFact, path: &mut String, suffix: &str) -> Option<NonDetachableValueKind> {
    let start = path.len();
    path.push_str(suffix);
    let rejection = rejection_path_into(fact, path);
    if rejection.is_none() {
        path.truncate(start);
    }
    rejection
}
