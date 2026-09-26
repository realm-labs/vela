use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_hir::module_graph::{DeclarationKind, Visibility};
use vela_syntax::ast::{AstNode, SyntaxImplItem};

use super::{SemanticTokenClassification, SemanticTokenModifiers as M, SemanticTokenType as T};
use crate::LanguageServiceDatabases;

pub(super) fn collect(
    db: &LanguageServiceDatabases,
    source_id: SourceId,
) -> BTreeMap<(usize, usize), SemanticTokenClassification> {
    let mut result = BTreeMap::new();
    let Some(source) = db
        .source_db()
        .records()
        .values()
        .find(|source| source.source_id() == source_id)
    else {
        return result;
    };
    let Some(parse) = db.parse_db().syntax_parse(source.document_id()) else {
        return result;
    };
    let graph = db.hir_db().graph();
    let Some(module) = graph.module_id(source.module_key()) else {
        return result;
    };
    let schema = db.schema_db().facts();
    for item in parse
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
            let expanded = graph.expand_import_path(module, &path);
            let modifiers = expanded.as_ref().map_or(M::NONE, |expanded| {
                if graph
                    .resolve_visible_declaration_path(module, expanded, kind)
                    .is_some_and(|declaration| {
                        declaration.module == module || declaration.visibility == Visibility::Public
                    })
                {
                    return M::SOURCE;
                }
                let name = expanded.join("::");
                let host = match kind {
                    DeclarationKind::Trait => schema.trait_fact(&name).is_some(),
                    _ => schema.type_fact(&name).is_some(),
                };
                if host {
                    M::HOST.union(M::SCHEMA)
                } else {
                    M::NONE
                }
            });
            for (index, token) in tokens.iter().enumerate() {
                let terminal = index + 1 == tokens.len();
                result.insert(
                    (
                        usize::from(token.text_range().start()),
                        usize::from(token.text_range().end()),
                    ),
                    SemanticTokenClassification::new(
                        if terminal { T::Type } else { T::Module },
                        if terminal { modifiers } else { M::NONE },
                    ),
                );
            }
        }
    }
    result
}
