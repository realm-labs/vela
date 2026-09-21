use vela_hir::binding::{BindingMap, LocalBinding};
use vela_hir::ids::{HirBodyId, HirLocalId};

use crate::{DocumentId, LanguageServiceDatabases, TextRange};

pub(crate) struct NamedArgumentSite {
    pub(crate) document: DocumentId,
    pub(crate) range: TextRange,
    pub(crate) name: String,
    pub(crate) owner: HirBodyId,
    pub(crate) parameter: Option<HirLocalId>,
}

pub(crate) fn matching(
    databases: &LanguageServiceDatabases,
    names: &[&str],
) -> Vec<NamedArgumentSite> {
    databases
        .source_db()
        .records()
        .keys()
        .flat_map(|document| in_document(databases, document, None, names))
        .collect()
}

pub(crate) fn target<'a>(
    databases: &'a LanguageServiceDatabases,
    document: &DocumentId,
    range: TextRange,
) -> Option<(&'a BindingMap, &'a LocalBinding)> {
    let site = in_document(databases, document, Some(range), &[])
        .into_iter()
        .next()?;
    let bindings = databases.hir_db().graph().bindings_for_body(site.owner)?;
    Some((bindings, bindings.local(site.parameter?)?))
}

fn in_document(
    databases: &LanguageServiceDatabases,
    document: &DocumentId,
    at: Option<TextRange>,
    names: &[&str],
) -> Vec<NamedArgumentSite> {
    let mut sites = Vec::new();
    let names = at.is_none().then_some(names);
    for call in crate::call_argument_sites::in_document(databases, document, at, names) {
        let parameters = call.parameters;
        if parameters.required_method.is_some() {
            continue;
        }
        // Match resolved signature spans to the canonical binding map. Method
        // bodies have their own maps even when they share an impl declaration.
        let graph = databases.hir_db().graph();
        let Some(bindings) = parameters
            .declaration
            .and_then(|declaration| graph.bindings(declaration))
            .or_else(|| {
                graph
                    .bodies()
                    .filter_map(|body| graph.bindings_for_body(body.id))
                    .find(|bindings| {
                        parameters
                            .params
                            .iter()
                            .any(|param| bindings.locals().any(|local| local.span == param.span))
                    })
            })
        else {
            continue;
        };
        for label in call.labels {
            let parameter = parameters
                .params
                .iter()
                .find(|param| param.name == label.name)
                .and_then(|param| bindings.locals().find(|local| local.span == param.span))
                .map(|local| local.id);
            sites.push(NamedArgumentSite {
                document: document.clone(),
                range: label.range,
                name: label.name,
                owner: bindings.body(),
                parameter,
            });
        }
    }
    sites
}
