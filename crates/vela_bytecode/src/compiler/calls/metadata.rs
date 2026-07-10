use vela_common::Span;
use vela_hir::type_hint::{HirTypeHint, ParamHint};
use vela_registry::{ParamDef, TypeHintDef};

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
            default_body: None,
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
