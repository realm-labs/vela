use vela_syntax::ast::{AstNode, SyntaxSourceFile, SyntaxTupleFieldList};

/// Tuple variant entries are parameters (`name: Type`), not bare type hints.
pub(super) fn is_tuple_field_name(tree: &SyntaxSourceFile, offset: usize) -> bool {
    tree.syntax()
        .descendants()
        .filter_map(SyntaxTupleFieldList::cast)
        .any(|list| {
            let range = list.syntax().text_range();
            if offset < usize::from(range.start()) || offset > usize::from(range.end()) {
                return false;
            }
            list.params().any(|param| {
                param.name_token().is_some_and(|name| {
                    let range = name.text_range();
                    usize::from(range.start()) <= offset && offset <= usize::from(range.end())
                })
            }) || list
                .syntax()
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .filter(|token| {
                    !token.kind().is_trivia() && usize::from(token.text_range().end()) <= offset
                })
                .last()
                .is_some_and(|token| {
                    token.parent().as_ref() == Some(list.syntax())
                        && matches!(
                            token.kind(),
                            vela_syntax::SyntaxKind::LParen | vela_syntax::SyntaxKind::Comma
                        )
                })
        })
        || incomplete_tuple_boundary(tree, offset)
}

// An unclosed enum can leave its variant tokens directly under EnumItem.
// Recover only a top-level tuple separator, never a nested default call.
fn incomplete_tuple_boundary(tree: &SyntaxSourceFile, offset: usize) -> bool {
    use vela_syntax::SyntaxKind as K;
    tree.syntax()
        .descendants()
        .filter(|node| node.kind() == K::EnumItem)
        .filter(|node| {
            usize::from(node.text_range().start()) <= offset
                && offset <= usize::from(node.text_range().end())
        })
        .any(|node| {
            let mut stack = Vec::new();
            let mut previous = None;
            let mut tuple = false;
            for token in node
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .filter(|token| {
                    !token.kind().is_trivia() && usize::from(token.text_range().end()) <= offset
                })
            {
                let kind = token.kind();
                match kind {
                    K::LParen => {
                        if stack == [K::LBrace] {
                            tuple = previous == Some(K::Ident);
                        }
                        stack.push(kind);
                    }
                    K::LBrace | K::LBracket => stack.push(kind),
                    K::RParen | K::RBrace | K::RBracket => {
                        stack.pop();
                    }
                    _ => {}
                }
                previous = Some(kind);
            }
            tuple
                && stack == [K::LBrace, K::LParen]
                && matches!(previous, Some(K::LParen | K::Comma))
        })
}

#[cfg(test)]
mod tests {
    use crate::{CursorContextKind, LineIndex, cursor_context_at};
    use vela_syntax::ast::AstNode;

    #[test]
    fn tuple_declaration_boundaries_preserve_names_types_and_default_expressions() {
        for crlf in [false, true] {
            for (source, expected) in [
                ("enum E { Value(|) }", CursorContextKind::RecordTypeField),
                (
                    "enum E { Value( /* 中😀 */ |) }",
                    CursorContextKind::RecordTypeField,
                ),
                (
                    "enum E { Value(first: i64, |) }",
                    CursorContextKind::RecordTypeField,
                ),
                (
                    "enum E { Value(first: i64, na|me: i64) }",
                    CursorContextKind::RecordTypeField,
                ),
                (
                    "enum E { Value(first: i64, |",
                    CursorContextKind::RecordTypeField,
                ),
                ("enum E { Value(name: |) }", CursorContextKind::Type),
                ("enum E { Value(name: i6|4 = 1) }", CursorContextKind::Type),
                (
                    "enum E { Value(name: api::Ty|pe) }",
                    CursorContextKind::ModulePath,
                ),
            ] {
                let source = format!("/* 中😀 */\n{source}");
                let source = if crlf {
                    source.replace('\n', "\r\n")
                } else {
                    source
                };
                let offset = source.find('|').expect("enum fixture invariant");
                let source = source.replace('|', "");
                let parse = vela_syntax::parse::parse_source(&source);
                let cursor = cursor_context_at(
                    &source,
                    LineIndex::new(&source).position(offset),
                    Some(&parse),
                );
                assert_eq!(
                    cursor.kind(),
                    expected,
                    "{source}: {:#?}",
                    parse.tree().syntax()
                );
            }
            for source in [
                "enum E { Value(name = va|lue) }",
                "enum E { Value(name: i64 = va|lue) }",
                "enum E { Value(name = make(va|lue)) }",
                "fn f() { E::Value(va|lue); }",
            ] {
                let offset = source.find('|').expect("enum fixture invariant");
                let source = source.replace('|', "");
                let parse = vela_syntax::parse::parse_source(&source);
                let cursor = cursor_context_at(
                    &source,
                    LineIndex::new(&source).position(offset),
                    Some(&parse),
                );
                assert!(
                    !matches!(
                        cursor.kind(),
                        CursorContextKind::RecordTypeField | CursorContextKind::Type
                    ),
                    "{source}: {:?}",
                    cursor.kind()
                );
            }
        }
    }
}
