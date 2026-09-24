use vela_common::Span;
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::body::{HirPath, HirPathKind, HirPathOwner};
use vela_hir::ids::{HirDeclId, HirExprId};
use vela_hir::module_graph::{
    Declaration, DeclarationKind, ImportResolution, ModuleGraph, Visibility,
};

use crate::{LanguageServiceDatabases, LineIndex, QueryContext, TextRange};

pub(crate) fn binding_maps(
    graph: &ModuleGraph,
) -> impl Iterator<Item = &vela_hir::binding::BindingMap> {
    let mut seen = std::collections::BTreeSet::new();
    graph
        .bodies()
        .filter_map(|body| graph.bindings_for_body(body.id))
        .filter(move |bindings| seen.insert(bindings.body()))
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PathSite<'a> {
    pub(crate) path: &'a [String],
    pub(crate) segment_range: TextRange,
}

pub(crate) fn site(path: &HirPath) -> Option<PathSite<'_>> {
    Some(PathSite {
        path: path.path.as_slice(),
        segment_range: text_range_for_span(path.segment_origin.span)?,
    })
}

pub(crate) const fn is_expression_path(kind: HirPathKind) -> bool {
    matches!(
        kind,
        HirPathKind::Value | HirPathKind::Callee | HirPathKind::Constructor
    )
}

pub(crate) fn text_range_for_span(span: Span) -> Option<TextRange> {
    Some(TextRange::new(
        usize::try_from(span.start).ok()?,
        usize::try_from(span.end).ok()?,
    ))
}

pub(crate) fn resolved_use_range(graph: &ModuleGraph, span: Span) -> Option<TextRange> {
    let range = text_range_for_span(span)?;
    Some(
        graph
            .paths_in_source(span.source)
            .filter(|path| is_expression_path(path.kind))
            .filter(|path| path.origin.span.start == span.start)
            .filter_map(site)
            .filter(|site| {
                range.start <= site.segment_range.start && site.segment_range.end <= range.end
            })
            .map(|site| site.segment_range)
            .max_by_key(|segment| segment.start)
            .unwrap_or(range),
    )
}

pub(crate) fn imported_declaration(
    graph: &ModuleGraph,
    source: vela_common::SourceId,
    range: TextRange,
) -> Option<&Declaration> {
    graph
        .module_ids()
        .filter_map(|module| graph.imports(module))
        .flatten()
        .find_map(|import| {
            if import.span.source != source {
                return None;
            }
            let terminal = import
                .path_spans
                .last()
                .copied()
                .and_then(text_range_for_span);
            let alias = import.alias_span.and_then(text_range_for_span);
            if terminal != Some(range) && alias != Some(range) {
                return None;
            }
            let ImportResolution::Declaration(declaration) = import.resolution?;
            graph.declaration(declaration)
        })
}

pub(crate) fn qualified_value_declaration(
    databases: &LanguageServiceDatabases,
    span: Span,
    path: &[String],
) -> Option<HirDeclId> {
    if path.len() < 2 {
        return None;
    }
    let graph = databases.hir_db().graph();
    let source = databases
        .source_db()
        .records()
        .values()
        .find(|source| source.source_id() == span.source)?;
    let range = resolved_use_range(graph, span)?;
    let query = QueryContext::from_databases(
        databases,
        source.document_id(),
        LineIndex::new(source.text()).position(range.start),
    )?;
    let module = graph.module_id(query.module_key()?)?;
    let expanded = query.expand_import_path(path)?;
    let declaration = [
        DeclarationKind::Function,
        DeclarationKind::Const,
        DeclarationKind::State,
    ]
    .into_iter()
    .find_map(|kind| graph.resolve_visible_declaration_path(module, &expanded, kind))?;
    (declaration.module == module || declaration.visibility == Visibility::Public)
        .then_some(declaration.id)
}

pub(crate) fn imported_module_value_for_expression(
    databases: &LanguageServiceDatabases,
    expression: HirExprId,
    imported_name: &str,
) -> Option<HirDeclId> {
    let graph = databases.hir_db().graph();
    let span = graph.expression_span(expression)?;
    let path = graph
        .paths_in_source(span.source)
        .find(|path| path.owner == HirPathOwner::Expression(expression))?;
    (path.path.first().is_some_and(|name| name == imported_name))
        .then(|| qualified_value_declaration(databases, span, &path.path))?
}

pub(crate) fn qualified_value_at_range(
    databases: &LanguageServiceDatabases,
    bindings: &BindingMap,
    range: TextRange,
) -> Option<HirDeclId> {
    let graph = databases.hir_db().graph();
    let source = graph.declaration(bindings.declaration)?.span.source;
    let expression = graph.expression_containing_span(Span::new(
        source,
        u32::try_from(range.start).ok()?,
        u32::try_from(range.end).ok()?,
    ))?;
    let span = graph.expression_span(expression)?;
    if resolved_use_range(graph, span)? != range {
        return None;
    }
    match bindings.resolution(expression)? {
        BindingResolution::QualifiedPath(path) => {
            qualified_value_declaration(databases, span, path)
        }
        BindingResolution::Import(name) => {
            imported_module_value_for_expression(databases, expression, name)
        }
        BindingResolution::Declaration(_) | BindingResolution::Local(_) => None,
    }
}
