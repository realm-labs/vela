use vela_common::{SourceId, Span};
use vela_syntax::ast::{AstNode, SyntaxExpression};

use super::CompilerExpressionPayload;

impl<'ast> CompilerExpressionPayload<'ast> {
    pub(in crate::compiler) fn from_syntax(
        source: Option<SourceId>,
        syntax: Option<SyntaxExpression>,
    ) -> Self {
        Self {
            source,
            syntax,
            _ast: std::marker::PhantomData,
        }
    }

    pub(in crate::compiler) fn syntax_span(&self) -> Option<Span> {
        Some(syntax_expression_span(self.source?, self.syntax.as_ref()?))
    }

    pub(in crate::compiler) fn syntax_path_segments(&self) -> Option<Vec<String>> {
        self.source?;
        let segments = self.syntax.as_ref()?.as_path()?.path_segments();
        (!segments.is_empty()).then_some(segments)
    }

    pub(in crate::compiler) fn syntax_is_self(&self) -> bool {
        if self.source.is_none() {
            return false;
        }
        self.syntax
            .as_ref()
            .and_then(SyntaxExpression::as_path)
            .is_some_and(|path| path.is_self())
    }

    pub(in crate::compiler) fn syntax_record_path_segments(&self) -> Option<Vec<String>> {
        self.source?;
        let segments = self.syntax.as_ref()?.as_record()?.path_segments();
        (!segments.is_empty()).then_some(segments)
    }

    pub(in crate::compiler) fn syntax_call_callee_path_segments(&self) -> Option<Vec<String>> {
        self.source?;
        let callee = self.syntax.as_ref()?.as_call()?.callee()?;
        let segments = callee.as_path()?.path_segments();
        (!segments.is_empty()).then_some(segments)
    }

    pub(in crate::compiler) fn syntax_call_callee_span(&self) -> Option<Span> {
        Some(syntax_expression_span(
            self.source?,
            &self.syntax.as_ref()?.as_call()?.callee()?,
        ))
    }
}

fn syntax_expression_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}
