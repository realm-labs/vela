use std::collections::{BTreeMap, BTreeSet};

use vela_common::{Diagnostic, SourceId, Span};
use vela_hir::ids::{HirDeclId, ModuleId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::EnumVariantFieldsHint;
use vela_syntax::ast::{Argument, Expr, RecordField, SyntaxExpression};

use crate::Constant;

use super::value_types::{RuntimeTypeFact, type_hint_value_type};

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

    pub(super) fn field_name_at(&self, index: usize) -> Option<&str> {
        self.fields.get(index).map(|field| field.name.as_str())
    }

    pub(super) fn field_value_type_at(&self, index: usize) -> Option<RuntimeTypeFact> {
        self.fields
            .get(index)
            .and_then(|field| field.value_type.clone())
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

    fn argument_names(&self) -> Vec<&str> {
        self.fields
            .iter()
            .map(|field| field.argument_name.as_str())
            .collect()
    }

    fn argument_index(&self, name: &str) -> Option<usize> {
        self.fields
            .iter()
            .position(|field| field.argument_name == name)
    }

    fn len(&self) -> usize {
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
    pub(super) value: SchemaDefaultValue,
    pub(super) constants: BTreeMap<String, Constant>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SchemaDefaultValue {
    source: SourceId,
    syntax: SyntaxExpression,
    legacy: Expr,
}

impl SchemaDefaultValue {
    pub(super) const fn new(source: SourceId, syntax: SyntaxExpression, legacy: Expr) -> Self {
        Self {
            source,
            syntax,
            legacy,
        }
    }

    pub(super) const fn source(&self) -> SourceId {
        self.source
    }

    pub(super) const fn syntax(&self) -> &SyntaxExpression {
        &self.syntax
    }

    pub(super) const fn legacy(&self) -> &Expr {
        &self.legacy
    }

    pub(super) const fn span(&self) -> Span {
        self.legacy.span
    }
}

pub(super) fn source_schema_defaults(
    default_payloads: &SchemaDefaultPayloads,
    graph: &ModuleGraph,
    module: ModuleId,
    type_symbols: &BTreeMap<HirDeclId, String>,
    constants: BTreeMap<String, Constant>,
) -> ScriptSchemaDefaults {
    let mut defaults = ScriptSchemaDefaults::default();

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
                        default: field
                            .default_value_span
                            .as_ref()
                            .and_then(|_| {
                                default_payloads.struct_field(&metadata.name, &field.name)
                            })
                            .map(|value| {
                                schema_field_default(field.name.clone(), value, constants.clone())
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
                        default_payloads,
                        constants.clone(),
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

    defaults
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

#[derive(Default)]
pub(super) struct SchemaDefaultPayloads {
    struct_fields: BTreeMap<(String, String), SchemaDefaultValue>,
    enum_tuple_fields: BTreeMap<(String, String, usize), SchemaDefaultValue>,
    enum_record_fields: BTreeMap<(String, String, String), SchemaDefaultValue>,
}

impl SchemaDefaultPayloads {
    pub(super) fn insert_struct_field(
        &mut self,
        type_name: String,
        field_name: String,
        value: SchemaDefaultValue,
    ) {
        self.struct_fields.insert((type_name, field_name), value);
    }

    pub(super) fn insert_enum_tuple_field(
        &mut self,
        type_name: String,
        variant_name: String,
        index: usize,
        value: SchemaDefaultValue,
    ) {
        self.enum_tuple_fields
            .insert((type_name, variant_name, index), value);
    }

    pub(super) fn insert_enum_record_field(
        &mut self,
        type_name: String,
        variant_name: String,
        field_name: String,
        value: SchemaDefaultValue,
    ) {
        self.enum_record_fields
            .insert((type_name, variant_name, field_name), value);
    }

    fn struct_field(&self, type_name: &str, field_name: &str) -> Option<SchemaDefaultValue> {
        self.struct_fields
            .get(&(type_name.to_owned(), field_name.to_owned()))
            .cloned()
    }

    fn enum_tuple_field(
        &self,
        type_name: &str,
        variant_name: &str,
        index: usize,
    ) -> Option<SchemaDefaultValue> {
        self.enum_tuple_fields
            .get(&(type_name.to_owned(), variant_name.to_owned(), index))
            .cloned()
    }

    fn enum_record_field(
        &self,
        type_name: &str,
        variant_name: &str,
        field_name: &str,
    ) -> Option<SchemaDefaultValue> {
        self.enum_record_fields
            .get(&(
                type_name.to_owned(),
                variant_name.to_owned(),
                field_name.to_owned(),
            ))
            .cloned()
    }
}

fn schema_field_default(
    name: String,
    value: SchemaDefaultValue,
    constants: BTreeMap<String, Constant>,
) -> SchemaFieldDefault {
    SchemaFieldDefault {
        name,
        value,
        constants,
    }
}

pub(super) fn record_constructor_diagnostics(
    type_name: &str,
    shape: Option<&ConstructorShape>,
    fields: &[RecordField],
    field_names: Option<&[Option<String>]>,
    constructor_span: Span,
) -> Vec<Diagnostic> {
    let mut diagnostics = duplicate_record_field_diagnostics(fields, field_names);
    let Some(shape) = shape else {
        return diagnostics;
    };
    let explicit = fields
        .iter()
        .enumerate()
        .map(|(index, field)| record_field_name(field_names, index, field).to_owned())
        .collect::<BTreeSet<_>>();

    for (index, field) in fields.iter().enumerate() {
        let field_name = record_field_name(field_names, index, field);
        if !shape.contains_field(field_name) {
            diagnostics.push(unknown_field_diagnostic(
                type_name,
                field_name,
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

pub(super) fn tuple_constructor_diagnostics(
    type_name: &str,
    variant: &str,
    shape: Option<&ConstructorShape>,
    args: &[Argument],
    arg_names: Option<&[Option<String>]>,
    constructor_span: Span,
) -> Vec<Diagnostic> {
    let Some(shape) = shape else {
        return Vec::new();
    };
    let owner = format!("{type_name}::{variant}");
    match resolve_tuple_constructor_arguments(shape, &owner, args, arg_names, constructor_span) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    }
}

pub(super) fn resolve_tuple_constructor_arguments<'ast>(
    shape: &ConstructorShape,
    owner: &str,
    args: &'ast [Argument],
    arg_names: Option<&[Option<String>]>,
    constructor_span: Span,
) -> Result<Vec<Option<&'ast Argument>>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut slots = vec![None; shape.len()];
    let mut slot_spans = vec![None; shape.len()];
    let mut next_positional = 0_usize;
    let mut seen_named = false;

    for (arg_index, arg) in args.iter().enumerate() {
        let arg_span = arg.value.span;
        let arg_name = arg_names
            .and_then(|names| names.get(arg_index))
            .and_then(|name| name.as_deref())
            .or(arg.name.as_deref());
        let Some(index) = tuple_argument_index(
            shape,
            arg_name,
            arg_span,
            &mut next_positional,
            &mut seen_named,
            &mut diagnostics,
            owner,
        ) else {
            continue;
        };

        if let Some(previous_span) = slot_spans[index] {
            diagnostics.push(duplicate_constructor_field_diagnostic(
                shape.fields[index].argument_name.as_str(),
                previous_span,
                arg_span,
            ));
            continue;
        }
        slots[index] = Some(arg);
        slot_spans[index] = Some(arg_span);
    }

    for (slot, field) in slots.iter().zip(&shape.fields) {
        if slot.is_none() && field.default.is_none() {
            diagnostics.push(missing_field_diagnostic(
                owner,
                &field.argument_name,
                constructor_span,
            ));
        }
    }

    if diagnostics.is_empty() {
        Ok(slots)
    } else {
        Err(diagnostics)
    }
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

fn duplicate_record_field_diagnostics(
    fields: &[RecordField],
    field_names: Option<&[Option<String>]>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = BTreeMap::<&str, Span>::new();
    for (index, field) in fields.iter().enumerate() {
        let field_name = record_field_name(field_names, index, field);
        if let Some(previous_span) = seen.insert(field_name, field.span) {
            diagnostics.push(duplicate_constructor_field_diagnostic(
                field_name,
                previous_span,
                field.span,
            ));
        }
    }
    diagnostics
}

fn record_field_name<'field>(
    field_names: Option<&'field [Option<String>]>,
    index: usize,
    field: &'field RecordField,
) -> &'field str {
    field_names
        .and_then(|names| names.get(index))
        .and_then(|name| name.as_deref())
        .unwrap_or(field.name.as_str())
}

fn tuple_argument_index(
    shape: &ConstructorShape,
    arg_name: Option<&str>,
    arg_span: Span,
    next_positional: &mut usize,
    seen_named: &mut bool,
    diagnostics: &mut Vec<Diagnostic>,
    owner: &str,
) -> Option<usize> {
    if let Some(name) = arg_name {
        *seen_named = true;
        return match shape.argument_index(name) {
            Some(index) => Some(index),
            None => {
                diagnostics.push(unknown_field_diagnostic(
                    owner,
                    name,
                    arg_span,
                    shape.argument_names(),
                ));
                None
            }
        };
    }

    if *seen_named {
        diagnostics.push(
            Diagnostic::error("positional argument after named argument")
                .with_code("compiler::positional_after_named_argument")
                .with_span(arg_span)
                .with_label(
                    arg_span,
                    "positional arguments must appear before named arguments",
                ),
        );
        return None;
    }

    let index = *next_positional;
    *next_positional = next_positional.saturating_add(1);
    if index >= shape.len() {
        diagnostics.push(unknown_field_diagnostic(
            owner,
            &index.to_string(),
            arg_span,
            shape.argument_names(),
        ));
        return None;
    }
    Some(index)
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
    enum_name: &str,
    variant_name: &str,
    fields: &EnumVariantFieldsHint,
    default_payloads: &SchemaDefaultPayloads,
    constants: BTreeMap<String, Constant>,
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
                default: field
                    .default_value_span
                    .as_ref()
                    .and_then(|_| default_payloads.enum_tuple_field(enum_name, variant_name, index))
                    .map(|value| schema_field_default(index.to_string(), value, constants.clone())),
            })
            .collect(),
        EnumVariantFieldsHint::Record(fields) => fields
            .iter()
            .map(|field| ConstructorField {
                name: field.name.clone(),
                argument_name: field.name.clone(),
                value_type: field.type_hint.as_ref().and_then(type_hint_value_type),
                default: field
                    .default_value_span
                    .as_ref()
                    .and_then(|_| {
                        default_payloads.enum_record_field(enum_name, variant_name, &field.name)
                    })
                    .map(|value| {
                        schema_field_default(field.name.clone(), value, constants.clone())
                    }),
            })
            .collect(),
    }
}
