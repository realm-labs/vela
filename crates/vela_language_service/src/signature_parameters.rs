use crate::{DocumentId, LanguageServiceDatabases, QueryContext, SymbolRef, TextRange};
use vela_common::Span;
use vela_hir::{
    binding::{BindingMap, BindingResolution},
    ids::HirLocalId,
    type_hint::{FunctionSignature, ParamHint},
};

#[derive(Clone, Copy)]
pub(crate) struct Target<'a> {
    pub(crate) method: Span,
    pub(crate) signature: &'a FunctionSignature,
    pub(crate) parameter: &'a ParamHint,
}

impl Target<'_> {
    pub(crate) fn symbol(&self, db: &LanguageServiceDatabases) -> Option<SymbolRef> {
        let source = db
            .source_db()
            .records()
            .values()
            .find(|source| source.source_id() == self.parameter.span.source)?;
        Some(SymbolRef::local_at(
            &self.parameter.name,
            source.document_id().clone(),
            range(self.parameter.span),
        ))
    }

    pub(crate) fn bindings<'a>(
        &self,
        db: &'a LanguageServiceDatabases,
    ) -> Vec<(&'a BindingMap, HirLocalId)> {
        self.signature
            .params
            .iter()
            .filter_map(|param| {
                let bindings = db.hir_db().graph().bindings_for_body(param.default_body?)?;
                let local = bindings
                    .locals()
                    .find(|local| local.span == self.parameter.span)?;
                Some((bindings, local.id))
            })
            .collect()
    }
}

fn targets(db: &LanguageServiceDatabases) -> Vec<Target<'_>> {
    let graph = db.hir_db().graph();
    graph
        .declarations()
        .filter_map(|declaration| graph.trait_shape(declaration.id))
        .flat_map(|shape| shape.methods.iter())
        .filter(|method| !method.has_default)
        .flat_map(|method| {
            method.signature.params.iter().map(move |parameter| Target {
                method: method.name_span,
                signature: &method.signature,
                parameter,
            })
        })
        .collect()
}

pub(crate) fn target<'a>(
    db: &'a LanguageServiceDatabases,
    query: &QueryContext<'_>,
) -> Option<Target<'a>> {
    let source = query.source_id()?;
    let token = query.identifier_range()?;
    let targets = targets(db);
    if targets.is_empty() {
        return None;
    }
    for target in &targets {
        if target.parameter.span.source == source && range(target.parameter.span) == token {
            return Some(*target);
        }
        for (bindings, local) in target.bindings(db) {
            if target.parameter.span.source == source
                && crate::query_context::binding_resolution_for_source_range(
                    db.hir_db().graph(),
                    bindings,
                    token,
                ) == Some(&BindingResolution::Local(local))
            {
                return Some(*target);
            }
        }
    }
    if let Some(call) =
        crate::call_argument_sites::in_document(db, query.document_id(), Some(token), None)
            .into_iter()
            .next()
    {
        let method = call.parameters.required_method?;
        let label = call.labels.first()?;
        return targets
            .into_iter()
            .find(|target| target.method == method && target.parameter.name == label.name);
    }
    None
}

#[derive(Clone, Copy)]
pub(crate) enum SiteKind {
    Declaration,
    Body,
    Label,
}

pub(crate) struct Site {
    pub(crate) document: DocumentId,
    pub(crate) span: Span,
    pub(crate) kind: SiteKind,
}

pub(crate) fn sites(
    db: &LanguageServiceDatabases,
    target: Target<'_>,
    include_declaration: bool,
) -> Vec<Site> {
    let mut result = Vec::new();
    if let Some(source) = db
        .source_db()
        .records()
        .values()
        .find(|source| source.source_id() == target.parameter.span.source)
    {
        if include_declaration {
            result.push(Site {
                document: source.document_id().clone(),
                span: target.parameter.span,
                kind: SiteKind::Declaration,
            });
        }
        for (bindings, local) in target.bindings(db) {
            for (expression, resolution) in bindings.resolutions() {
                if *resolution == BindingResolution::Local(local)
                    && let Some(span) = db.hir_db().graph().expression_span(expression)
                {
                    result.push(Site {
                        document: source.document_id().clone(),
                        span,
                        kind: SiteKind::Body,
                    });
                }
            }
        }
    }
    for (document, source) in db.source_db().records() {
        for call in crate::call_argument_sites::in_document(
            db,
            document,
            None,
            Some(&[&target.parameter.name]),
        ) {
            if call.parameters.required_method != Some(target.method) {
                continue;
            }
            result.extend(call.labels.into_iter().map(|label| Site {
                document: document.clone(),
                span: Span::new(
                    source.source_id(),
                    label.range.start as u32,
                    label.range.end as u32,
                ),
                kind: SiteKind::Label,
            }));
        }
    }
    result.sort_by_key(|site| (site.document.clone(), site.span.start, site.span.end));
    result.dedup_by(|a, b| a.document == b.document && a.span == b.span);
    result
}

pub(crate) fn captures_label(
    db: &LanguageServiceDatabases,
    target: Target<'_>,
    name: &str,
) -> bool {
    name != target.parameter.name
        && db.source_db().records().keys().any(|document| {
            crate::call_argument_sites::in_document(db, document, None, Some(&[name]))
                .iter()
                .any(|call| call.parameters.required_method == Some(target.method))
        })
}

pub(crate) fn range(span: Span) -> TextRange {
    TextRange::new(span.start as usize, span.end as usize)
}
