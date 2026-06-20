use vela_common::{Diagnostic, SourceId, Span};

use crate::TextRange;
use crate::ast::{AstNode, SyntaxSourceFile, SyntaxTypeHint};

pub(crate) fn validate_source(source: SourceId, tree: &SyntaxSourceFile) -> Vec<Diagnostic> {
    tree.syntax()
        .descendants()
        .filter_map(SyntaxTypeHint::cast)
        .flat_map(|hint| validate_type_hint(source, &hint))
        .collect()
}

fn validate_type_hint(source: SourceId, hint: &SyntaxTypeHint) -> Vec<Diagnostic> {
    let Some(args) = hint.type_arg_list() else {
        return Vec::new();
    };
    let path = hint.path_segments();
    let arg_hints = args.type_hints().collect::<Vec<_>>();
    let Some(contract) = type_argument_contract(&path) else {
        let span = args.less_token().map_or_else(
            || span_for(source, args.syntax().text_range()),
            |token| span_for(source, token.text_range()),
        );
        return vec![
            Diagnostic::error(
                "only builtin container, Option, and Result type hints support type arguments",
            )
            .with_code("syntax::generic_type_hint")
            .with_span(span)
            .with_label(
                span,
                "use a builtin parameterized type hint or remove these type arguments",
            ),
        ];
    };

    let expected = contract.arity();
    if arg_hints.len() != expected {
        let span = span_for(source, args.syntax().text_range());
        return vec![
            Diagnostic::error(format!(
                "`{}` expects {expected} type argument{}",
                path.join("::"),
                if expected == 1 { "" } else { "s" }
            ))
            .with_code("syntax::type_argument_arity")
            .with_span(span)
            .with_label(span, "wrong number of type arguments"),
        ];
    }

    if matches!(contract, TypeArgumentContract::KeyedMap) && !is_keyable_type_hint(&arg_hints[0]) {
        let span = span_for(source, arg_hints[0].syntax().text_range());
        return vec![
            Diagnostic::error("`Map` key type hints require a keyable type")
                .with_code("syntax::map_key_type_argument")
                .with_span(span)
                .with_label(
                    span,
                    "use a ValueKey-supported key type or an unparameterized Map",
                ),
        ];
    }

    if matches!(contract, TypeArgumentContract::KeyedSet) && !is_keyable_type_hint(&arg_hints[0]) {
        let span = span_for(source, arg_hints[0].syntax().text_range());
        return vec![
            Diagnostic::error("`Set` type hints require a keyable element type")
                .with_code("syntax::set_element_type_argument")
                .with_span(span)
                .with_label(
                    span,
                    "use a ValueKey-supported element type or an unparameterized Set",
                ),
        ];
    }

    Vec::new()
}

#[derive(Clone, Copy)]
enum TypeArgumentContract {
    FixedArity(usize),
    KeyedMap,
    KeyedSet,
}

impl TypeArgumentContract {
    const fn arity(self) -> usize {
        match self {
            Self::FixedArity(arity) => arity,
            Self::KeyedMap => 2,
            Self::KeyedSet => 1,
        }
    }
}

fn type_argument_contract(path: &[String]) -> Option<TypeArgumentContract> {
    match path {
        [name] if name == "Array" => Some(TypeArgumentContract::FixedArity(1)),
        [name] if name == "Set" => Some(TypeArgumentContract::KeyedSet),
        [name] if name == "Map" => Some(TypeArgumentContract::KeyedMap),
        [name] if name == "Iterator" => Some(TypeArgumentContract::FixedArity(1)),
        [name] if name == "Option" => Some(TypeArgumentContract::FixedArity(1)),
        [name] if name == "Result" => Some(TypeArgumentContract::FixedArity(2)),
        _ => None,
    }
}

fn is_keyable_type_hint(hint: &SyntaxTypeHint) -> bool {
    match hint.path_segments().as_slice() {
        [name]
            if matches!(
                name.as_str(),
                "Range" | "Function" | "PathProxy" | "path_proxy"
            ) =>
        {
            false
        }
        [_] => true,
        _ => hint.type_arg_list().is_none(),
    }
}

fn span_for(source: SourceId, range: TextRange) -> Span {
    Span::new(source, range.start().into(), range.end().into())
}
