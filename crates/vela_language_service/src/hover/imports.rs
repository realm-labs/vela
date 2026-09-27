use vela_hir::module_graph::{DeclarationKind, ImportResolution, Visibility};

use super::Hover;
use crate::{DiagnosticRange, LanguageServiceDatabases, QueryContext, symbol_target::SymbolTarget};

// Unresolved HIR imports can still name registered or builtin metadata. Expand
// the actual scoped alias; private source owners remain closed to that fallback.
pub(super) fn use_hover(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    target: &SymbolTarget,
    range: DiagnosticRange,
) -> Option<Hover> {
    let path = query.expand_import_path(&[target.text().to_owned()])?;
    let graph = db.hir_db().graph();
    let key = query.module_key()?;
    let module = graph.module_id(key)?;
    for kind in [
        DeclarationKind::Function,
        DeclarationKind::Const,
        DeclarationKind::State,
        DeclarationKind::Struct,
        DeclarationKind::Enum,
        DeclarationKind::Trait,
    ] {
        if let Some(declaration) = graph.declaration_by_type_path(&path, key, kind) {
            return (declaration.module == module || declaration.visibility == Visibility::Public)
                .then(|| {
                    super::hover_from_declaration(
                        graph,
                        db.graph_analysis_facts(),
                        declaration,
                        range,
                    )
                });
        }
    }
    super::module_hover(graph, key, &path, range, None).or_else(|| {
        let name = path.join("::");
        super::schema::symbol_hover(db.schema_db().facts(), &name, range)
            .or_else(|| super::stdlib_function_hover(&name, range))
    })
}

// Import paths and alias declarations own unresolved results, so inaccessible
// source names cannot fall through to an unrelated short registry/builtin name.
pub(super) fn hover(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    target: &SymbolTarget,
    range: DiagnosticRange,
) -> Option<Option<Hover>> {
    let source = query.source_id()?;
    let graph = db.hir_db().graph();
    let key = query.module_key()?;
    let module = graph.module_id(key)?;
    for import in graph.imports(module)? {
        if import.span.source != source {
            continue;
        }
        let Some(segment) = super::import_path_segment_at(import, target).or_else(|| {
            import
                .alias_span
                .filter(|span| super::span_text_range(*span) == Some(target.range()))
                .and_then(|_| import.path.len().checked_sub(1))
        }) else {
            continue;
        };
        if segment + 1 < import.path.len() {
            return Some(super::module_hover(
                graph,
                key,
                &import.path[..=segment],
                range,
                None,
            ));
        }
        let declaration = import
            .resolution
            .and_then(|ImportResolution::Declaration(id)| graph.declaration(id))
            .or_else(|| {
                [
                    DeclarationKind::Function,
                    DeclarationKind::Const,
                    DeclarationKind::State,
                    DeclarationKind::Struct,
                    DeclarationKind::Enum,
                    DeclarationKind::Trait,
                ]
                .into_iter()
                .find_map(|kind| graph.declaration_by_type_path(&import.path, key, kind))
            });
        if let Some(declaration) = declaration {
            return Some(
                (declaration.module == module || declaration.visibility == Visibility::Public)
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
        if let Some(hover) = super::module_hover(graph, key, &import.path, range, None) {
            return Some(Some(hover));
        }
        let name = import.path.join("::");
        return Some(
            super::schema::symbol_hover(db.schema_db().facts(), &name, range)
                .or_else(|| super::stdlib_function_hover(&name, range)),
        );
    }
    None
}
