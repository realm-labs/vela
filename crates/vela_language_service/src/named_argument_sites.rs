use vela_hir::binding::{BindingMap, LocalBinding};
use vela_hir::ids::{HirBodyId, HirLocalId};
use vela_syntax::{
    SyntaxKind,
    ast::{AstNode, SyntaxCallExpr},
};

use crate::{DocumentId, LanguageServiceDatabases, LineIndex, TextRange};

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
    let Some(parsed) = databases.parse_db().syntax_parse(document) else {
        return Vec::new();
    };
    let Some(source) = databases.source_db().records().get(document) else {
        return Vec::new();
    };
    let line_index = LineIndex::new(source.text());
    let mut sites = Vec::new();
    for call in parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxCallExpr::cast)
    {
        let labels = call
            .arguments()
            .iter()
            .filter_map(|argument| argument.name_token())
            .filter(|label| {
                at.map_or_else(
                    || names.contains(&label.text()),
                    |range| {
                        range
                            == TextRange::new(
                                usize::from(label.text_range().start()),
                                usize::from(label.text_range().end()),
                            )
                    },
                )
            })
            .collect::<Vec<_>>();
        if labels.is_empty() {
            continue;
        }
        let Some(expression) = call.callee() else {
            continue;
        };
        let Some(name) = expression
            .as_path()
            .and_then(|path| path.path_tokens().last().cloned())
            .filter(|token| token.kind() == SyntaxKind::Ident)
            .or_else(|| expression.as_field().and_then(|field| field.name_token()))
        else {
            continue;
        };
        let Some(callee) = databases.definition(
            document,
            line_index.position(usize::from(name.text_range().start())),
        ) else {
            continue;
        };
        let Some(parameters) = databases.source_parameters_for_navigation(&callee) else {
            continue;
        };
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
        for label in labels {
            let parameter = parameters
                .params
                .iter()
                .find(|param| param.name == label.text())
                .and_then(|param| bindings.locals().find(|local| local.span == param.span))
                .map(|local| local.id);
            sites.push(NamedArgumentSite {
                document: document.clone(),
                range: TextRange::new(
                    usize::from(label.text_range().start()),
                    usize::from(label.text_range().end()),
                ),
                name: label.text().to_owned(),
                owner: bindings.body(),
                parameter,
            });
        }
    }
    sites
}
