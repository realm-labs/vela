//! Exact import targets and alias declarations, with source ownership first.
use std::collections::BTreeMap;

use vela_common::SourceId;

use crate::LanguageServiceDatabases;

use super::{
    SemanticTokenClassification as C, SemanticTokenModifiers as M, SemanticTokenType as T,
    path_targets::Targets,
};

pub(super) fn collect(
    db: &LanguageServiceDatabases,
    source: SourceId,
) -> BTreeMap<(usize, usize), C> {
    let graph = db.hir_db().graph();
    let mut result = BTreeMap::new();
    for module in graph.module_ids() {
        let Some(imports) = graph.imports(module) else {
            continue;
        };
        if !imports.iter().any(|import| import.span.source == source) {
            continue;
        }
        let targets = Targets::new(db, module);
        for import in imports.iter().filter(|import| import.span.source == source) {
            let target = targets.import(&import.path, import.resolution);
            for (index, span) in import.path_spans.iter().enumerate() {
                result.insert(
                    (span.start as usize, span.end as usize),
                    if index + 1 == import.path.len() {
                        target
                    } else {
                        C::new(T::Module, M::NONE)
                    },
                );
            }
            if let Some(span) = import.alias_span {
                let alias = if target.token_type == T::UnresolvedReference {
                    target
                } else {
                    C::new(target.token_type, target.modifiers.union(M::DECLARATION))
                };
                result.insert((span.start as usize, span.end as usize), alias);
            }
        }
    }
    result
}
