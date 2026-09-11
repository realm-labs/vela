use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, Visibility};
use vela_hir::type_hint::EnumVariantFieldsHint;

use super::{
    CompletionInsertFormat, CompletionItem, CompletionKind, display_type_detail_parts,
    model::RecordConstructor,
};
use crate::callable_context::query_type_fact_from_hint;
use crate::symbol_ref::{qualified_source_declaration_name, source_child_symbol};

// Some(empty) is an owned negative result: a private type or a non-record
// variant must not fall through to unrelated schema metadata.
pub(super) fn source_record_field_completions(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    constructor: &RecordConstructor,
) -> Option<Vec<CompletionItem>> {
    let module = graph.module_id(constructor.current_module.as_ref()?)?;
    let source = graph
        .resolve_visible_declaration_path(module, &constructor.path, DeclarationKind::Struct)
        .map(|declaration| (declaration, None))
        .or_else(|| {
            let (variant, owner) = constructor.path.split_last()?;
            graph
                .resolve_visible_declaration_path(module, owner, DeclarationKind::Enum)
                .map(|declaration| (declaration, Some(variant)))
        })?;
    let (declaration, variant) = source;
    if declaration.module != module && declaration.visibility != Visibility::Public {
        return Some(Vec::new());
    }
    let owner = qualified_source_declaration_name(graph, declaration);
    let (owner, fields) = if let Some(variant) = variant {
        let fields = graph
            .enum_shape(declaration.id)?
            .variants
            .iter()
            .find(|entry| entry.name == *variant)
            .and_then(|entry| match &entry.fields {
                EnumVariantFieldsHint::Record(fields) => Some(fields.as_slice()),
                _ => None,
            })
            .unwrap_or_default();
        (format!("{owner}::{variant}"), fields)
    } else {
        (owner, graph.struct_shape(declaration.id)?.fields.as_slice())
    };
    Some(
        fields
            .iter()
            .map(|field| {
                let fact = field.type_hint.as_ref().map_or(TypeFact::Unknown, |hint| {
                    query_type_fact_from_hint(graph, hint, schema)
                });
                let detail = display_type_detail_parts(fact.display_name());
                CompletionItem {
                    label: field.name.clone(),
                    kind: CompletionKind::Field,
                    detail: detail.render(),
                    insert_text: Some(field.name.clone()),
                    insert_format: CompletionInsertFormat::PlainText,
                    sort_text: None,
                    metadata: Default::default(),
                }
                .with_detail_parts(detail)
                .with_symbol(source_child_symbol(&owner, &field.name))
            })
            .collect(),
    )
}
