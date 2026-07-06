use vela_common::{Diagnostic, Span};
use vela_hir::type_hint::{HirTypeHint, ParamHint};
use vela_registry::{ParamDef, TypeHintDef};

use crate::compiler::{CompileError, CompileErrorKind};

pub(in crate::compiler) fn registry_param_hints(
    params: &[ParamDef],
    call_span: Span,
) -> Vec<ParamHint> {
    params
        .iter()
        .map(|param| ParamHint {
            name: param.name.clone(),
            span: call_span,
            type_hint: param
                .type_hint
                .as_ref()
                .map(|hint| registry_type_hint(hint, call_span)),
            default_value_span: param.has_default.then_some(call_span),
        })
        .collect()
}

pub(in crate::compiler) fn registry_type_hint(hint: &TypeHintDef, span: Span) -> HirTypeHint {
    HirTypeHint {
        path: hint.path.clone(),
        args: hint
            .args
            .iter()
            .map(|arg| registry_type_hint(arg, span))
            .collect(),
        span,
    }
}

pub(in crate::compiler) fn unresolved_static_method_error(
    method: &str,
    span: Span,
) -> CompileError {
    CompileError::new(CompileErrorKind::SemanticDiagnostics(vec![
        Diagnostic::error(format!("unresolved method `{method}`"))
            .with_code("compiler::unresolved_method")
            .with_span(span)
            .with_label(span, "method is not defined for the known receiver type"),
    ]))
}
