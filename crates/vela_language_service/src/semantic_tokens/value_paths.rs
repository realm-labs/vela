//! Whole known value paths assign provenance only to their terminal segment.
use std::collections::BTreeMap;

use vela_common::{SourceId, Span};
use vela_hir::binding::BindingResolution;
use vela_syntax::{
    SyntaxKind,
    ast::{AstNode, SyntaxPathExpr},
};

use super::{
    SemanticTokenClassification as C, SemanticTokenModifiers as M, SemanticTokenType as T,
    path_targets::Targets,
};
use crate::LanguageServiceDatabases;

pub(super) fn collect(
    db: &LanguageServiceDatabases,
    source: SourceId,
) -> BTreeMap<(usize, usize), C> {
    let mut result = BTreeMap::new();
    let graph = db.hir_db().graph();
    let Some(record) = db
        .source_db()
        .records()
        .values()
        .find(|record| record.source_id() == source)
    else {
        return result;
    };
    let Some(module) = graph.module_id(record.module_key()) else {
        return result;
    };
    let Some(parsed) = db.parse_db().syntax_parse(record.document_id()) else {
        return result;
    };
    let targets = Targets::new(db, module);
    for path in parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxPathExpr::cast)
    {
        let span = Span::new(
            source,
            path.syntax().text_range().start().into(),
            path.syntax().text_range().end().into(),
        );
        let local = graph
            .body_containing_offset(source, span.start)
            .and_then(|body| graph.bindings_for_body(body.id))
            .and_then(|bindings| {
                graph
                    .expression_containing_span(span)
                    .and_then(|id| bindings.resolution(id))
                    .and_then(|resolution| match resolution {
                        BindingResolution::Local(id) => bindings.local(*id),
                        _ => None,
                    })
            });
        let tokens: Vec<_> = path
            .path_tokens()
            .into_iter()
            .filter(|token| token.kind() == SyntaxKind::Ident)
            .collect();
        if let Some(local) = local {
            for (index, token) in tokens.iter().enumerate() {
                result.insert(
                    (
                        usize::from(token.text_range().start()),
                        usize::from(token.text_range().end()),
                    ),
                    if index == 0 {
                        super::local_use_classification(local)
                    } else {
                        C::new(T::Variable, M::NONE)
                    },
                );
            }
            continue;
        }
        let Some(expanded) = graph.expand_import_path(module, &path.path_segments()) else {
            continue;
        };
        let Some(target) = targets.value(&expanded) else {
            continue;
        };
        for (index, token) in tokens.iter().enumerate() {
            result.insert(
                (
                    usize::from(token.text_range().start()),
                    usize::from(token.text_range().end()),
                ),
                if index + 1 == tokens.len() || target.token_type == T::UnresolvedReference {
                    target
                } else {
                    C::new(T::Module, M::NONE)
                },
            );
        }
    }
    result
}
