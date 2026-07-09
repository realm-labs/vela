use std::collections::{BTreeMap, HashMap};

use vela_common::{SourceId, Span};
use vela_hir::ids::{HirDeclId, ModuleId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::{AstNode, SyntaxExpression, SyntaxSourceFile};

use super::schema_defaults::{SchemaDefaultPayloads, SchemaDefaultValue};

pub(super) fn const_value_payloads(
    source: SourceId,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    graph: &ModuleGraph,
    module: ModuleId,
) -> BTreeMap<HirDeclId, SyntaxExpression> {
    let mut payloads = BTreeMap::new();
    let targets = const_value_targets(graph, module);
    for item in parsed.tree().consts() {
        let Some(value) = item.value() else {
            continue;
        };
        let Some(declaration) = targets.get(&syntax_expression_span(source, &value)) else {
            continue;
        };
        payloads.entry(*declaration).or_insert(value);
    }
    payloads
}

fn const_value_targets(graph: &ModuleGraph, module: ModuleId) -> HashMap<Span, HirDeclId> {
    graph
        .declarations_in_module(module)
        .into_iter()
        .filter(|declaration| declaration.kind == DeclarationKind::Const)
        .filter_map(|declaration| {
            graph
                .const_metadata(declaration.id)
                .map(|metadata| (metadata.value_span, declaration.id))
        })
        .collect()
}

pub(super) fn schema_default_payloads(
    source: SourceId,
    syntax: &SyntaxParse<SyntaxSourceFile>,
    graph: &ModuleGraph,
    module: ModuleId,
) -> SchemaDefaultPayloads {
    let mut payloads = SchemaDefaultPayloads::default();
    let targets = schema_default_targets(graph, module);
    for item in syntax.tree().structs() {
        let Some(fields) = item.field_list() else {
            continue;
        };
        for field in fields.fields() {
            let Some(value) = field.default_value() else {
                continue;
            };
            let Some((type_name, field_name)) = targets
                .struct_fields
                .get(&syntax_expression_span(source, &value))
            else {
                continue;
            };
            payloads.insert_struct_field(
                type_name.clone(),
                field_name.clone(),
                SchemaDefaultValue::new(source, value),
            );
        }
    }

    for item in syntax.tree().enums() {
        let Some(variants) = item.variant_list() else {
            continue;
        };
        for variant in variants.variants() {
            if let Some(fields) = variant.tuple_field_list() {
                for field in fields.params() {
                    let Some(value) = field.default_value() else {
                        continue;
                    };
                    let Some((type_name, variant_name, target_index)) = targets
                        .enum_tuple_fields
                        .get(&syntax_expression_span(source, &value))
                    else {
                        continue;
                    };
                    payloads.insert_enum_tuple_field(
                        type_name.clone(),
                        variant_name.clone(),
                        *target_index,
                        SchemaDefaultValue::new(source, value),
                    );
                }
            }
            if let Some(fields) = variant.record_field_list() {
                for field in fields.fields() {
                    let Some(value) = field.default_value() else {
                        continue;
                    };
                    let Some((type_name, variant_name, field_name)) = targets
                        .enum_record_fields
                        .get(&syntax_expression_span(source, &value))
                    else {
                        continue;
                    };
                    payloads.insert_enum_record_field(
                        type_name.clone(),
                        variant_name.clone(),
                        field_name.clone(),
                        SchemaDefaultValue::new(source, value),
                    );
                }
            }
        }
    }

    payloads
}

#[derive(Default)]
struct SchemaDefaultTargets {
    struct_fields: HashMap<Span, (String, String)>,
    enum_tuple_fields: HashMap<Span, (String, String, usize)>,
    enum_record_fields: HashMap<Span, (String, String, String)>,
}

fn schema_default_targets(graph: &ModuleGraph, module: ModuleId) -> SchemaDefaultTargets {
    let mut targets = SchemaDefaultTargets::default();
    for declaration in graph.declarations_in_module(module) {
        match declaration.kind {
            DeclarationKind::Struct => {
                if let Some(shape) = graph.struct_shape(declaration.id) {
                    for field in &shape.fields {
                        if let Some(span) = field.default_value_span {
                            targets
                                .struct_fields
                                .insert(span, (declaration.name.clone(), field.name.clone()));
                        }
                    }
                }
            }
            DeclarationKind::Enum => {
                if let Some(shape) = graph.enum_shape(declaration.id) {
                    for variant in &shape.variants {
                        match &variant.fields {
                            vela_hir::type_hint::EnumVariantFieldsHint::Tuple(fields) => {
                                for (index, field) in fields.iter().enumerate() {
                                    if let Some(span) = field.default_value_span {
                                        targets.enum_tuple_fields.insert(
                                            span,
                                            (declaration.name.clone(), variant.name.clone(), index),
                                        );
                                    }
                                }
                            }
                            vela_hir::type_hint::EnumVariantFieldsHint::Record(fields) => {
                                for field in fields {
                                    if let Some(span) = field.default_value_span {
                                        targets.enum_record_fields.insert(
                                            span,
                                            (
                                                declaration.name.clone(),
                                                variant.name.clone(),
                                                field.name.clone(),
                                            ),
                                        );
                                    }
                                }
                            }
                            vela_hir::type_hint::EnumVariantFieldsHint::Unit => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
    targets
}

fn syntax_expression_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}
