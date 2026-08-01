use vela_common::{CallableAsyncness, Diagnostic, Span};

use super::{DeclarationKind, ModuleGraph};
use crate::binding::{BindingMap, BindingResolution, TaskLexicalCapability};
use crate::ids::HirExprId;

pub(super) fn validate_once(graph: &mut ModuleGraph) {
    if graph.task_references_validated {
        return;
    }
    graph.task_references_validated = true;
    let diagnostics = graph
        .bindings
        .values()
        .chain(graph.trait_default_method_bindings.values())
        .chain(graph.impl_method_bindings.values())
        .flat_map(|bindings| binding_diagnostics(graph, bindings))
        .collect::<Vec<_>>();
    graph.diagnostics.extend(diagnostics);
}

fn binding_diagnostics(graph: &ModuleGraph, bindings: &BindingMap) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (callee, capability) in bindings.task_capabilities() {
        let Some((body, call)) = graph.bodies.values().find_map(|body| {
            body.calls()
                .find(|(_, call)| call.callee == callee)
                .map(|(_, call)| (body, call))
        }) else {
            continue;
        };
        let Some(worker) = call
            .arguments
            .first()
            .and_then(|argument| argument.value)
            .and_then(|expression| body.call(expression))
        else {
            continue;
        };
        validate_target(
            graph,
            bindings,
            worker.callee,
            call.arguments[0].origin.span,
            TargetRole::Worker,
            &mut diagnostics,
        );
        if capability == TaskLexicalCapability::SpawnScopedThen
            && let Some(continuation) = call.arguments.get(1).and_then(|argument| argument.value)
        {
            validate_target(
                graph,
                bindings,
                continuation,
                call.arguments[1].origin.span,
                TargetRole::Continuation,
                &mut diagnostics,
            );
        }
    }
    diagnostics
}

#[derive(Clone, Copy)]
enum TargetRole {
    Worker,
    Continuation,
}

fn validate_target(
    graph: &ModuleGraph,
    bindings: &BindingMap,
    expression: HirExprId,
    span: Span,
    role: TargetRole,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(BindingResolution::Declaration(declaration)) = bindings.resolution(expression) else {
        return;
    };
    let Some(declaration) = graph.declaration(*declaration) else {
        return;
    };
    if declaration.kind != DeclarationKind::Function {
        diagnostics.push(
            Diagnostic::error(match role {
                TargetRole::Worker => "detached worker target must be a declared function",
                TargetRole::Continuation => "task continuation target must be a declared function",
            })
            .with_code(match role {
                TargetRole::Worker => "hir::task_worker_not_function",
                TargetRole::Continuation => "hir::task_continuation_not_function",
            })
            .with_span(span)
            .with_label(declaration.name_span, "target is declared here"),
        );
        return;
    }
    let Some(signature) = graph.function_signature(declaration.id) else {
        return;
    };
    let invalid_asyncness = match role {
        TargetRole::Worker => signature.asyncness != CallableAsyncness::Async,
        TargetRole::Continuation => signature.asyncness != CallableAsyncness::Sync,
    };
    if invalid_asyncness {
        diagnostics.push(
            Diagnostic::error(match role {
                TargetRole::Worker => "detached worker must be declared `async fn`",
                TargetRole::Continuation => "task continuation must be synchronous",
            })
            .with_code(match role {
                TargetRole::Worker => "hir::task_worker_not_async",
                TargetRole::Continuation => "hir::task_continuation_async",
            })
            .with_span(span)
            .with_label(declaration.name_span, "target is declared here"),
        );
    }
}
