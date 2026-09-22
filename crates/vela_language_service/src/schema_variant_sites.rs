use vela_hir::{body::HirPathKind, module_graph::DeclarationKind};

use crate::{
    LanguageServiceDatabases, LineIndex, QueryContext, ReferenceKind, SourceRecord, TextRange,
    hir_path_sites,
};

pub(crate) struct Site {
    pub(crate) owner: String,
    pub(crate) variant: String,
    pub(crate) range: TextRange,
    pub(crate) edit_range: Option<TextRange>,
    pub(crate) kind: ReferenceKind,
}

pub(crate) fn target(
    db: &LanguageServiceDatabases,
    source: &SourceRecord,
    range: TextRange,
) -> Option<(String, String)> {
    db.schema_db()
        .facts()
        .variants()
        .find_map(|variant| {
            let span = db
                .schema_db()
                .source_locations()
                .variant_span(&variant.owner, &variant.name)?;
            (span.source == source.source_id()
                && hir_path_sites::text_range_for_span(span) == Some(range))
            .then_some((variant.owner, variant.name))
        })
        .or_else(|| {
            collect(db, source, Some(range))
                .into_iter()
                .find(|site| site.range == range || site.edit_range == Some(range))
                .map(|site| (site.owner, site.variant))
        })
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
    let schema = db.schema_db().facts();
    let names = names(db);
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
            let expanded = scoped_path(db, &query, site.path, site.segment_range)?;
            let module = query.module_key().and_then(|key| graph.module_id(key))?;
            let (owner, variant) = resolve_names(&names, &expanded)?.clone();
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
                edit_range: (!explicit_alias).then_some(site.segment_range),
                kind: if path.kind == HirPathKind::Pattern {
                    ReferenceKind::Pattern
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
            .filter(|import| {
                import.span.source == source.source_id() && import.resolution.is_none()
            })
            .filter_map(|import| {
                let (variant, parent) = import.path.split_last()?;
                let owner = parent.join("::");
                schema.variant_fact(&owner, variant)?;
                let terminal = *import.path_spans.last()?;
                Some(Site {
                    owner,
                    variant: variant.clone(),
                    range: hir_path_sites::text_range_for_span(
                        import.alias_span.unwrap_or(terminal),
                    )?,
                    edit_range: hir_path_sites::text_range_for_span(terminal),
                    kind: ReferenceKind::Import,
                })
            }),
    );
    result
}

pub(crate) type VariantName = (String, String);

pub(crate) fn names(db: &LanguageServiceDatabases) -> Vec<VariantName> {
    db.schema_db()
        .facts()
        .variants()
        .map(|variant| (variant.owner, variant.name))
        .collect()
}

pub(crate) fn scoped_path(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    path: &[String],
    range: TextRange,
) -> Option<Vec<String>> {
    let expanded = crate::schema_function_sites::scoped_path(db, query, path, range)?;
    let graph = db.hir_db().graph();
    let module = query.module_key().and_then(|key| graph.module_id(key))?;
    let (_, parent) = expanded.split_last()?;
    if [
        DeclarationKind::Enum,
        DeclarationKind::Struct,
        DeclarationKind::Trait,
        DeclarationKind::Function,
        DeclarationKind::Const,
        DeclarationKind::State,
    ]
    .into_iter()
    .any(|kind| {
        graph
            .resolve_visible_declaration_path(module, parent, kind)
            .is_some()
    }) {
        return None;
    }
    Some(expanded)
}

pub(crate) fn resolve_names<'a>(
    names: &'a [VariantName],
    path: &[String],
) -> Option<&'a VariantName> {
    let (variant, parent) = path.split_last()?;
    if parent.is_empty() {
        return None;
    }
    let owner = parent.join("::");
    if let Some(exact) = names
        .iter()
        .find(|(candidate_owner, name)| candidate_owner == &owner && name == variant)
    {
        return Some(exact);
    }
    if parent.len() != 1 {
        return None;
    }
    let mut matches = names.iter().filter(|(candidate_owner, name)| {
        name == variant && candidate_owner.rsplit("::").next() == Some(owner.as_str())
    });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}
