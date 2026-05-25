use std::collections::{BTreeMap, BTreeSet};

use vela_common::{Diagnostic, Span};
use vela_hir::{HirDeclId, ModuleGraph, ModuleId};
use vela_syntax::{Argument, EnumVariantFields, Expr, ItemKind, RecordField, SourceFile};

use crate::Constant;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ScriptSchemaDefaults {
    record_shapes: BTreeMap<String, ConstructorShape>,
    enum_shapes: BTreeMap<(String, String), ConstructorShape>,
    enum_variants: BTreeMap<String, BTreeSet<String>>,
}

impl ScriptSchemaDefaults {
    pub(super) fn merge(&mut self, other: Self) {
        self.record_shapes.extend(other.record_shapes);
        self.enum_shapes.extend(other.enum_shapes);
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

    fn len(&self) -> usize {
        self.fields.len()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ConstructorField {
    name: String,
    default: Option<SchemaFieldDefault>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SchemaFieldDefault {
    pub(super) name: String,
    pub(super) value: Expr,
    pub(super) constants: BTreeMap<String, Constant>,
}

pub(super) fn source_schema_defaults(
    parsed: &SourceFile,
    graph: &ModuleGraph,
    module: ModuleId,
    type_symbols: &BTreeMap<HirDeclId, String>,
    constants: BTreeMap<String, Constant>,
) -> ScriptSchemaDefaults {
    let mut defaults = ScriptSchemaDefaults::default();
    let Some(declarations) = graph.module(module) else {
        return defaults;
    };

    for item in &parsed.items {
        match &item.kind {
            ItemKind::Struct(record) => {
                let Some(type_name) = declarations
                    .get(&record.name)
                    .and_then(|declaration| type_symbols.get(&declaration))
                    .cloned()
                else {
                    continue;
                };
                let fields = record
                    .fields
                    .iter()
                    .map(|field| ConstructorField {
                        name: field.name.clone(),
                        default: field.default_value.clone().map(|value| SchemaFieldDefault {
                            name: field.name.clone(),
                            value,
                            constants: constants.clone(),
                        }),
                    })
                    .collect::<Vec<_>>();
                defaults
                    .record_shapes
                    .insert(type_name, ConstructorShape::new(fields));
            }
            ItemKind::Enum(enumeration) => {
                let Some(type_name) = declarations
                    .get(&enumeration.name)
                    .and_then(|declaration| type_symbols.get(&declaration))
                    .cloned()
                else {
                    continue;
                };
                for variant in &enumeration.variants {
                    defaults
                        .enum_variants
                        .entry(type_name.clone())
                        .or_default()
                        .insert(variant.name.clone());
                    let fields = enum_variant_fields(&variant.fields, constants.clone());
                    defaults.enum_shapes.insert(
                        (type_name.clone(), variant.name.clone()),
                        ConstructorShape::new(fields),
                    );
                }
            }
            _ => {}
        }
    }

    defaults
}

pub(super) fn record_constructor_diagnostics(
    type_name: &str,
    shape: Option<&ConstructorShape>,
    fields: &[RecordField],
    constructor_span: Span,
) -> Vec<Diagnostic> {
    let mut diagnostics = duplicate_record_field_diagnostics(fields);
    let Some(shape) = shape else {
        return diagnostics;
    };
    let explicit = fields
        .iter()
        .map(|field| field.name.as_str())
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
        if !explicit.contains(field.name.as_str()) {
            diagnostics.push(missing_field_diagnostic(
                type_name,
                &field.name,
                constructor_span,
            ));
        }
    }

    diagnostics
}

pub(super) fn tuple_constructor_diagnostics(
    type_name: &str,
    variant: &str,
    shape: Option<&ConstructorShape>,
    args: &[Argument],
    constructor_span: Span,
) -> Vec<Diagnostic> {
    let Some(shape) = shape else {
        return Vec::new();
    };
    let owner = format!("{type_name}.{variant}");
    let mut diagnostics = Vec::new();
    for (index, arg) in args.iter().enumerate().skip(shape.len()) {
        diagnostics.push(unknown_field_diagnostic(
            &owner,
            &index.to_string(),
            arg.value.span,
            shape.field_names(),
        ));
    }
    for field in shape.required_fields().skip(args.len()) {
        diagnostics.push(missing_field_diagnostic(
            &owner,
            &field.name,
            constructor_span,
        ));
    }
    diagnostics
}

pub(super) fn unknown_enum_variant_diagnostic(
    enum_name: &str,
    variant: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(format!("unknown enum variant `{enum_name}.{variant}`"))
        .with_code("compiler::unknown_constructor_variant")
        .with_span(span)
        .with_label(span, "variant is not declared on this enum")
}

fn duplicate_record_field_diagnostics(fields: &[RecordField]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = BTreeMap::<&str, Span>::new();
    for field in fields {
        if let Some(previous_span) = seen.insert(&field.name, field.span) {
            diagnostics.push(
                Diagnostic::error(format!("duplicate constructor field `{}`", field.name))
                    .with_code("compiler::duplicate_constructor_field")
                    .with_span(field.span)
                    .with_label(previous_span, "previous field is here")
                    .with_label(field.span, "duplicate field is here"),
            );
        }
    }
    diagnostics
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
    fields: &EnumVariantFields,
    constants: BTreeMap<String, Constant>,
) -> Vec<ConstructorField> {
    match fields {
        EnumVariantFields::Unit => Vec::new(),
        EnumVariantFields::Tuple(fields) => fields
            .iter()
            .enumerate()
            .map(|(index, field)| ConstructorField {
                name: index.to_string(),
                default: field.default_value.clone().map(|value| SchemaFieldDefault {
                    name: index.to_string(),
                    value,
                    constants: constants.clone(),
                }),
            })
            .collect(),
        EnumVariantFields::Record(fields) => fields
            .iter()
            .map(|field| ConstructorField {
                name: field.name.clone(),
                default: field.default_value.clone().map(|value| SchemaFieldDefault {
                    name: field.name.clone(),
                    value,
                    constants: constants.clone(),
                }),
            })
            .collect(),
    }
}
