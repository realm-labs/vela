use vela_hir::module_graph::{DeclarationKind, Visibility};
use vela_syntax::ast::{AstNode, SyntaxTypeHint};

use crate::{LanguageServiceDatabases, QueryContext, symbol_target::SymbolTarget};

use super::Definition;

impl LanguageServiceDatabases {
    /// A recognized type reference owns navigation even when it is unresolved.
    /// In particular, a same-named value must not supply a fallback target.
    pub(super) fn source_type_hint_definition(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
    ) -> Option<Option<Definition>> {
        let tree = query.syntax_parse()?.tree();
        for hint in tree.syntax().descendants().filter_map(SyntaxTypeHint::cast) {
            let tokens = hint.path_tokens();
            let Some(index) = tokens.iter().position(|token| {
                usize::from(token.text_range().start()) == target.range().start
                    && usize::from(token.text_range().end()) == target.range().end
            }) else {
                continue;
            };
            if index + 1 != tokens.len() {
                return Some(None);
            }
            let graph = self.hir_db().graph();
            let definition = query
                .module_key()
                .and_then(|key| graph.module_id(key))
                .and_then(|module| {
                    let path = graph.expand_import_path(module, &hint.path_segments())?;
                    [
                        DeclarationKind::Struct,
                        DeclarationKind::Enum,
                        DeclarationKind::Trait,
                    ]
                    .into_iter()
                    .find_map(|kind| {
                        graph
                            .declaration_by_type_path(&path, query.module_key()?, kind)
                            .filter(|declaration| {
                                declaration.module == module
                                    || declaration.visibility == Visibility::Public
                            })
                    })
                })
                .and_then(|declaration| self.definition_from_declaration(declaration));
            // Schema navigation retains its source-span contract.
            return Some(definition.or_else(|| {
                target
                    .is_schema_symbol()
                    .then(|| self.schema_definition_for_target(target))
                    .flatten()
            }));
        }
        None
    }
}
