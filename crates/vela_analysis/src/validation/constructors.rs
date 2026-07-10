use std::collections::{BTreeMap, BTreeSet};

use vela_common::{Diagnostic, Span};
use vela_hir::body::{HirArgument, HirBody, HirExprKind, HirRecordField};
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, ModuleId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{EnumVariantFieldsHint, HirTypeHint};

use super::{
    ConstructorFieldSlotFact, ConstructorInputKindFact, ConstructorPlacementFact,
    ConstructorSlotValueFact, ConstructorSourceValueFact, ExecutableValidationFacts,
};
use crate::facts::AnalysisFacts;
use crate::hints::type_fact_from_hint_in_module;
use crate::registry::RegistryFacts;
use crate::semantic_facts::{CallTargetFact, ConstructorTargetFact};
use crate::type_fact::TypeFact;

pub(super) fn record_body(
    validation: &mut ExecutableValidationFacts,
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    facts: &AnalysisFacts,
    body: &HirBody,
) {
    let context = ConstructorContext {
        graph,
        schema,
        facts,
        body,
    };
    for expression in body.expressions.values() {
        match &expression.kind {
            HirExprKind::Record { fields, .. } => record_field_constructor(
                validation,
                &context,
                expression.id,
                expression.origin.span,
                fields,
            ),
            HirExprKind::Call(call)
                if matches!(
                    facts.call_target(expression.id),
                    Some(CallTargetFact::Variant { .. })
                ) =>
            {
                tuple_argument_constructor(
                    validation,
                    &context,
                    expression.id,
                    expression.origin.span,
                    &call.arguments,
                );
            }
            _ => {}
        }
    }
}

struct ConstructorContext<'analysis> {
    graph: &'analysis ModuleGraph,
    schema: Option<&'analysis RegistryFacts>,
    facts: &'analysis AnalysisFacts,
    body: &'analysis HirBody,
}

fn record_field_constructor(
    validation: &mut ExecutableValidationFacts,
    context: &ConstructorContext<'_>,
    expression: HirExprId,
    constructor_span: Span,
    fields: &[HirRecordField],
) {
    let target = context
        .facts
        .constructor_target(expression)
        .cloned()
        .unwrap_or(ConstructorTargetFact::Unresolved);
    let source_order = record_source_values(fields);
    let uses = source_order
        .iter()
        .map(|field| FieldUse {
            source_index: field.source_index,
            name: field.name.clone().unwrap_or_default(),
            span: field.span,
        })
        .collect::<Vec<_>>();

    let (slots, diagnostics) = match constructor_shape(context.graph, context.schema, &target) {
        ConstructorShapeLookup::Known {
            display_name,
            specs,
        } => place_fields(
            &display_name,
            &specs,
            &uses,
            &source_order,
            constructor_span,
        ),
        ConstructorShapeLookup::UnknownVariant { enum_name, variant } => (
            None,
            vec![unknown_variant_diagnostic(
                &enum_name,
                &variant,
                constructor_span,
            )],
        ),
        ConstructorShapeLookup::Dynamic => {
            let display_name = constructor_path(context.body, expression)
                .map(|path| path.join("::"))
                .unwrap_or_default();
            if let Some((enum_name, variant)) = context
                .schema
                .and_then(|schema| registered_unknown_variant(schema, context.body, expression))
            {
                (
                    None,
                    vec![unknown_variant_diagnostic(
                        &enum_name,
                        &variant,
                        constructor_span,
                    )],
                )
            } else {
                let diagnostics = field_diagnostics(&display_name, None, &uses, constructor_span);
                (None, diagnostics)
            }
        }
        ConstructorShapeLookup::Unavailable => (None, Vec::new()),
    };
    record_diagnostics(validation, expression, constructor_span, diagnostics);
    validation.constructors.insert(
        expression,
        ConstructorPlacementFact {
            target,
            input_kind: ConstructorInputKindFact::RecordFields,
            source_order,
            declaration_slots: slots,
        },
    );
}

