use vela_hir::{
    ids::HirDeclId,
    module_graph::{DeclarationKind, Visibility},
    type_hint::EnumVariantFieldsHint,
};
use vela_syntax::ast::{AstNode, SyntaxRecordExpr, SyntaxRecordPattern};

mod tuple;

use crate::{LanguageServiceDatabases, LineIndex, QueryContext, SourceRecord, TextRange};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RecordOwner {
    pub(crate) declaration: HirDeclId,
    pub(crate) variant: Option<String>,
    pub(crate) tuple: bool,
}

pub(crate) struct FieldSignature<'a> {
    pub(crate) name: &'a str,
    pub(crate) span: vela_common::Span,
}

impl RecordOwner {
    pub(crate) fn fields<'a>(
        &self,
        graph: &'a vela_hir::module_graph::ModuleGraph,
    ) -> Option<Vec<FieldSignature<'a>>> {
        if let Some(name) = &self.variant {
            let variant = graph
                .enum_shape(self.declaration)?
                .variants
                .iter()
                .find(|variant| variant.name == *name)?;
            match &variant.fields {
                EnumVariantFieldsHint::Record(fields) if !self.tuple => Some(
                    fields
                        .iter()
                        .map(|field| FieldSignature {
                            name: &field.name,
                            span: field.span,
                        })
                        .collect(),
                ),
                EnumVariantFieldsHint::Tuple(params) if self.tuple => Some(
                    params
                        .iter()
                        .map(|param| FieldSignature {
                            name: &param.name,
                            span: param.span,
                        })
                        .collect(),
                ),
                _ => None,
            }
        } else {
            Some(
                graph
                    .struct_shape(self.declaration)?
                    .fields
                    .iter()
                    .map(|field| FieldSignature {
                        name: &field.name,
                        span: field.span,
                    })
                    .collect(),
            )
        }
    }

    pub(crate) fn symbol(
        &self,
        graph: &vela_hir::module_graph::ModuleGraph,
        field: &str,
    ) -> Option<crate::SymbolRef> {
        if let Some(variant) = &self.variant {
            crate::symbol_ref::source_variant_field_symbol(graph, self.declaration, variant, field)
        } else {
            crate::symbol_ref::source_member_symbol(graph, self.declaration, field)
        }
    }
}

pub(crate) fn declaration_target(
    graph: &vela_hir::module_graph::ModuleGraph,
    source: vela_common::SourceId,
    range: TextRange,
) -> Option<(RecordOwner, String)> {
    for declaration in graph
        .declarations()
        .filter(|declaration| declaration.span.source == source)
    {
        let variants = match declaration.kind {
            DeclarationKind::Struct => vec![(None, false)],
            DeclarationKind::Enum => graph
                .enum_shape(declaration.id)?
                .variants
                .iter()
                .map(|variant| {
                    (
                        Some(variant.name.clone()),
                        matches!(variant.fields, EnumVariantFieldsHint::Tuple(_)),
                    )
                })
                .collect(),
            _ => continue,
        };
        for (variant, tuple) in variants {
            let owner = RecordOwner {
                declaration: declaration.id,
                variant,
                tuple,
            };
            let Some(fields) = owner.fields(graph) else {
                continue;
            };
            if let Some(field) = fields.iter().find(|field| {
                field.span.start as usize == range.start && field.span.end as usize == range.end
            }) {
                return Some((owner, field.name.to_owned()));
            }
        }
    }
    None
}

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
    pub(crate) owner: RecordOwner,
    pub(crate) name: String,
    pub(crate) range: TextRange,
    pub(crate) shorthand: bool,
    pub(crate) pattern: bool,
}

/// Resolve labels in their own module/import scope. Keep unknown field names on
/// a known owner so rename can reject accidentally capturing such a label.
pub(crate) fn sites(databases: &LanguageServiceDatabases, source: &SourceRecord) -> Vec<FieldSite> {
    collect_sites(databases, source, None)
}

fn collect_sites(
    databases: &LanguageServiceDatabases,
    source: &SourceRecord,
    at: Option<TextRange>,
) -> Vec<FieldSite> {
    let mut sites = tuple::sites(databases, source, at);
    sites.extend(collect_record_sites(databases, source, at));
    sites
}

pub(crate) fn record_sites(
    databases: &LanguageServiceDatabases,
    source: &SourceRecord,
) -> Vec<FieldSite> {
    collect_record_sites(databases, source, None)
}

fn collect_record_sites(
    databases: &LanguageServiceDatabases,
    source: &SourceRecord,
    at: Option<TextRange>,
) -> Vec<FieldSite> {
    let Some(parsed) = databases.parse_db().syntax_parse(source.document_id()) else {
        return Vec::new();
    };
    let graph = databases.hir_db().graph();
    let lines = LineIndex::new(source.text());
    let mut sites = Vec::new();
    for node in parsed.tree().syntax().descendants() {
        let (path, mut fields, pattern) = if let Some(record) = SyntaxRecordExpr::cast(node.clone())
        {
            (
                record.path_segments(),
                record
                    .fields()
                    .into_iter()
                    .filter_map(|field| Some((field.label_token()?, field.is_shorthand())))
                    .collect::<Vec<_>>(),
                false,
            )
        } else if let Some(pattern) = SyntaxRecordPattern::cast(node.clone()) {
            (
                pattern.path_segments(),
                pattern
                    .fields()
                    .filter_map(|field| Some((field.label_token()?, field.is_shorthand())))
                    .collect(),
                true,
            )
        } else {
            continue;
        };
        if let Some(range) = at {
            fields.retain(|(label, shorthand)| {
                !shorthand
                    && usize::from(label.text_range().start()) == range.start
                    && usize::from(label.text_range().end()) == range.end
            });
        }
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
        let resolved = graph
            .resolve_visible_declaration_path(module, &path, DeclarationKind::Struct)
            .map(|owner| (owner, None))
            .or_else(|| {
                let (variant, parent) = path.split_last()?;
                graph
                    .resolve_visible_declaration_path(module, parent, DeclarationKind::Enum)
                    .map(|owner| (owner, Some(variant.clone())))
            });
        let Some((owner, variant)) = resolved else {
            continue;
        };
        if owner.module != module && owner.visibility != Visibility::Public {
            continue;
        }
        let owner = RecordOwner {
            declaration: owner.id,
            variant,
            tuple: false,
        };
        sites.extend(fields.into_iter().map(|(label, shorthand)| FieldSite {
            owner: owner.clone(),
            name: label.text().to_owned(),
            range: TextRange::new(
                usize::from(label.text_range().start()),
                usize::from(label.text_range().end()),
            ),
            shorthand,
            pattern,
        }));
    }
    sites
}

pub(crate) fn explicit_target(
    databases: &LanguageServiceDatabases,
    source: &SourceRecord,
    range: TextRange,
) -> Option<FieldSite> {
    collect_sites(databases, source, Some(range))
        .into_iter()
        .find(|site| !site.shorthand && site.range == range)
}
