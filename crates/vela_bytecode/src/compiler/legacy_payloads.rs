use std::collections::HashMap;

use vela_common::{SourceId, Span};
use vela_hir::ids::ModuleId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{EnumVariantFieldsHint, FunctionSignature};
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::{
    AstNode, Block, EnumVariantFields, Expr, FunctionItem, ItemKind, Param, SourceFile,
    SyntaxBlock, SyntaxSourceFile,
};
use vela_syntax::parser::parse_source as parse_legacy_source;

use super::body_payloads::CompilerBodyPayload;
use super::param_defaults::{ParamDefaultValue, syntax_param_default_values};
use super::schema_defaults::{SchemaDefaultPayloads, SchemaDefaultValue};

pub(super) struct LegacySourceFallback {
    parsed: SourceFile,
}

impl LegacySourceFallback {
    pub(super) fn parse(source: SourceId, text: &str) -> Self {
        Self {
            parsed: parse_legacy_source(source, text),
        }
    }

    pub(super) fn impl_methods_by_body_span(&self) -> HashMap<Span, LegacyMethodFallback<'_>> {
        let mut methods = HashMap::new();
        for item in &self.parsed.items {
            let ItemKind::Impl(item) = &item.kind else {
                continue;
            };
            for method in &item.methods {
                methods.insert(
                    method.function.body.span,
                    LegacyMethodFallback {
                        param_defaults: legacy_param_defaults(&method.function.params),
                        body: &method.function.body,
                    },
                );
            }
        }
        methods
    }

    pub(super) fn trait_default_methods_by_body_span(
        &self,
    ) -> HashMap<Span, LegacyMethodFallback<'_>> {
        let mut methods = HashMap::new();
        for item in &self.parsed.items {
            let ItemKind::Trait(item) = &item.kind else {
                continue;
            };
            for method in &item.methods {
                let Some(body) = &method.default_body else {
                    continue;
                };
                methods.insert(
                    body.span,
                    LegacyMethodFallback {
                        param_defaults: legacy_param_defaults(&method.params),
                        body,
                    },
                );
            }
        }
        methods
    }

    pub(super) fn schema_default_payloads<'ast>(
        &'ast self,
        source: SourceId,
        syntax: &SyntaxParse<SyntaxSourceFile>,
        graph: &ModuleGraph,
        module: ModuleId,
    ) -> SchemaDefaultPayloads<'ast> {
        let legacy = legacy_default_exprs_by_span(&self.parsed);
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
                let Some(default_span) =
                    graph_struct_field_default_span(graph, module, &type_name, &field_name)
                else {
                    continue;
                };
                let legacy_value = legacy.get(&default_span).copied();
                payloads.insert_struct_field(
                    type_name.clone(),
                    field_name,
                    SchemaDefaultValue::new(source, value, legacy_value),
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
                        let Some(default_span) = graph_enum_tuple_field_default_span(
                            graph,
                            module,
                            &type_name,
                            &variant_name,
                            index,
                        ) else {
                            continue;
                        };
                        let legacy_value = legacy.get(&default_span).copied();
                        payloads.insert_enum_tuple_field(
                            type_name.clone(),
                            variant_name.clone(),
                            index,
                            SchemaDefaultValue::new(source, value, legacy_value),
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
                        let Some(default_span) = graph_enum_record_field_default_span(
                            graph,
                            module,
                            &type_name,
                            &variant_name,
                            &field_name,
                        ) else {
                            continue;
                        };
                        let legacy_value = legacy.get(&default_span).copied();
                        payloads.insert_enum_record_field(
                            type_name.clone(),
                            variant_name.clone(),
                            field_name,
                            SchemaDefaultValue::new(source, value, legacy_value),
                        );
                    }
                }
            }
        }

        payloads
    }
}

pub(super) struct LegacyMethodFallback<'ast> {
    pub(super) param_defaults: Vec<Option<&'ast Expr>>,
    pub(super) body: &'ast Block,
}

pub(super) struct FunctionBodyPayload<'ast> {
    pub(super) name: String,
    pub(super) body: CompilerBodyPayload<'ast>,
    pub(super) param_defaults: Vec<Option<ParamDefaultValue<'ast>>>,
}

pub(super) fn function_body_payload<'ast>(
    source: SourceId,
    syntax: &SyntaxParse<SyntaxSourceFile>,
    legacy: &'ast LegacySourceFallback,
    name: &str,
    signature: &FunctionSignature,
) -> Option<FunctionBodyPayload<'ast>> {
    let syntax_function = syntax
        .tree()
        .functions()
        .find(|function| function.name_text().as_deref() == Some(name))?;
    let syntax_body = syntax_function.body()?;
    let function = legacy_function_body(&legacy.parsed, syntax_body_span(source, &syntax_body))?;
    let legacy_defaults = legacy_param_defaults(&function.params);
    let param_defaults = syntax_param_default_values(
        source,
        syntax_function.param_list(),
        &legacy_defaults,
        signature.params.len(),
    );
    Some(FunctionBodyPayload {
        name: name.to_owned(),
        body: CompilerBodyPayload::syntax(source, syntax_body, &function.body),
        param_defaults,
    })
}

fn legacy_function_body(parsed: &SourceFile, body_span: Span) -> Option<&FunctionItem> {
    parsed.items.iter().find_map(|item| match &item.kind {
        ItemKind::Function(function) if function.body.span == body_span => Some(function),
        _ => None,
    })
}

fn syntax_body_span(source: SourceId, body: &SyntaxBlock) -> Span {
    let range = body.syntax().text_range();
    let start: u32 = range.start().into();
    let end: u32 = range.end().into();
    Span::new(source, start, end)
}

fn legacy_param_defaults(params: &[Param]) -> Vec<Option<&Expr>> {
    params
        .iter()
        .map(|param| param.default_value.as_ref())
        .collect()
}

fn legacy_default_exprs_by_span(parsed: &SourceFile) -> HashMap<Span, &Expr> {
    let mut defaults = HashMap::new();
    for item in &parsed.items {
        match &item.kind {
            ItemKind::Struct(record) => {
                for field in &record.fields {
                    if let Some(default_value) = field.default_value.as_ref() {
                        defaults.insert(default_value.span, default_value);
                    }
                }
            }
            ItemKind::Enum(enumeration) => {
                for variant in &enumeration.variants {
                    match &variant.fields {
                        EnumVariantFields::Unit => {}
                        EnumVariantFields::Tuple(fields) => {
                            for field in fields {
                                if let Some(default_value) = field.default_value.as_ref() {
                                    defaults.insert(default_value.span, default_value);
                                }
                            }
                        }
                        EnumVariantFields::Record(fields) => {
                            for field in fields {
                                if let Some(default_value) = field.default_value.as_ref() {
                                    defaults.insert(default_value.span, default_value);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    defaults
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