fn tuple_argument_constructor(
    validation: &mut ExecutableValidationFacts,
    context: &ConstructorContext<'_>,
    expression: HirExprId,
    constructor_span: Span,
    arguments: &[HirArgument],
) {
    let Some(CallTargetFact::Variant {
        enum_declaration,
        variant,
    }) = context.facts.call_target(expression)
    else {
        return;
    };
    let target = ConstructorTargetFact::Variant {
        enum_declaration: *enum_declaration,
        variant: variant.clone(),
    };
    let source_order = argument_source_values(arguments);
    let (slots, diagnostics) = match source_variant_shape(context.graph, *enum_declaration, variant)
    {
        ConstructorShapeLookup::Known {
            display_name,
            specs,
        } => {
            let uses = tuple_field_uses(&specs, &source_order);
            let diagnostics =
                field_diagnostics(&display_name, Some(&specs), &uses, constructor_span);
            let has_call_diagnostics = validation
                .call_diagnostic_batches
                .iter()
                .any(|(call, _, _)| *call == expression);
            let slots = (diagnostics.is_empty() && !has_call_diagnostics)
                .then(|| declaration_slots(&specs, &uses, &source_order))
                .flatten();
            (slots, diagnostics)
        }
        ConstructorShapeLookup::UnknownVariant { enum_name, variant } => (
            None,
            vec![unknown_variant_diagnostic(
                &enum_name,
                &variant,
                constructor_span,
            )],
        ),
        ConstructorShapeLookup::Dynamic | ConstructorShapeLookup::Unavailable => (None, Vec::new()),
    };
    record_diagnostics(validation, expression, constructor_span, diagnostics);
    validation.constructors.insert(
        expression,
        ConstructorPlacementFact {
            target,
            input_kind: ConstructorInputKindFact::TupleArguments,
            source_order,
            declaration_slots: slots,
        },
    );
}

fn record_diagnostics(
    validation: &mut ExecutableValidationFacts,
    expression: HirExprId,
    span: Span,
    diagnostics: Vec<Diagnostic>,
) {
    if !diagnostics.is_empty() {
        validation
            .constructor_diagnostic_batches
            .push((expression, span, diagnostics));
    }
}

enum ConstructorShapeLookup {
    Known {
        display_name: String,
        specs: Vec<FieldSpec>,
    },
    UnknownVariant {
        enum_name: String,
        variant: String,
    },
    Dynamic,
    Unavailable,
}

#[derive(Clone)]
struct FieldSpec {
    declaration_index: usize,
    field_name: String,
    parameter_name: String,
    expected: TypeFact,
    declaration_span: Option<Span>,
    default: FieldDefault,
}

#[derive(Clone, Copy)]
enum FieldDefault {
    Required,
    Source(HirBodyId),
    SourceUnavailable(Option<HirBodyId>),
    RegisteredUnavailable,
}

fn constructor_shape(
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    target: &ConstructorTargetFact,
) -> ConstructorShapeLookup {
    match target {
        ConstructorTargetFact::Declaration(declaration) => source_record_shape(graph, *declaration),
        ConstructorTargetFact::Variant {
            enum_declaration,
            variant,
        } => source_variant_shape(graph, *enum_declaration, variant),
        ConstructorTargetFact::RegistryType { path } => schema
            .map_or(ConstructorShapeLookup::Unavailable, |schema| {
                registered_shape(schema, path, path)
            }),
        ConstructorTargetFact::RegistryVariant { owner, variant } => {
            let field_owner = format!("{owner}::{variant}");
            schema.map_or(ConstructorShapeLookup::Unavailable, |schema| {
                registered_shape(schema, &field_owner, &field_owner)
            })
        }
        ConstructorTargetFact::Dynamic => ConstructorShapeLookup::Dynamic,
        ConstructorTargetFact::Unresolved => ConstructorShapeLookup::Unavailable,
    }
}

fn source_record_shape(graph: &ModuleGraph, declaration: HirDeclId) -> ConstructorShapeLookup {
    let Some(metadata) = graph.declaration(declaration) else {
        return ConstructorShapeLookup::Unavailable;
    };
    if metadata.kind != DeclarationKind::Struct {
        return ConstructorShapeLookup::Unavailable;
    }
    let Some(shape) = graph.struct_shape(declaration) else {
        return ConstructorShapeLookup::Unavailable;
    };
    let specs = shape
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| FieldSpec {
            declaration_index: index,
            field_name: field.name.clone(),
            parameter_name: field.name.clone(),
            expected: hint_fact(graph, metadata.module, field.type_hint.as_ref()),
            declaration_span: Some(field.span),
            default: source_default(graph, field.default_value_span, field.default_body),
        })
        .collect();
    ConstructorShapeLookup::Known {
        display_name: metadata.name.clone(),
        specs,
    }
}

