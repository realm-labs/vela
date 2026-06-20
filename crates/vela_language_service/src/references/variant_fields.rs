use vela_common::SourceId;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};
use vela_hir::type_hint::EnumVariantFieldsHint;
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::SyntaxSourceFile;

use crate::LanguageServiceDatabases;

use super::{
    Reference, ReferenceKind, ReferenceToken, declaration_name_matches, diagnostic_range,
    name_range_in_text, record_fields, record_variant_patterns, source_variant_field_symbol,
    span_text_range, token_text,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct VariantFieldReferenceTarget {
    owner: HirDeclId,
    variant: String,
    field: String,
}

pub(super) fn script_variant_field_references(
    databases: &LanguageServiceDatabases,
    target: &VariantFieldReferenceTarget,
    include_declaration: bool,
) -> Vec<Reference> {
    let graph = databases.hir_db().graph();
    let mut references = Vec::new();

    if include_declaration
        && let Some(reference) = reference_for_variant_field_declaration(databases, target)
    {
        references.push(reference);
    }

    for source in databases.source_db().records().values() {
        if let Some(parsed) = databases.parse_db().syntax_parse(source.document_id()) {
            references.extend(variant_constructor_field_references_for_source(
                graph, parsed, source, target,
            ));
            references.extend(variant_pattern_field_references_for_source(
                graph, parsed, source, target,
            ));
        }
    }

    references.sort_by_key(|reference| {
        let start = reference.range().start();
        (
            reference.document_id().as_str().to_owned(),
            start.line,
            start.character,
            reference.kind(),
        )
    });
    references
}

pub(super) fn script_variant_field_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &ReferenceToken,
) -> Option<VariantFieldReferenceTarget> {
    let start = u32::try_from(token.range.start).ok()?;
    for declaration in graph.declarations() {
        if declaration.kind != DeclarationKind::Enum
            || declaration.span.source != source_id
            || !declaration.span.contains(start)
        {
            continue;
        }
        let shape = graph.enum_shape(declaration.id)?;
        for variant in &shape.variants {
            let EnumVariantFieldsHint::Record(fields) = &variant.fields else {
                continue;
            };
            for field in fields {
                let span_range = span_text_range(field.span)?;
                let name_range = name_range_in_text(text, span_range, &field.name)?;
                if name_range.start <= token.range.start && token.range.end <= name_range.end {
                    return Some(VariantFieldReferenceTarget {
                        owner: declaration.id,
                        variant: variant.name.clone(),
                        field: field.name.clone(),
                    });
                }
            }
        }
    }
    None
}

pub(super) fn script_variant_field_use_target(
    graph: &ModuleGraph,
    syntax_parse: Option<&SyntaxParse<SyntaxSourceFile>>,
    text: &str,
    token: &ReferenceToken,
) -> Option<VariantFieldReferenceTarget> {
    let field = token_text(text, token.range)?;
    let parsed = syntax_parse?;
    variant_constructor_field_use_target(graph, parsed, token, field)
        .or_else(|| variant_pattern_field_use_target(graph, parsed, token, field))
}

fn reference_for_variant_field_declaration(
    databases: &LanguageServiceDatabases,
    target: &VariantFieldReferenceTarget,
) -> Option<Reference> {
    let graph = databases.hir_db().graph();
    let variant = graph
        .enum_shape(target.owner)?
        .variants
        .iter()
        .find(|variant| variant.name == target.variant)?;
    let EnumVariantFieldsHint::Record(fields) = &variant.fields else {
        return None;
    };
    let field = fields.iter().find(|field| field.name == target.field)?;
    let source = databases
        .source_db()
        .records()
        .values()
        .find(|record| record.source_id() == field.span.source)?;
    let span_range = span_text_range(field.span)?;
    let name_range =
        name_range_in_text(source.text(), span_range, &field.name).unwrap_or(span_range);
    Some(Reference {
        document_id: source.document_id().clone(),
        range: diagnostic_range(source.text(), name_range),
        kind: ReferenceKind::Declaration,
        symbol: source_variant_field_symbol(graph, target.owner, &target.variant, &target.field)?,
    })
}

fn variant_constructor_field_use_target(
    graph: &ModuleGraph,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    token: &ReferenceToken,
    field: &str,
) -> Option<VariantFieldReferenceTarget> {
    record_fields::record_field_sites(parsed)
        .into_iter()
        .find(|site| {
            site.name == field
                && site.name_range.start <= token.range.start
                && token.range.end <= site.name_range.end
        })
        .and_then(|site| variant_field_target_for_path(graph, &site.path, field))
}

fn variant_pattern_field_use_target(
    graph: &ModuleGraph,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    token: &ReferenceToken,
    field: &str,
) -> Option<VariantFieldReferenceTarget> {
    record_variant_patterns::record_pattern_field_sites(parsed)
        .into_iter()
        .find(|site| {
            site.name == field
                && site.name_range.start <= token.range.start
                && token.range.end <= site.name_range.end
        })
        .and_then(|site| variant_field_target_for_path(graph, &site.path, field))
}

fn variant_constructor_field_references_for_source(
    graph: &ModuleGraph,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    source: &crate::SourceRecord,
    target: &VariantFieldReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let text = source.text();
    for field in record_fields::record_field_sites(parsed) {
        if field.name != target.field {
            continue;
        }
        if variant_field_target_for_path(graph, &field.path, &target.field).as_ref() != Some(target)
        {
            continue;
        };
        references.push(Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(text, field.name_range),
            kind: ReferenceKind::Read,
            symbol: source_variant_field_symbol(
                graph,
                target.owner,
                &target.variant,
                &target.field,
            )
            .expect("variant field target should have a source symbol"),
        });
    }
    references
}

fn variant_pattern_field_references_for_source(
    graph: &ModuleGraph,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    source: &crate::SourceRecord,
    target: &VariantFieldReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let text = source.text();
    for field in record_variant_patterns::record_pattern_field_sites(parsed) {
        if field.name != target.field {
            continue;
        }
        if variant_field_target_for_path(graph, &field.path, &target.field).as_ref() != Some(target)
        {
            continue;
        }
        references.push(Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(text, field.name_range),
            kind: ReferenceKind::Pattern,
            symbol: source_variant_field_symbol(
                graph,
                target.owner,
                &target.variant,
                &target.field,
            )
            .expect("variant field target should have a source symbol"),
        });
    }
    references
}

fn variant_field_target_for_path(
    graph: &ModuleGraph,
    path: &[String],
    field: &str,
) -> Option<VariantFieldReferenceTarget> {
    let (variant, owner_path) = path.split_last()?;
    let owner = graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Enum
            || !enum_path_matches(graph, declaration, owner_path)
        {
            return None;
        }
        let has_field = graph.enum_shape(declaration.id).is_some_and(|shape| {
            shape.variants.iter().any(|entry| {
                entry.name == *variant
                    && matches!(
                        &entry.fields,
                        EnumVariantFieldsHint::Record(fields)
                            if fields.iter().any(|entry| entry.name == field)
                    )
            })
        });
        has_field.then_some(declaration.id)
    })?;
    Some(VariantFieldReferenceTarget {
        owner,
        variant: variant.clone(),
        field: field.to_owned(),
    })
}

fn enum_path_matches(graph: &ModuleGraph, declaration: &Declaration, path: &[String]) -> bool {
    match path {
        [name] => declaration_name_matches(graph, declaration, name),
        segments => graph
            .module_path(declaration.module)
            .is_some_and(|module_path| {
                module_path
                    .segments()
                    .iter()
                    .chain(std::iter::once(&declaration.name))
                    .eq(segments.iter())
            }),
    }
}
