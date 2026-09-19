use vela_syntax::SyntaxKind as K;
use vela_syntax::ast::{AstNode, SyntaxTypeArgList, SyntaxTypeHint};

use super::TypeLocation;
use crate::QueryContext;

pub(super) fn type_location(query: &QueryContext<'_>, offset: usize) -> TypeLocation {
    let Some(parse) = query.syntax_parse() else {
        return TypeLocation::Other;
    };
    let source = parse.tree();
    if let Some(arguments) = source
        .syntax()
        .descendants()
        .filter_map(SyntaxTypeArgList::cast)
        .filter(|arguments| {
            let start = arguments
                .less_token()
                .map(|token| usize::from(token.text_range().end()));
            let end = arguments.greater_token().map_or(
                usize::from(arguments.syntax().text_range().end()),
                |token| usize::from(token.text_range().start()),
            );
            start.is_some_and(|start| start <= offset && offset <= end)
        })
        .min_by_key(|arguments| arguments.syntax().text_range().len())
    {
        return argument_location(&arguments, offset).unwrap_or(TypeLocation::Other);
    }
    let hint = parse
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxTypeHint::cast)
        .filter(|hint| {
            let range = hint.syntax().text_range();
            usize::from(range.start()) <= offset && offset <= usize::from(range.end())
        })
        .min_by_key(|hint| hint.syntax().text_range().len());
    let Some(hint) = hint else {
        return TypeLocation::Other;
    };
    for ancestor in hint.syntax().ancestors().skip(1) {
        match ancestor.kind() {
            K::Param => {
                return if ancestor
                    .parent()
                    .is_some_and(|parent| parent.kind() == K::TupleFieldList)
                {
                    TypeLocation::StructField
                } else {
                    TypeLocation::Parameter
                };
            }
            K::StructField => return TypeLocation::StructField,
            K::FunctionItem | K::TraitMethod | K::ImplMethod => return TypeLocation::Return,
            K::LetStmt | K::StateItem | K::ConstItem => return TypeLocation::Other,
            _ => {}
        }
    }
    TypeLocation::Other
}

fn argument_location(arguments: &SyntaxTypeArgList, offset: usize) -> Option<TypeLocation> {
    let container = arguments
        .syntax()
        .parent()
        .and_then(SyntaxTypeHint::cast)?
        .path_text()?;
    if !matches!(
        container.as_str(),
        "Array" | "Set" | "Map" | "Iterator" | "Option" | "Result"
    ) {
        return None;
    }
    let argument_index = arguments
        .separator_tokens()
        .iter()
        .filter(|token| usize::from(token.text_range().end()) <= offset)
        .count();
    Some(TypeLocation::BuiltinTypeArgument {
        container,
        argument_index,
    })
}