fn source_variant_shape(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    variant_name: &str,
) -> ConstructorShapeLookup {
    let Some(metadata) = graph.declaration(declaration) else {
        return ConstructorShapeLookup::Unavailable;
    };
    if metadata.kind != DeclarationKind::Enum {
        return ConstructorShapeLookup::Unavailable;
    }
    let Some(shape) = graph.enum_shape(declaration) else {
        return ConstructorShapeLookup::Unavailable;
    };
    let Some(variant) = shape
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)
    else {
        return ConstructorShapeLookup::UnknownVariant {
            enum_name: metadata.name.clone(),
            variant: variant_name.to_owned(),
        };
    };
    let specs = match &variant.fields {
        EnumVariantFieldsHint::Unit => Vec::new(),
        EnumVariantFieldsHint::Tuple(parameters) => parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| FieldSpec {
                declaration_index: index,
                field_name: index.to_string(),
                parameter_name: parameter.name.clone(),
                expected: hint_fact(graph, metadata.module, parameter.type_hint.as_ref()),
                declaration_span: Some(parameter.span),
                default: source_default(
                    graph,
                    parameter.default_value_span,
                    parameter.default_body,
                ),
            })
            .collect(),
        EnumVariantFieldsHint::Record(fields) => fields
            .iter()
            .enumerate()
            .map(|(index, field)| FieldSpec {
                declaration_index: index,
                field_name: field.name.clone(),
                parameter_name: field.name.clone(),
                expected: hint_fact(graph, metadata.module, field.type_hint.as_ref()),
                declaration_span: Some(field.span),
                default: source_default(graph, field.default_value_span, field.default_body),
            })
            .collect(),
    };
    ConstructorShapeLookup::Known {
        display_name: format!("{}::{variant_name}", metadata.name),
        specs,
    }
}

fn registered_shape(
    schema: &RegistryFacts,
    owner: &str,
    display_name: &str,
) -> ConstructorShapeLookup {
    let Some(members) = schema.fields_for_exact_or_unique_short_name(owner) else {
        return ConstructorShapeLookup::Unavailable;
    };
    let targets = schema.field_targets_for_owner_or_short_name(owner);
    let target_keys = targets
        .iter()
        .map(|target| (target.owner_name.as_str(), target.name.as_str()))
        .collect::<BTreeSet<_>>();
    if members
        .iter()
        .any(|member| !target_keys.contains(&(member.owner.as_str(), member.name.as_str())))
    {
        return ConstructorShapeLookup::Unavailable;
    }
    let specs = targets
        .into_iter()
        .map(|target| FieldSpec {
            declaration_index: usize::try_from(target.declaration_order)
                .expect("u32 declaration order must fit usize"),
            field_name: target.name.clone(),
            parameter_name: target.name.clone(),
            expected: schema
                .field_fact(&target.owner_name, &target.name)
                .cloned()
                .unwrap_or(TypeFact::Unknown),
            declaration_span: None,
            default: if target.has_default {
                FieldDefault::RegisteredUnavailable
            } else {
                FieldDefault::Required
            },
        })
        .collect();
    ConstructorShapeLookup::Known {
        display_name: display_name.to_owned(),
        specs,
    }
}

fn hint_fact(graph: &ModuleGraph, module: ModuleId, hint: Option<&HirTypeHint>) -> TypeFact {
    hint.map_or(TypeFact::Unknown, |hint| {
        type_fact_from_hint_in_module(graph, module, hint)
    })
}

fn source_default(
    graph: &ModuleGraph,
    declared: Option<Span>,
    body: Option<HirBodyId>,
) -> FieldDefault {
    match (declared, body) {
        (None, None) => FieldDefault::Required,
        (Some(_), Some(body)) if graph.body(body).is_some() => FieldDefault::Source(body),
        (_, body) => FieldDefault::SourceUnavailable(body),
    }
}

fn record_source_values(fields: &[HirRecordField]) -> Vec<ConstructorSourceValueFact> {
    fields
        .iter()
        .enumerate()
        .map(|(source_index, field)| ConstructorSourceValueFact {
            source_index,
            name: Some(field.name.clone()),
            value: field.value,
            span: field.name_origin.span,
        })
        .collect()
}

fn argument_source_values(arguments: &[HirArgument]) -> Vec<ConstructorSourceValueFact> {
    arguments
        .iter()
        .enumerate()
        .map(|(source_index, argument)| ConstructorSourceValueFact {
            source_index,
            name: argument.name.clone(),
            value: argument.value,
            span: argument.origin.span,
        })
        .collect()
}

#[derive(Clone)]
struct FieldUse {
    source_index: usize,
    name: String,
    span: Span,
}

fn tuple_field_uses(
    specs: &[FieldSpec],
    source_order: &[ConstructorSourceValueFact],
) -> Vec<FieldUse> {
    source_order
        .iter()
        .map(|argument| {
            let name = match argument.name.as_deref() {
                Some(name) => specs
                    .iter()
                    .find(|spec| spec.parameter_name == name)
                    .map_or_else(|| name.to_owned(), |spec| spec.field_name.clone()),
                None => specs.get(argument.source_index).map_or_else(
                    || argument.source_index.to_string(),
                    |spec| spec.field_name.clone(),
                ),
            };
            FieldUse {
                source_index: argument.source_index,
                name,
                span: argument.span,
            }
        })
        .collect()
}

