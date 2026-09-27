//! The common CST to HIR conversion for declaration and editor type hints.
use super::HirTypeHint;
use vela_common::{SourceId, Span};
use vela_syntax::ast::{AstNode, SyntaxTypeHint};

/// Lower a parsed hint without introducing a second parser or type resolver.
pub fn lower_syntax_type_hint(source: SourceId, hint: &SyntaxTypeHint) -> HirTypeHint {
    let span = Span::new(
        source,
        hint.syntax().text_range().start().into(),
        hint.syntax().text_range().end().into(),
    );
    if hint.is_unit() {
        return HirTypeHint {
            path: vec![HirTypeHint::UNIT_PATH.to_owned()],
            args: Vec::new(),
            span,
        };
    }

    let tuple_elements = hint.tuple_element_hints().collect::<Vec<_>>();
    if hint.is_tuple() {
        return HirTypeHint {
            path: vec![HirTypeHint::UNIT_PATH.to_owned()],
            args: tuple_elements
                .iter()
                .map(|arg| lower_syntax_type_hint(source, arg))
                .collect(),
            span,
        };
    }

    if hint.l_paren_token().is_some() && tuple_elements.len() == 1 {
        return lower_syntax_type_hint(source, &tuple_elements[0]);
    }

    HirTypeHint {
        path: hint.path_segments(),
        args: hint
            .type_arg_list()
            .into_iter()
            .flat_map(|args| args.type_hints())
            .map(|arg| lower_syntax_type_hint(source, &arg))
            .collect(),
        span,
    }
}
