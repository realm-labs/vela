use vela_hir::module_graph::{DeclarationKind, Visibility};
use vela_syntax::ast::{AstNode, SyntaxImplItem};

use super::Hover;
use crate::{DiagnosticRange, LanguageServiceDatabases, QueryContext, symbol_target::SymbolTarget};

pub(super) fn hover(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    target: &SymbolTarget,
    range: DiagnosticRange,
) -> Option<Option<Hover>> {
    let graph = db.hir_db().graph();
    let key = query.module_key()?;
    let module = graph.module_id(key)?;
    for item in query
        .syntax_parse()?
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxImplItem::cast)
    {
        for (tokens, path, required) in [
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
            let Some(path) = graph.expand_import_path(module, &path[..=index]) else {
                return Some(None);
            };
            if index + 1 < tokens.len() {
                return Some(super::module_hover(
                    graph,
                    key,
                    &path,
                    range,
                    target.symbol().cloned(),
                ));
            }
            for kind in [
                DeclarationKind::Struct,
                DeclarationKind::Enum,
                DeclarationKind::Trait,
                DeclarationKind::Function,
                DeclarationKind::Const,
                DeclarationKind::State,
            ] {
                if let Some(declaration) = graph.declaration_by_type_path(&path, key, kind) {
                    return Some(
                        (kind == required
                            && (declaration.module == module
                                || declaration.visibility == Visibility::Public))
                            .then(|| {
                                super::hover_from_declaration(
                                    graph,
                                    db.graph_analysis_facts(),
                                    declaration,
                                    range,
                                )
                            }),
                    );
                }
            }
            let name = path.join("::");
            let schema = db.schema_db().facts();
            return Some(
                ((required == DeclarationKind::Trait && schema.trait_fact(&name).is_some())
                    || (required == DeclarationKind::Struct && schema.type_fact(&name).is_some()))
                .then(|| super::schema::symbol_hover(schema, &name, range))
                .flatten(),
            );
        }
    }
    None
}
