use vela_hir::module_graph::{DeclarationKind, Visibility};

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
    path_hover(db, query, &path, range)
}

// Resolve the selected import segment, rather than treating every prefix as a
// module or using the terminal import's declaration for an enum variant.
pub(super) fn path_hover(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    path: &[String],
    range: DiagnosticRange,
) -> Option<Hover> {
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
        if let Some(declaration) = graph.declaration_by_type_path(path, key, kind) {
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
    if let Some((name, owner)) = path.split_last()
        && let Some(declaration) = graph.declaration_by_type_path(owner, key, DeclarationKind::Enum)
    {
        return (declaration.module == module || declaration.visibility == Visibility::Public)
            .then(|| {
                graph
                    .enum_shape(declaration.id)?
                    .variants
                    .iter()
                    .find(|variant| variant.name == *name)
                    .map(|variant| super::enum_variant_hover(graph, declaration, variant, range))
            })
            .flatten();
    }
    super::module_hover(graph, key, path, range, None).or_else(|| {
        let name = path.join("::");
        super::schema::symbol_hover(db.schema_db().facts(), &name, range)
            .or_else(|| super::stdlib_symbol_hover(&name, range))
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
        return Some(path_hover(db, query, &import.path[..=segment], range));
    }
    None
}
