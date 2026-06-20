use vela_syntax::ast::{
    AstNode, SyntaxPattern, SyntaxPatternKind, SyntaxRecordPatternField, SyntaxSourceFile,
};
use vela_syntax::{SyntaxNode, SyntaxToken, TextRange as SyntaxTextRange, TextSize, TokenAtOffset};

pub(super) fn is_pattern_context(source: &SyntaxSourceFile, offset: usize) -> bool {
    let Some(offset) = syntax_offset(offset) else {
        return false;
    };
    let Some(token) = significant_token_at(source.syntax(), offset) else {
        return false;
    };
    token
        .parent_ancestors()
        .any(|node| pattern_node_contains_offset(&node, offset))
}

fn pattern_node_contains_offset(node: &SyntaxNode, offset: TextSize) -> bool {
    if let Some(pattern) = SyntaxPattern::cast(node.clone()) {
        return pattern.pattern_kind().is_some_and(|kind| {
            !matches!(
                kind,
                SyntaxPatternKind::Wildcard | SyntaxPatternKind::Literal
            ) && range_contains_offset(pattern.syntax().text_range(), offset)
        });
    }

    if let Some(field) = SyntaxRecordPatternField::cast(node.clone()) {
        return range_contains_offset(field.syntax().text_range(), offset);
    }

    false
}

fn significant_token_at(root: &SyntaxNode, offset: TextSize) -> Option<SyntaxToken> {
    match root.token_at_offset(offset) {
        TokenAtOffset::None => None,
        TokenAtOffset::Single(token) => non_trivia_token(token),
        TokenAtOffset::Between(left, right) => {
            non_trivia_token(left).or_else(|| non_trivia_token(right))
        }
    }
}

fn non_trivia_token(token: SyntaxToken) -> Option<SyntaxToken> {
    (!token.kind().is_trivia()).then_some(token)
}

fn range_contains_offset(range: SyntaxTextRange, offset: TextSize) -> bool {
    range.start() <= offset && offset <= range.end()
}

fn syntax_offset(offset: usize) -> Option<TextSize> {
    let offset = u32::try_from(offset).ok()?;
    Some(TextSize::from(offset))
}
