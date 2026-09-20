use vela_hir::{
    ids::HirDeclId,
    module_graph::{DeclarationKind, Visibility},
};
use vela_syntax::ast::{AstNode, SyntaxRecordExpr, SyntaxRecordPattern};

use crate::{LanguageServiceDatabases, LineIndex, QueryContext, SourceRecord, TextRange};

pub(crate) fn receiver_owner(
    graph: &vela_hir::module_graph::ModuleGraph,
    fact: &vela_analysis::type_fact::TypeFact,
    field: &str,
) -> Option<HirDeclId> {
    use vela_analysis::type_fact::TypeFact;
    match fact {
        TypeFact::Record { name } => {
            let mut owners = graph.declarations().filter(|declaration| {
                declaration.kind == DeclarationKind::Struct
                    && (crate::symbol_ref::qualified_source_declaration_path(graph, declaration)
                        .join("::")
                        == *name
                        || (!name.contains("::") && declaration.name == *name))
                    && graph
                        .struct_shape(declaration.id)
                        .is_some_and(|shape| shape.fields.iter().any(|entry| entry.name == field))
            });
            let owner = owners.next()?.id;
            owners.next().is_none().then_some(owner)
        }
        TypeFact::Union(facts) => {
            let mut owners = facts.iter().map(|fact| receiver_owner(graph, fact, field));
            let owner = owners.next()??;
            owners
                .all(|candidate| candidate == Some(owner))
                .then_some(owner)
        }
        _ => None,
    }
}

pub(crate) struct FieldSite {
    pub(crate) owner: HirDeclId,
    pub(crate) name: String,
    pub(crate) range: TextRange,
    pub(crate) shorthand: bool,
}

/// Resolve labels in their own module/import scope. Keep unknown field names on
/// a known owner so rename can reject accidentally capturing such a label.
pub(crate) fn sites(databases: &LanguageServiceDatabases, source: &SourceRecord) -> Vec<FieldSite> {
    let Some(parsed) = databases.parse_db().syntax_parse(source.document_id()) else {
        return Vec::new();
    };
    let graph = databases.hir_db().graph();
    let lines = LineIndex::new(source.text());
    let mut sites = Vec::new();
    for node in parsed.tree().syntax().descendants() {
        let (path, fields) = if let Some(record) = SyntaxRecordExpr::cast(node.clone()) {
            (
                record.path_segments(),
                record
                    .fields()
                    .into_iter()
                    .filter_map(|field| Some((field.label_token()?, field.is_shorthand())))
                    .collect::<Vec<_>>(),
            )
        } else if let Some(pattern) = SyntaxRecordPattern::cast(node) {
            (
                pattern.path_segments(),
                pattern
                    .fields()
                    .filter_map(|field| Some((field.label_token()?, field.is_shorthand())))
                    .collect(),
            )
        } else {
            continue;
        };
        let Some((first, _)) = fields.first() else {
            continue;
        };
        let Some(query) = QueryContext::from_databases(
            databases,
            source.document_id(),
            lines.position(usize::from(first.text_range().start())),
        ) else {
            continue;
        };
        let Some(module) = query.module_key().and_then(|key| graph.module_id(key)) else {
            continue;
        };
        let Some(path) = query.expand_import_path(&path) else {
            continue;
        };
        let Some(owner) =
            graph.resolve_visible_declaration_path(module, &path, DeclarationKind::Struct)
        else {
            continue;
        };
        if owner.module != module && owner.visibility != Visibility::Public {
            continue;
        }
        sites.extend(fields.into_iter().map(|(label, shorthand)| FieldSite {
            owner: owner.id,
            name: label.text().to_owned(),
            range: TextRange::new(
                usize::from(label.text_range().start()),
                usize::from(label.text_range().end()),
            ),
            shorthand,
        }));
    }
    sites
}

pub(crate) fn explicit_target(
    databases: &LanguageServiceDatabases,
    source: &SourceRecord,
    range: TextRange,
) -> Option<FieldSite> {
    sites(databases, source)
        .into_iter()
        .find(|site| !site.shorthand && site.range == range)
}
