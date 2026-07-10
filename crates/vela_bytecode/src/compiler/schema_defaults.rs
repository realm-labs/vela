use std::collections::{BTreeMap, BTreeSet};

use vela_common::{Diagnostic, Span};
use vela_hir::body::HirBodyOwner;
use vela_hir::ids::{HirBodyId, HirDeclId, ModuleId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::EnumVariantFieldsHint;

use crate::Constant;

use super::const_eval::evaluate_const_body;
use super::error::CompileResult;
use super::value_types::{RuntimeTypeFact, type_hint_value_type};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ConstructorFieldUse {
    pub(super) name: String,
    pub(super) span: Span,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ScriptSchemaDefaults {
    record_shapes: BTreeMap<String, ConstructorShape>,
    enum_shapes: BTreeMap<(String, String), ConstructorShape>,
    enum_variants: BTreeMap<String, BTreeSet<String>>,
    evaluated_defaults: BTreeMap<HirBodyId, Option<Constant>>,
}

impl ScriptSchemaDefaults {
    pub(super) fn merge(&mut self, other: Self) {
        self.record_shapes.extend(other.record_shapes);
        self.enum_shapes.extend(other.enum_shapes);
        self.evaluated_defaults.extend(other.evaluated_defaults);
        for (enum_name, variants) in other.enum_variants {
            self.enum_variants
                .entry(enum_name)
                .or_default()
                .extend(variants);
        }
    }

    pub(super) fn record(&self, type_name: &str) -> Option<&ConstructorShape> {
        self.record_shapes.get(type_name)
    }

    pub(super) fn enum_variant(&self, type_name: &str, variant: &str) -> Option<&ConstructorShape> {
        self.enum_shapes
            .get(&(type_name.to_owned(), variant.to_owned()))
    }

    pub(super) fn enum_contains_variant(&self, type_name: &str, variant: &str) -> bool {
        self.enum_variants
            .get(type_name)
            .is_some_and(|variants| variants.contains(variant))
    }

    pub(super) fn evaluated_defaults(&self) -> &BTreeMap<HirBodyId, Option<Constant>> {
        &self.evaluated_defaults
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ConstructorShape {
    fields: Vec<ConstructorField>,
}

impl ConstructorShape {
    fn new(fields: Vec<ConstructorField>) -> Self {
        Self { fields }
    }

    pub(super) fn defaults(&self) -> impl Iterator<Item = &SchemaFieldDefault> {
        self.fields
            .iter()
            .filter_map(|field| field.default.as_ref())
    }

    pub(super) fn default_fields(&self) -> Vec<SchemaFieldDefault> {
        self.defaults().cloned().collect()
    }

    pub(super) fn field_name_at(&self, index: usize) -> Option<&str> {
        self.fields.get(index).map(|field| field.name.as_str())
    }

    pub(super) fn field_value_type_at(&self, index: usize) -> Option<RuntimeTypeFact> {
        self.fields
            .get(index)
            .and_then(|field| field.value_type.clone())
    }

    pub(in crate::compiler) fn argument_name_at(&self, index: usize) -> Option<&str> {
        self.fields
            .get(index)
            .map(|field| field.argument_name.as_str())
    }

    pub(in crate::compiler) fn field_has_default_at(&self, index: usize) -> bool {
        self.fields
            .get(index)
            .is_some_and(|field| field.default.is_some())
    }

    pub(super) fn field_value_type(&self, name: &str) -> Option<RuntimeTypeFact> {
        self.fields
            .iter()
            .find(|field| field.name == name)
            .and_then(|field| field.value_type.clone())
    }

    fn contains_field(&self, name: &str) -> bool {
        self.fields.iter().any(|field| field.name == name)
    }

    fn required_fields(&self) -> impl Iterator<Item = &ConstructorField> {
        self.fields.iter().filter(|field| field.default.is_none())
    }

    fn field_names(&self) -> Vec<&str> {
        self.fields
            .iter()
            .map(|field| field.name.as_str())
            .collect()
    }

    pub(in crate::compiler) fn argument_index(&self, name: &str) -> Option<usize> {
        self.fields
            .iter()
            .position(|field| field.argument_name == name)
    }

    pub(in crate::compiler) fn len(&self) -> usize {
        self.fields.len()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ConstructorField {
    name: String,
    argument_name: String,
    value_type: Option<RuntimeTypeFact>,
    default: Option<SchemaFieldDefault>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SchemaFieldDefault {
    pub(super) name: String,
    pub(super) value: Option<Constant>,
    pub(super) span: Span,
}

pub(super) fn source_schema_defaults(
    graph: &ModuleGraph,
    module: ModuleId,
    type_symbols: &BTreeMap<HirDeclId, String>,
    constants: &BTreeMap<HirDeclId, Constant>,
) -> CompileResult<ScriptSchemaDefaults> {
    let mut defaults = ScriptSchemaDefaults::default();
    let mut evaluated_defaults = BTreeMap::new();
    for body in graph.bodies() {
        let HirBodyOwner::SchemaFieldDefault(declaration) = body.owner else {
            continue;
        };
        if !type_symbols.contains_key(&declaration) {
            continue;
        }
        let Some(bindings) = graph.schema_field_default_bindings(body.id) else {
            continue;
        };
        evaluated_defaults.insert(body.id, evaluate_const_body(body, bindings, constants)?);
    }

    for declaration in module_schema_declarations(graph, module) {
        let Some(metadata) = graph.declaration(declaration) else {
            continue;
        };
        match metadata.kind {
            DeclarationKind::Struct => {
                let Some(type_name) = type_symbols.get(&declaration).cloned() else {
                    continue;
                };
                let Some(shape) = graph.struct_shape(declaration) else {
                    continue;
                };
                let fields = shape
                    .fields
                    .iter()
                    .map(|field| ConstructorField {
                        name: field.name.clone(),
                        argument_name: field.name.clone(),
                        value_type: field.type_hint.as_ref().and_then(type_hint_value_type),
                        default: field.default_body.and_then(|body| {
                            schema_field_default(
                                field.name.clone(),
                                body,
                                field.default_value_span?,
                                &evaluated_defaults,
                            )
                        }),
                    })
                    .collect::<Vec<_>>();
                defaults
                    .record_shapes
                    .insert(type_name, ConstructorShape::new(fields));
            }
            DeclarationKind::Enum => {
                let Some(type_name) = type_symbols.get(&declaration).cloned() else {
                    continue;
                };
                let Some(shape) = graph.enum_shape(declaration) else {
                    continue;
                };
                for variant in &shape.variants {
                    defaults
                        .enum_variants
                        .entry(type_name.clone())
                        .or_default()
                        .insert(variant.name.clone());
                    let fields = enum_variant_fields(
                        &metadata.name,
                        &variant.name,
                        &variant.fields,
                        &evaluated_defaults,
                    );
                    defaults.enum_shapes.insert(
                        (type_name.clone(), variant.name.clone()),
                        ConstructorShape::new(fields),
                    );
                }
            }
            _ => {}
        }
    }

    defaults.evaluated_defaults = evaluated_defaults;

    Ok(defaults)
}

fn module_schema_declarations(graph: &ModuleGraph, module: ModuleId) -> Vec<HirDeclId> {
    let Some(declarations) = graph.module(module) else {
        return Vec::new();
    };

    let mut schema_declarations = declarations
        .names()
        .filter_map(|name| {
            let declaration = declarations.get(name)?;
            let metadata = graph.declaration(declaration)?;
            match metadata.kind {
                DeclarationKind::Struct | DeclarationKind::Enum => Some(declaration),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    schema_declarations.sort_unstable();
    schema_declarations
}

fn schema_field_default(
    name: String,
    body: HirBodyId,
    span: Span,
    values: &BTreeMap<HirBodyId, Option<Constant>>,
) -> Option<SchemaFieldDefault> {
    let value = values.get(&body)?.clone();
    Some(SchemaFieldDefault { name, value, span })
}

pub(super) fn record_constructor_field_diagnostics(
    type_name: &str,
    shape: Option<&ConstructorShape>,
    fields: &[ConstructorFieldUse],
    constructor_span: Span,
) -> Vec<Diagnostic> {
    let mut diagnostics = duplicate_record_field_diagnostics(fields);
    let Some(shape) = shape else {
        return diagnostics;
    };
    let explicit = fields
        .iter()
        .map(|field| field.name.clone())
        .collect::<BTreeSet<_>>();

    for field in fields {
        if !shape.contains_field(&field.name) {
            diagnostics.push(unknown_field_diagnostic(
                type_name,
                &field.name,
                field.span,
                shape.field_names(),
            ));
        }
    }

    for field in shape.required_fields() {
        if !explicit.contains(&field.name) {
            diagnostics.push(missing_field_diagnostic(
                type_name,
                &field.name,
                constructor_span,
            ));
        }
    }

    diagnostics
}

pub(super) fn unknown_enum_variant_diagnostic(
    enum_name: &str,
    variant: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(format!("unknown enum variant `{enum_name}::{variant}`"))
        .with_code("compiler::unknown_constructor_variant")
        .with_span(span)
        .with_label(span, "variant is not declared on this enum")
}

fn duplicate_record_field_diagnostics(fields: &[ConstructorFieldUse]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = BTreeMap::<&str, Span>::new();
    for field in fields {
        if let Some(previous_span) = seen.insert(&field.name, field.span) {
            diagnostics.push(duplicate_constructor_field_diagnostic(
                &field.name,
                previous_span,
                field.span,
            ));
        }
    }
    diagnostics
}

fn duplicate_constructor_field_diagnostic(
    name: &str,
    previous_span: Span,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(format!("duplicate constructor field `{name}`"))
        .with_code("compiler::duplicate_constructor_field")
        .with_span(span)
        .with_label(previous_span, "previous field is here")
        .with_label(span, "duplicate field is here")
}

fn unknown_field_diagnostic(
    type_name: &str,
    field: &str,
    span: Span,
    candidates: Vec<&str>,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(format!(
        "unknown constructor field `{field}` for `{type_name}`"
    ))
    .with_code("compiler::unknown_constructor_field")
    .with_span(span)
    .with_label(span, "field is not declared by the constructor schema");
    if !candidates.is_empty() {
        diagnostic =
            diagnostic.with_label(span, format!("available fields: {}", candidates.join(", ")));
    }
    diagnostic
}

fn missing_field_diagnostic(type_name: &str, field: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!(
        "missing constructor field `{field}` for `{type_name}`"
    ))
    .with_code("compiler::missing_constructor_field")
    .with_span(span)
    .with_label(span, "required field is not provided and has no default")
}

fn enum_variant_fields(
    _enum_name: &str,
    _variant_name: &str,
    fields: &EnumVariantFieldsHint,
    evaluated_defaults: &BTreeMap<HirBodyId, Option<Constant>>,
) -> Vec<ConstructorField> {
    match fields {
        EnumVariantFieldsHint::Unit => Vec::new(),
        EnumVariantFieldsHint::Tuple(fields) => fields
            .iter()
            .enumerate()
            .map(|(index, field)| ConstructorField {
                name: index.to_string(),
                argument_name: field.name.clone(),
                value_type: field.type_hint.as_ref().and_then(type_hint_value_type),
                default: field.default_body.and_then(|body| {
                    schema_field_default(
                        index.to_string(),
                        body,
                        field.default_value_span?,
                        evaluated_defaults,
                    )
                }),
            })
            .collect(),
        EnumVariantFieldsHint::Record(fields) => fields
            .iter()
            .map(|field| ConstructorField {
                name: field.name.clone(),
                argument_name: field.name.clone(),
                value_type: field.type_hint.as_ref().and_then(type_hint_value_type),
                default: field.default_body.and_then(|body| {
                    schema_field_default(
                        field.name.clone(),
                        body,
                        field.default_value_span?,
                        evaluated_defaults,
                    )
                }),
            })
            .collect(),
    }
}
