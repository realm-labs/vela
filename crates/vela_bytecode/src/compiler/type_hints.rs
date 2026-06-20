use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::TypeHint;

pub(super) fn hir_type_hint_from_syntax(hint: &TypeHint) -> HirTypeHint {
    HirTypeHint {
        path: hint.path.clone(),
        args: hint.args.iter().map(hir_type_hint_from_syntax).collect(),
        span: hint.span,
    }
}
