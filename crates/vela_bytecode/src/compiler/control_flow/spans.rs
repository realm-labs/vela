use vela_common::{SourceId, Span};
use vela_syntax::ast::{AstNode, SyntaxExpression};

pub(super) fn syntax_expression_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}
