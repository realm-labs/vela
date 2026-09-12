use super::QueryContext;
use crate::{LanguageServiceDatabases, callable_context::CallableFacts};
use vela_hir::module_graph::DeclarationKind;
use vela_hir::{body::HirBody, ids::HirExprId};
use vela_syntax::ast::{AstNode, SyntaxCallExpr};

impl<'a> QueryContext<'a> {
    pub(crate) fn named_callable_facts_by_path(
        &self,
        databases: &LanguageServiceDatabases,
        path: &[String],
    ) -> Vec<CallableFacts> {
        if let Some(callables) = self.service_callable_facts(databases, path) {
            return callables;
        }
        let source = self.source_callable_facts_by_path(databases, path);
        let graph = databases.hir_db().graph();
        let source_owned = self
            .module_key()
            .and_then(|key| graph.module_id(key))
            .is_some_and(|module| {
                graph
                    .resolve_visible_declaration_path(module, path, DeclarationKind::Function)
                    .is_some()
            });
        if source_owned {
            return source;
        }
        let Some(first) = path.first() else {
            return Vec::new();
        };
        let imports = self
            .module_key()
            .and_then(|key| graph.module_id(key))
            .and_then(|module| graph.imports(module));
        let mut matches = imports
            .into_iter()
            .flatten()
            .filter(|import| import.alias.as_ref().or_else(|| import.path.last()) == Some(first));
        let imported = matches.next().map(|import| {
            import
                .path
                .iter()
                .chain(&path[1..])
                .cloned()
                .collect::<Vec<_>>()
        });
        if matches.next().is_some() {
            return Vec::new();
        }
        let external = imported.as_deref().unwrap_or(path);
        crate::callable_context::named_external_callable_facts(
            databases.schema_db().facts(),
            &external.join("::"),
        )
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
