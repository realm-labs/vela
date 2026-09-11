use vela_hir::module_graph::{DeclarationKind, Visibility};
use vela_syntax::ast::{AstNode, SyntaxImplItem};

use super::Definition;
use crate::{LanguageServiceDatabases, QueryContext, symbol_target::SymbolTarget};

impl LanguageServiceDatabases {
    pub(super) fn impl_header_definition(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
    ) -> Option<Option<Definition>> {
        let graph = self.hir_db().graph();
        let module = graph.module_id(query.module_key()?)?;
        for item in query
            .syntax_parse()?
            .tree()
            .syntax()
            .descendants()
            .filter_map(SyntaxImplItem::cast)
        {
            for (tokens, path, kind) in [
                (
                    item.trait_path_tokens(),
                    item.trait_path_segments(),
                    DeclarationKind::Trait,
                ),
                (
                    item.target_path_tokens(),
                    item.target_path_segments(),
                    DeclarationKind::Struct,
                ),
            ] {
                let Some(index) = tokens.iter().position(|token| {
                    usize::from(token.text_range().start()) == target.range().start
                        && usize::from(token.text_range().end()) == target.range().end
                }) else {
                    continue;
                };
                if index + 1 != tokens.len() {
                    return Some(None);
                }
                if let Some(declaration) =
                    graph.resolve_visible_declaration_path(module, &path, kind)
                {
                    return Some(
                        (declaration.module == module
                            || declaration.visibility == Visibility::Public)
                            .then(|| self.definition_from_declaration(declaration))
                            .flatten(),
                    );
                }
                let name = path.join("::");
                return Some(match kind {
                    DeclarationKind::Trait => self.schema_trait_definition_for_name(&name),
                    _ => self.schema_type_definition_for_name(&name),
                });
            }
        }
        None
    }
}
