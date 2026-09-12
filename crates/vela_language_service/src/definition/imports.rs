use vela_hir::module_graph::{Declaration, DeclarationKind, ImportResolution, Visibility};

use crate::{LanguageServiceDatabases, QueryContext, symbol_target::SymbolTarget};

impl LanguageServiceDatabases {
    pub(super) fn definition_from_imported_path(
        &self,
        query: &QueryContext<'_>,
        path: &[String],
    ) -> Option<super::Definition> {
        let expanded = query.expand_import_path(path)?;
        if expanded == path {
            return None;
        }
        let graph = self.hir_db().graph();
        let module = graph.module_id(query.module_key()?)?;
        let declaration = [
            DeclarationKind::Function,
            DeclarationKind::Const,
            DeclarationKind::State,
            DeclarationKind::Struct,
            DeclarationKind::Enum,
            DeclarationKind::Trait,
        ]
        .into_iter()
        .find_map(|kind| graph.resolve_visible_declaration_path(module, &expanded, kind))?;
        if declaration.module != module && declaration.visibility != Visibility::Public {
            return None;
        }
        self.definition_from_declaration(declaration)
    }
    // Recognized import syntax owns even a null result: a prefix, unresolved
    // import or inaccessible declaration must not borrow a same-name symbol.
    pub(super) fn source_import_declaration<'a>(
        &'a self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
    ) -> Option<Option<&'a Declaration>> {
        let graph = self.hir_db().graph();
        let source = query.source_id()?;
        let module = graph.module_id(query.module_key()?)?;
        let import = graph.imports(module)?.iter().find(|import| {
            import.span.source == source
                && super::text_range_for_span(import.span).is_some_and(|range| {
                    range.start <= target.range().start && target.range().end <= range.end
                })
        })?;
        let is_target = import
            .path_spans
            .last()
            .copied()
            .into_iter()
            .chain(import.alias_span)
            .any(|span| super::text_range_for_span(span) == Some(target.range()));
        Some((|| {
            if !is_target {
                return None;
            }
            let ImportResolution::Declaration(id) = import.resolution?;
            graph.declaration(id).filter(|declaration| {
                declaration.module == module || declaration.visibility == Visibility::Public
            })
        })())
    }
}
