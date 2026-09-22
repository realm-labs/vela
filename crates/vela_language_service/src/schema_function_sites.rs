use vela_common::SourceId;
use vela_hir::module_graph::DeclarationKind;
use vela_hir::{binding::BindingResolution, body::HirPathKind};

use crate::{
    LanguageServiceDatabases, LineIndex, QueryContext, ReferenceKind, SourceRecord, TextRange,
    hir_path_sites,
};

pub(crate) struct Site {
    pub(crate) name: String,
    pub(crate) range: TextRange,
    pub(crate) kind: ReferenceKind,
    pub(crate) edit_range: Option<TextRange>,
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
            let module = query.module_key().and_then(|key| graph.module_id(key))?;
            if [
                DeclarationKind::Function,
                DeclarationKind::Const,
                DeclarationKind::State,
                DeclarationKind::Struct,
                DeclarationKind::Enum,
                DeclarationKind::Trait,
            ]
            .into_iter()
            .any(|kind| {
                graph
                    .resolve_visible_declaration_path(module, &expanded, kind)
                    .is_some()
            }) {
                return None;
            }
            let name = resolve(db, &expanded.join("::"))?;
            let explicit_alias = site.path.len() == 1
                && query
                    .module_key()
                    .and_then(|key| graph.module_id(key))
                    .and_then(|module| graph.imports(module))
                    .is_some_and(|imports| {
                        imports.iter().any(|import| {
                            import.alias.as_ref() == site.path.first() && import.path == expanded
                        })
                    });
            Some(Site {
                name,
                range: site.segment_range,
                kind: if path.kind == HirPathKind::Callee {
                    ReferenceKind::Call
                } else {
                    ReferenceKind::Read
                },
                edit_range: (!explicit_alias).then_some(site.segment_range),
            })
        })
        .chain(import_sites(db, source).into_iter().filter(|site| {
            at.is_none_or(|range| site.range == range || site.edit_range == Some(range))
        }))
        .collect()
}

fn import_sites(db: &LanguageServiceDatabases, source: &SourceRecord) -> Vec<Site> {
    let graph = db.hir_db().graph();
    graph
        .module_ids()
        .filter_map(|module| graph.imports(module))
        .flatten()
        .filter(|import| import.span.source == source.source_id() && import.resolution.is_none())
        .filter_map(|import| {
            let name = import.path.join("::");
            db.schema_db().facts().function_fact(&name)?;
            let terminal = *import.path_spans.last()?;
            let range = hir_path_sites::text_range_for_span(import.alias_span.unwrap_or(terminal))?;
            Some(Site {
                name,
                range,
                kind: ReferenceKind::Import,
                edit_range: hir_path_sites::text_range_for_span(terminal),
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
