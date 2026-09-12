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

    /// Semantic parameter ownership, separate from the CST argument ordinal.
    pub(crate) fn call_parameter_index(&self, callable: &CallableFacts) -> Option<usize> {
        let expression = self.syntax_call()?;
        let ordinal = self.call_active_parameter_index()?;
        let separators = expression.separator_tokens();
        let arguments = expression.arguments();
        let argument_ordinal = |argument: &vela_syntax::ast::SyntaxArgument| {
            separators.partition_point(|separator| {
                separator.text_range().end() <= argument.syntax().text_range().start()
            })
        };
        let current = arguments
            .iter()
            .find(|argument| argument_ordinal(argument) == ordinal);
        let named_index = |name: &str| {
            callable
                .supports_named_arguments()
                .then(|| {
                    callable
                        .params()
                        .iter()
                        .position(|parameter| parameter.name() == name)
                })
                .flatten()
        };
        let mut occupied = vec![false; callable.params().len()];
        let positional_slots = arguments
            .iter()
            .find(|argument| {
                argument_ordinal(argument) < ordinal && argument.name_token().is_some()
            })
            .map_or(ordinal, &argument_ordinal);
        // Missing expressions before separators still reserve their positional
        // slots. Recovery must not shift later arguments into those holes.
        let positional_slots_in_range = positional_slots.min(occupied.len());
        occupied[..positional_slots_in_range].fill(true);
        let seen_named = positional_slots < ordinal;
        for argument in arguments
            .iter()
            .filter(|argument| argument_ordinal(argument) < ordinal)
        {
            let index = argument.name_text().and_then(|name| named_index(&name));
            if let Some(slot) = index.and_then(|index| occupied.get_mut(index)) {
                *slot = true;
            }
        }
        if let Some(name) = current.and_then(|argument| argument.name_text()) {
            let index = named_index(&name)?;
            return (!occupied[index]).then_some(index);
        }
        // An empty slot is still an authoring opportunity. Reserve names to its
        // right too, as parameter-name completion does. A written expression is
        // positional and must obey the compiler's positional-before-named rule.
        let empty = current.is_none_or(|argument| argument.expression().is_none());
        if empty && callable.supports_named_arguments() {
            for argument in arguments
                .iter()
                .filter(|argument| argument_ordinal(argument) > ordinal)
            {
                if let Some(index) = argument.name_text().and_then(|name| named_index(&name)) {
                    occupied[index] = true;
                }
            }
            return occupied.iter().position(|used| !used);
        }
        (!seen_named && ordinal < occupied.len() && !occupied[ordinal]).then_some(ordinal)
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
