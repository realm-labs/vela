use std::collections::BTreeMap;

use vela_common::{SourceId, Span};
use vela_hir::ids::ModuleId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::EnumVariantFieldsHint;
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::{SyntaxExpression, SyntaxSourceFile};

use super::schema_defaults::{SchemaDefaultPayloads, SchemaDefaultValue};

pub(super) fn const_value_payloads(
    parsed: &SyntaxParse<SyntaxSourceFile>,
) -> BTreeMap<String, SyntaxExpression> {
    let mut payloads = BTreeMap::new();
    for item in parsed.tree().consts() {
        let Some(name) = item.name_text() else {
            continue;
        };
        let Some(value) = item.value() else {
            continue;
        };
        payloads.entry(name).or_insert(value);
    }
    payloads
}

pub(super) fn schema_default_payloads(
    source: SourceId,
    syntax: &SyntaxParse<SyntaxSourceFile>,
    graph: &ModuleGraph,
    module: ModuleId,
) -> SchemaDefaultPayloads {
    let mut payloads = SchemaDefaultPayloads::default();
    for item in syntax.tree().structs() {
        let Some(type_name) = item.name_text() else {
            continue;
        };
        let Some(fields) = item.field_list() else {
            continue;
        };
        for field in fields.fields() {
            let Some(field_name) = field.name_text() else {
                continue;
            };
            let Some(value) = field.default_value() else {
                continue;
            };
            let Some(_default_span) =
                graph_struct_field_default_span(graph, module, &type_name, &field_name)
            else {
                continue;
            };
            payloads.insert_struct_field(
                type_name.clone(),
                field_name,
                SchemaDefaultValue::new(source, value),
            );
        }
    }

    for item in syntax.tree().enums() {
        let Some(type_name) = item.name_text() else {
            continue;
        };
        let Some(variants) = item.variant_list() else {
            continue;
        };
        for variant in variants.variants() {
            let Some(variant_name) = variant.name_text() else {
                continue;
            };
            if let Some(fields) = variant.tuple_field_list() {
                for (index, field) in fields.params().enumerate() {
                    let Some(value) = field.default_value() else {
                        continue;
                    };
                    let Some(_default_span) = graph_enum_tuple_field_default_span(
                        graph,
                        module,
                        &type_name,
                        &variant_name,
                        index,
                    ) else {
                        continue;
                    };
                    payloads.insert_enum_tuple_field(
                        type_name.clone(),
                        variant_name.clone(),
                        index,
                        SchemaDefaultValue::new(source, value),
                    );
                }
            }
            if let Some(fields) = variant.record_field_list() {
                for field in fields.fields() {
                    let Some(field_name) = field.name_text() else {
                        continue;
                    };
                    let Some(value) = field.default_value() else {
                        continue;
                    };
                    let Some(_default_span) = graph_enum_record_field_default_span(
                        graph,
                        module,
                        &type_name,
                        &variant_name,
                        &field_name,
                    ) else {
                        continue;
                    };
                    payloads.insert_enum_record_field(
                        type_name.clone(),
                        variant_name.clone(),
                        field_name,
                        SchemaDefaultValue::new(source, value),
                    );
                }
            }
        }
    }

    payloads
}

fn graph_struct_field_default_span(
    graph: &ModuleGraph,
    module: ModuleId,
    type_name: &str,
    field_name: &str,
) -> Option<Span> {
    let declaration = graph_schema_declaration(graph, module, type_name, DeclarationKind::Struct)?;
    let shape = graph.struct_shape(declaration)?;
    let field = shape.fields.iter().find(|field| field.name == field_name)?;
    field.default_value_span
}

fn graph_enum_tuple_field_default_span(
    graph: &ModuleGraph,
    module: ModuleId,
    type_name: &str,
    variant_name: &str,
    index: usize,
) -> Option<Span> {
    let fields = graph_enum_variant_fields(graph, module, type_name, variant_name)?;
    let EnumVariantFieldsHint::Tuple(fields) = fields else {
        return None;
    };
    let field = fields.get(index)?;
    field.default_value_span
}

fn graph_enum_record_field_default_span(
    graph: &ModuleGraph,
    module: ModuleId,
    type_name: &str,
    variant_name: &str,
    field_name: &str,
) -> Option<Span> {
    let fields = graph_enum_variant_fields(graph, module, type_name, variant_name)?;
    let EnumVariantFieldsHint::Record(fields) = fields else {
        return None;
    };
    let field = fields.iter().find(|field| field.name == field_name)?;
    field.default_value_span
}

fn graph_enum_variant_fields<'graph>(
    graph: &'graph ModuleGraph,
    module: ModuleId,
    type_name: &str,
    variant_name: &str,
) -> Option<&'graph EnumVariantFieldsHint> {
    let declaration = graph_schema_declaration(graph, module, type_name, DeclarationKind::Enum)?;
    let shape = graph.enum_shape(declaration)?;
    shape
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)
        .map(|variant| &variant.fields)
}

fn graph_schema_declaration(
    graph: &ModuleGraph,
    module: ModuleId,
    type_name: &str,
    kind: DeclarationKind,
) -> Option<vela_hir::ids::HirDeclId> {
    graph
        .declarations_in_module(module)
        .into_iter()
        .find(|declaration| declaration.name == type_name && declaration.kind == kind)
        .map(|declaration| declaration.id)
}
