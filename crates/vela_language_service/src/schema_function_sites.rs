use vela_common::SourceId;
use vela_hir::{binding::BindingResolution, body::HirPathKind};

use crate::{
    LanguageServiceDatabases, LineIndex, QueryContext, SourceRecord, TextRange, hir_path_sites,
};

pub(crate) struct Site {
    pub(crate) name: String,
    pub(crate) range: TextRange,
    pub(crate) call: bool,
}

pub(crate) fn target(
    db: &LanguageServiceDatabases,
    source: &SourceRecord,
    range: TextRange,
) -> Option<String> {
    declaration(db, source.source_id(), range).or_else(|| {
        collect(db, source, Some(range))
            .into_iter()
            .next()
            .map(|site| site.name)
    })
}

pub(crate) fn declaration(
    db: &LanguageServiceDatabases,
    source: SourceId,
    range: TextRange,
) -> Option<String> {
    db.schema_db().facts().functions().find_map(|function| {
        let span = db
            .schema_db()
            .source_locations()
            .function_span(&function.name)?;
        (span.source == source && hir_path_sites::text_range_for_span(span) == Some(range))
            .then_some(function.name)
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
    let lines = LineIndex::new(source.text());
    graph
        .paths_in_source(source.source_id())
        .filter(|path| matches!(path.kind, HirPathKind::Value | HirPathKind::Callee))
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
            if query
                .bindings()
                .and_then(|bindings| {
                    crate::query_context::binding_resolution_for_source_range(
                        graph,
                        bindings,
                        site.segment_range,
                    )
                })
                .is_some_and(|resolution| {
                    matches!(
                        resolution,
                        BindingResolution::Local(_) | BindingResolution::Declaration(_)
                    )
                })
            {
                return None;
            }
            let expanded = query.expand_import_path(site.path)?;
            // Alias tokens have a different editable identity from the imported
            // function's terminal name and are not equivalent rename sites.
            if expanded != site.path {
                return None;
            }
            let name = resolve(db, &expanded.join("::"))?;
            Some(Site {
                name,
                range: site.segment_range,
                call: path.kind == HirPathKind::Callee,
            })
        })
        .collect()
}

fn resolve(db: &LanguageServiceDatabases, path: &str) -> Option<String> {
    let schema = db.schema_db().facts();
    if schema.function_fact(path).is_some() {
        return Some(path.to_owned());
    }
    if path.contains("::") {
        return None;
    }
    let mut matches = schema
        .functions()
        .filter(|function| function.name.rsplit("::").next() == Some(path));
    let found = matches.next()?.name;
    matches.next().is_none().then_some(found)
}
