use super::QueryContext;
use crate::LanguageServiceDatabases;
use crate::callable_context::CallableFacts;
use vela_hir::{body::HirBody, ids::HirExprId};
use vela_syntax::ast::{AstNode, SyntaxCallExpr};

impl<'a> QueryContext<'a> {
    pub(crate) fn call_target_facts(
        &self,
        databases: &LanguageServiceDatabases,
    ) -> Vec<CallableFacts> {
        let Some(call) = self.call_argument_facts() else {
            return Vec::new();
        };
        let mut callables = if let (Some(receiver), Some(method)) =
            (call.member_receiver(), call.member_method())
        {
            self.member_callable_facts(databases, receiver, method, call.args_prefix())
        } else if let Some(path) = call.callee_path() {
            self.callable_facts_by_path(databases, path)
        } else {
            self.callable_facts(databases, call.callee())
        };
        callables.sort_by_key(|callable| callable.origin());
        callables
    }

    pub(crate) fn syntax_call(&self) -> Option<SyntaxCallExpr> {
        let open = self.call_open_offset()?;
        let snapshot_parse;
        let parse = if let Some(parse) = self.syntax_parse() {
            parse
        } else {
            snapshot_parse = vela_syntax::parse::parse_source(self.text());
            &snapshot_parse
        };
        parse
            .tree()
            .syntax()
            .descendants()
            .filter_map(SyntaxCallExpr::cast)
            .find(|expression| {
                expression
                    .l_paren_token()
                    .is_some_and(|token| usize::from(token.text_range().start()) == open)
            })
    }

    pub(super) fn hir_call_for_cursor(&self) -> Option<(&'a HirBody, HirExprId)> {
        // Recovery spans can end before trailing whitespace or a missing argument.
        // Match the actual callee instead of requiring the cursor inside the HIR call.
        let callee = self.syntax_call()?.callee()?.syntax().text_range();
        let source = self.source_id()?;
        let graph = self.graph?;
        let body = self
            .body
            .or_else(|| graph.body_containing_offset(source, u32::from(callee.start())))?;
        graph.body_and_ancestors(body.id).find_map(|body| {
            body.calls().find_map(|(_, call)| {
                let origin = body.expressions.get(&call.callee)?.origin;
                (origin.source == source
                    && origin.span.start == u32::from(callee.start())
                    && origin.span.end == u32::from(callee.end()))
                .then_some((body, call.expression))
            })
        })
    }
}
