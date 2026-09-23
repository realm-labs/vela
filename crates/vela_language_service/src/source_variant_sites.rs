use vela_hir::{
    body::HirPathKind,
    ids::{HirDeclId, ModuleId},
    module_graph::{DeclarationKind, Visibility},
};

use crate::{
    LanguageServiceDatabases, LineIndex, QueryContext, ReferenceKind, SourceRecord, TextRange,
    hir_path_sites,
};

pub(crate) struct Site {
    pub(crate) owner: HirDeclId,
    pub(crate) variant: String,
    pub(crate) range: TextRange,
    terminal_range: TextRange,
    pub(crate) edit_range: Option<TextRange>,
    pub(crate) kind: ReferenceKind,
}

pub(crate) fn target(
    db: &LanguageServiceDatabases,
    source: &SourceRecord,
    range: TextRange,
) -> Option<Site> {
    collect(db, source, Some(range))
        .into_iter()
        .find(|site| site.range == range || site.terminal_range == range)
}

pub(crate) fn sites(db: &LanguageServiceDatabases, source: &SourceRecord) -> Vec<Site> {
    collect(db, source, None)
}

fn collect(
    db: &LanguageServiceDatabases,
    source: &SourceRecord,
    at: Option<TextRange>,
) -> Vec<Site> {
    let graph = db.hir_db().graph();
    let lines = LineIndex::new(source.text());
    let mut result: Vec<_> = graph
        .paths_in_source(source.source_id())
        .filter(|path| {
            hir_path_sites::is_expression_path(path.kind) || path.kind == HirPathKind::Pattern
        })
        .filter_map(|path| {
            let site = hir_path_sites::site(path)?;
            if at.is_some_and(|range| range != site.segment_range) {
                return None;
            }
            let query = QueryContext::from_databases(
                db,
                source.document_id(),
                lines.position(site.segment_range.start),
            )?;
            let module = graph.module_id(query.module_key()?)?;
            let expanded = query.expand_import_path(site.path)?;
            let (owner, variant) = resolve(db, module, &expanded)?;
            let explicit_alias = site.path.len() == 1
                && graph.imports(module).is_some_and(|imports| {
                    imports.iter().any(|import| {
                        import.alias.as_ref() == site.path.first() && import.path == expanded
                    })
                });
            Some(Site {
                owner,
                variant,
                range: site.segment_range,
                terminal_range: site.segment_range,
                edit_range: (!explicit_alias).then_some(site.segment_range),
                kind: if path.kind == HirPathKind::Pattern {
                    ReferenceKind::Pattern
                } else if path.kind == HirPathKind::Callee {
                    ReferenceKind::Call
                } else {
                    ReferenceKind::Read
                },
            })
        })
        .collect();
    result.extend(
        graph
            .module_ids()
            .filter_map(|module| graph.imports(module))
            .flatten()
            .filter(|import| import.span.source == source.source_id())
            .filter_map(|import| {
                let terminal_range =
                    hir_path_sites::text_range_for_span(*import.path_spans.last()?)?;
                let range = import
                    .alias_span
                    .and_then(hir_path_sites::text_range_for_span)
                    .unwrap_or(terminal_range);
                if at.is_some_and(|at| at != range && at != terminal_range) {
                    return None;
                }
                let (owner, variant) = resolve(db, import.module, &import.path)?;
                Some(Site {
                    owner,
                    variant,
                    range,
                    terminal_range,
                    edit_range: Some(terminal_range),
                    kind: ReferenceKind::Import,
                })
            }),
    );
    result
}

fn resolve(
    db: &LanguageServiceDatabases,
    module: ModuleId,
    path: &[String],
) -> Option<(HirDeclId, String)> {
    let (name, parent) = path.split_last()?;
    if parent.is_empty() {
        return None;
    }
    let graph = db.hir_db().graph();
    let owner = graph.resolve_visible_declaration_path(module, parent, DeclarationKind::Enum)?;
    if owner.module != module && owner.visibility != Visibility::Public {
        return None;
    }
    graph
        .enum_shape(owner.id)?
        .variants
        .iter()
        .find(|variant| variant.name == *name)?;
    Some((owner.id, name.clone()))
}