fn place_fields(
    display_name: &str,
    specs: &[FieldSpec],
    uses: &[FieldUse],
    source_order: &[ConstructorSourceValueFact],
    constructor_span: Span,
) -> (Option<Vec<ConstructorFieldSlotFact>>, Vec<Diagnostic>) {
    let diagnostics = field_diagnostics(display_name, Some(specs), uses, constructor_span);
    let slots = diagnostics
        .is_empty()
        .then(|| declaration_slots(specs, uses, source_order))
        .flatten();
    (slots, diagnostics)
}

fn declaration_slots(
    specs: &[FieldSpec],
    uses: &[FieldUse],
    source_order: &[ConstructorSourceValueFact],
) -> Option<Vec<ConstructorFieldSlotFact>> {
    let by_name = uses
        .iter()
        .map(|field| (field.name.as_str(), field.source_index))
        .collect::<BTreeMap<_, _>>();
    specs
        .iter()
        .map(|spec| {
            let value = if let Some(source_index) = by_name.get(spec.field_name.as_str()).copied() {
                let source = source_order.get(source_index)?;
                ConstructorSlotValueFact::Explicit {
                    source_index,
                    value: source.value,
                }
            } else {
                match spec.default {
                    FieldDefault::Required => return None,
                    FieldDefault::Source(body) => ConstructorSlotValueFact::SourceDefault { body },
                    FieldDefault::SourceUnavailable(body) => {
                        ConstructorSlotValueFact::SourceDefaultUnavailable { body }
                    }
                    FieldDefault::RegisteredUnavailable => {
                        ConstructorSlotValueFact::RegisteredDefaultUnavailable
                    }
                }
            };
            Some(ConstructorFieldSlotFact {
                declaration_index: spec.declaration_index,
                field_name: spec.field_name.clone(),
                parameter_name: spec.parameter_name.clone(),
                expected: spec.expected.clone(),
                declaration_span: spec.declaration_span,
                value,
            })
        })
        .collect()
}

fn field_diagnostics(
    type_name: &str,
    specs: Option<&[FieldSpec]>,
    fields: &[FieldUse],
    constructor_span: Span,
) -> Vec<Diagnostic> {
    let mut diagnostics = duplicate_field_diagnostics(fields);
    let Some(specs) = specs else {
        return diagnostics;
    };
    let known = specs
        .iter()
        .map(|field| field.field_name.as_str())
        .collect::<BTreeSet<_>>();
    let explicit = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<BTreeSet<_>>();
    let candidates = specs
        .iter()
        .map(|field| field.field_name.as_str())
        .collect::<Vec<_>>();
    for field in fields {
        if !known.contains(field.name.as_str()) {
            diagnostics.push(unknown_field_diagnostic(
                type_name,
                &field.name,
                field.span,
                &candidates,
            ));
        }
    }
    for field in specs {
        if matches!(field.default, FieldDefault::Required)
            && !explicit.contains(field.field_name.as_str())
        {
            diagnostics.push(missing_field_diagnostic(
                type_name,
                &field.field_name,
                constructor_span,
            ));
        }
    }
    diagnostics
}

fn duplicate_field_diagnostics(fields: &[FieldUse]) -> Vec<Diagnostic> {
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
    candidates: &[&str],
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

fn unknown_variant_diagnostic(enum_name: &str, variant: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!("unknown enum variant `{enum_name}::{variant}`"))
        .with_code("compiler::unknown_constructor_variant")
        .with_span(span)
        .with_label(span, "variant is not declared on this enum")
}

fn constructor_path(body: &HirBody, expression: HirExprId) -> Option<&[String]> {
    let HirExprKind::Record { constructor, .. } = &body.expression(expression)?.kind else {
        return None;
    };
    let constructor = constructor.as_ref()?;
    Some(body.paths.get(constructor)?.path.as_slice())
}

fn registered_unknown_variant(
    schema: &RegistryFacts,
    body: &HirBody,
    expression: HirExprId,
) -> Option<(String, String)> {
    let (variant, owner) = constructor_path(body, expression)?.split_last()?;
    if owner.is_empty() {
        return None;
    }
    let owner = owner.join("::");
    let owner_fact = schema.type_fact(&owner).or_else(|| {
        owner
            .rsplit("::")
            .next()
            .and_then(|name| schema.type_fact(name))
    })?;
    if !matches!(owner_fact, TypeFact::Enum { .. })
        || schema
            .variant_for_owner_or_unique_short_name(&owner, variant)
            .is_some()
    {
        return None;
    }
    Some((owner, variant.clone()))
}
