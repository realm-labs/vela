use vela_common::Diagnostic;
use vela_hir::binding::BindingResolution;
use vela_hir::body::{HirBody, HirExprKind};
use vela_hir::ids::HirExprId;
use vela_hir::module_graph::ModuleGraph;
use vela_hir::type_hint::StateStorage;

use super::ExecutableValidationFacts;

pub(super) fn record_body(
    validation: &mut ExecutableValidationFacts,
    graph: &ModuleGraph,
    body: &HirBody,
) {
    let Some(bindings) = graph.bindings_for_body(body.id) else {
        return;
    };
    for expression in body.expressions.values() {
        let HirExprKind::Assign {
            target: Some(target),
            ..
        } = expression.kind
        else {
            continue;
        };
        let target = assignment_root(body, target);
        let Some(BindingResolution::Declaration(declaration)) = bindings.resolution(target) else {
            continue;
        };
        let Some(metadata) = graph.state_metadata(*declaration) else {
            continue;
        };
        if metadata.storage != StateStorage::Extern {
            continue;
        }
        let target_span = body
            .expression(target)
            .map_or(expression.origin.span, |target| target.origin.span);
        let declaration_span = graph
            .declaration(*declaration)
            .map_or(metadata.type_hint.span, |declaration| declaration.span);
        validation.diagnostics.push(
            Diagnostic::error("cannot assign directly to `extern state`")
                .with_code("analysis::extern_state_assignment")
                .with_span(expression.origin.span)
                .with_label(target_span, "this state is owned by the host")
                .with_label(
                    declaration_span,
                    "declared as `extern state`; mutate host fields through HostAccess instead",
                ),
        );
    }
}

fn assignment_root(body: &HirBody, mut expression: HirExprId) -> HirExprId {
    while let Some(HirExprKind::Paren {
        expression: Some(inner),
    }) = body.expression(expression).map(|value| &value.kind)
    {
        expression = *inner;
    }
    expression
}
