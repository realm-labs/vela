use std::collections::BTreeSet;

use vela_common::{CallableAsyncness, Diagnostic, Span};
use vela_hir::body::{HirArgument, HirBody, HirExprKind};
use vela_hir::ids::{HirDeclId, HirNodeId, ModuleId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{EnumVariantFieldsHint, FunctionSignature, ParamHint};

use super::{
    CallArgumentPlacementFact, CallParameterSlotFact, CallParameterSlotValueFact,
    CallPlacementModeFact, CallSourceArgumentFact, ExecutableValidationFacts,
};
use crate::callable::{
    CallableParameterFact, CallableParameterRequirementFact, CallableSignatureFact,
};
use crate::facts::AnalysisFacts;
use crate::hints::type_fact_from_hint_in_module;
use crate::registry::RegistryFacts;
use crate::semantic_facts::{CallTargetFact, registry_callable_owner};
use crate::stdlib::stdlib_method_fact_for_call;
use crate::type_fact::TypeFact;

pub(super) fn record_body(
    validation: &mut ExecutableValidationFacts,
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    facts: &AnalysisFacts,
    body: &HirBody,
) {
    let awaited_calls = body
        .expressions
        .values()
        .filter_map(|expression| match expression.kind {
            HirExprKind::Await { expression } => expression,
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for (expression, call) in body.calls() {
        let Some(target) = facts.call_target(expression) else {
            continue;
        };
        let source_order = source_arguments(&call.arguments);
        let policy = placement_policy(graph, schema, facts, body, call, target);
        let call_span = body
            .expression(expression)
            .map_or(body.origin.span, |expression| expression.origin.span);
        let mut call_diagnostics = Vec::new();
        if policy.asyncness().is_async() && !awaited_calls.contains(&expression) {
            call_diagnostics.push(async_call_requires_await_diagnostic(call_span));
        }
        let (mode, parameter_slots) = match policy {
            PlacementPolicy::Strict(signature) => {
                let slots =
                    resolve_arguments(&signature, &source_order, call_span, &mut call_diagnostics);
                (CallPlacementModeFact::Strict, slots)
            }
            PlacementPolicy::External(signature) => {
                if source_order.iter().all(|argument| argument.name.is_none()) {
                    (CallPlacementModeFact::ExternalPositional, None)
                } else if let Some(signature) = signature {
                    let slots = resolve_arguments(
                        &signature,
                        &source_order,
                        call_span,
                        &mut call_diagnostics,
                    );
                    (CallPlacementModeFact::ExternalNamed, slots)
                } else {
                    (CallPlacementModeFact::Unresolved, None)
                }
            }
            PlacementPolicy::ExactExternal(signature) => {
                let slots =
                    resolve_arguments(&signature, &source_order, call_span, &mut call_diagnostics);
                let mode = if source_order.iter().all(|argument| argument.name.is_none()) {
                    CallPlacementModeFact::ExternalPositional
                } else {
                    CallPlacementModeFact::ExternalNamed
                };
                (mode, slots)
            }
            PlacementPolicy::Dynamic => (CallPlacementModeFact::Dynamic, None),
            PlacementPolicy::Positional => {
                reject_named_positional_arguments(&source_order, &mut call_diagnostics);
                (CallPlacementModeFact::Positional, None)
            }
            PlacementPolicy::Unresolved => (CallPlacementModeFact::Unresolved, None),
        };
        if !call_diagnostics.is_empty() {
            validation
                .call_diagnostic_batches
                .push((expression, call_span, call_diagnostics));
        }
        validation.calls.insert(
            expression,
            CallArgumentPlacementFact {
                mode,
                source_order,
                parameter_slots,
            },
        );
    }
}

enum PlacementPolicy {
    Strict(CallableSignatureFact),
    External(Option<CallableSignatureFact>),
    ExactExternal(CallableSignatureFact),
    Dynamic,
    Positional,
    Unresolved,
}

impl PlacementPolicy {
    const fn asyncness(&self) -> CallableAsyncness {
        match self {
            Self::Strict(signature) | Self::ExactExternal(signature) => signature.asyncness,
            Self::External(Some(signature)) => signature.asyncness,
            Self::External(None) | Self::Dynamic | Self::Positional | Self::Unresolved => {
                CallableAsyncness::Sync
            }
        }
    }
}

fn placement_policy(
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    facts: &AnalysisFacts,
    body: &HirBody,
    call: &vela_hir::body::HirCall,
    target: &CallTargetFact,
) -> PlacementPolicy {
    match target {
        CallTargetFact::Declaration(declaration) => source_function_signature(graph, *declaration)
            .map_or(PlacementPolicy::Unresolved, PlacementPolicy::Strict),
        CallTargetFact::Variant {
            enum_declaration,
            variant,
        } => source_variant_signature(graph, *enum_declaration, variant)
            .map_or(PlacementPolicy::Unresolved, PlacementPolicy::Strict),
        CallTargetFact::RegistryVariant { owner, variant } => schema
            .and_then(|schema| registry_variant_signature(schema, owner, variant))
            .map_or(PlacementPolicy::Unresolved, PlacementPolicy::ExactExternal),
        CallTargetFact::ScriptMethod { method } => source_method_signature(graph, *method)
            .map_or(PlacementPolicy::Unresolved, PlacementPolicy::Strict),
        CallTargetFact::RegistryFunction { path }
        | CallTargetFact::NativeFunction { path }
        | CallTargetFact::StdlibFunction { path } => {
            let signature = schema.and_then(|schema| schema.function_signature_fact(path).cloned());
            if path == "set::from_array" {
                signature.map_or(
                    PlacementPolicy::External(None),
                    PlacementPolicy::ExactExternal,
                )
            } else {
                PlacementPolicy::External(signature)
            }
        }
        CallTargetFact::HostMethod { owner, name }
        | CallTargetFact::RegistryMethod { owner, name } => PlacementPolicy::External(
            schema.and_then(|schema| registry_method_signature(schema, owner, name)),
        ),
        CallTargetFact::StdlibMethod { name } => {
            let signature = body.field(call.callee).and_then(|field| {
                let receiver = facts.expression(field.receiver)?;
                let owner = registry_callable_owner(receiver)?;
                let signature =
                    schema.and_then(|schema| registry_method_signature(schema, owner, name))?;
                Some(specialize_stdlib_method_signature(
                    facts, call, receiver, name, signature,
                ))
            });
            PlacementPolicy::External(signature)
        }
        CallTargetFact::Local(_) | CallTargetFact::Lambda(_) => PlacementPolicy::Positional,
        CallTargetFact::Dynamic => PlacementPolicy::Dynamic,
        CallTargetFact::KnownReceiverMiss { .. } | CallTargetFact::Unresolved => {
            PlacementPolicy::Unresolved
        }
    }
}

fn specialize_stdlib_method_signature(
    facts: &AnalysisFacts,
    call: &vela_hir::body::HirCall,
    receiver: &TypeFact,
    method: &str,
    mut signature: CallableSignatureFact,
) -> CallableSignatureFact {
    if method != "extend" {
        return signature;
    }
    let arguments = call
        .arguments
        .iter()
        .map(|argument| {
            argument
                .value
                .and_then(|value| facts.expression(value).cloned())
                .unwrap_or(TypeFact::Unknown)
        })
        .collect::<Vec<_>>();
    let Some(method) =
        stdlib_method_fact_for_call(receiver, method, None, None, arguments.as_slice())
    else {
        return signature;
    };
    for (parameter, specialized) in signature.parameters.iter_mut().zip(method.params) {
        parameter.type_fact = specialized;
    }
    signature
}

fn registry_variant_signature(
    schema: &RegistryFacts,
    owner: &str,
    variant: &str,
) -> Option<CallableSignatureFact> {
    let target = schema.variant_for_owner_or_unique_short_name(owner, variant)?;
    let field_owner = format!("{}::{}", target.owner, target.name);
    let parameters = schema
        .field_targets_for_owner_or_short_name(&field_owner)
        .into_iter()
        .map(|field| {
            CallableParameterFact::new(
                &field.name,
                schema
                    .field_fact(&field.owner_name, &field.name)
                    .cloned()
                    .unwrap_or(TypeFact::Unknown),
                requirement(field.has_default),
            )
        })
        .collect::<Vec<_>>();
    Some(CallableSignatureFact::new(parameters, target.fact))
}

fn source_function_signature(
    graph: &ModuleGraph,
    declaration: HirDeclId,
) -> Option<CallableSignatureFact> {
    let declaration_metadata = graph.declaration(declaration)?;
    let signature = graph.function_signature(declaration)?;
    Some(hir_signature(
        graph,
        declaration_metadata.module,
        signature,
        false,
    ))
}

fn source_variant_signature(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    variant: &str,
) -> Option<CallableSignatureFact> {
    let declaration_metadata = graph.declaration(declaration)?;
    let variant = graph
        .enum_shape(declaration)?
        .variants
        .iter()
        .find(|candidate| candidate.name == variant)?;
    let parameters = match &variant.fields {
        EnumVariantFieldsHint::Unit => Vec::new(),
        EnumVariantFieldsHint::Tuple(parameters) => {
            hir_parameters(graph, declaration_metadata.module, parameters.iter())
        }
        EnumVariantFieldsHint::Record(fields) => fields
            .iter()
            .map(|field| {
                CallableParameterFact::new(
                    &field.name,
                    field.type_hint.as_ref().map_or(TypeFact::Unknown, |hint| {
                        type_fact_from_hint_in_module(graph, declaration_metadata.module, hint)
                    }),
                    requirement(field.default_value_span.is_some() || field.default_body.is_some()),
                )
                .declared_at(field.span)
            })
            .collect(),
    };
    Some(CallableSignatureFact::new(
        parameters,
        TypeFact::enum_type(&declaration_metadata.name, Some(&variant.name)),
    ))
}

fn source_method_signature(
    graph: &ModuleGraph,
    method: HirNodeId,
) -> Option<CallableSignatureFact> {
    for declaration in graph.declarations_by_kind(DeclarationKind::Impl) {
        let Some(metadata) = graph.impl_metadata(declaration.id) else {
            continue;
        };
        if let Some(method) = metadata
            .methods
            .iter()
            .find(|candidate| candidate.node == method)
        {
            return Some(hir_signature(
                graph,
                declaration.module,
                &method.signature,
                true,
            ));
        }
    }
    for declaration in graph.declarations_by_kind(DeclarationKind::Trait) {
        let Some(shape) = graph.trait_shape(declaration.id) else {
            continue;
        };
        if let Some(method) = shape
            .methods
            .iter()
            .find(|candidate| candidate.default_body_node == Some(method))
        {
            return Some(hir_signature(
                graph,
                declaration.module,
                &method.signature,
                true,
            ));
        }
    }
    None
}

fn hir_signature(
    graph: &ModuleGraph,
    module: ModuleId,
    signature: &FunctionSignature,
    skip_receiver: bool,
) -> CallableSignatureFact {
    let parameters = hir_parameters(
        graph,
        module,
        signature.params.iter().skip(usize::from(skip_receiver)),
    );
    let returns = signature
        .return_type
        .as_ref()
        .map_or(TypeFact::Unknown, |hint| {
            type_fact_from_hint_in_module(graph, module, hint)
        });
    CallableSignatureFact::new(parameters, returns).asyncness(signature.asyncness)
}

fn hir_parameters<'a>(
    graph: &ModuleGraph,
    module: ModuleId,
    parameters: impl Iterator<Item = &'a ParamHint>,
) -> Vec<CallableParameterFact> {
    parameters
        .map(|parameter| {
            CallableParameterFact::new(
                &parameter.name,
                parameter
                    .type_hint
                    .as_ref()
                    .map_or(TypeFact::Unknown, |hint| {
                        type_fact_from_hint_in_module(graph, module, hint)
                    }),
                requirement(
                    parameter.default_value_span.is_some() || parameter.default_body.is_some(),
                ),
            )
            .declared_at(parameter.span)
        })
        .collect()
}

const fn requirement(has_default: bool) -> CallableParameterRequirementFact {
    if has_default {
        CallableParameterRequirementFact::Defaulted
    } else {
        CallableParameterRequirementFact::Required
    }
}

fn registry_method_signature(
    schema: &RegistryFacts,
    owner: &str,
    name: &str,
) -> Option<CallableSignatureFact> {
    schema
        .method_signature_fact(owner, name)
        .or_else(|| schema.trait_method_signature_fact(owner, name))
        .cloned()
}

fn source_arguments(arguments: &[HirArgument]) -> Vec<CallSourceArgumentFact> {
    arguments
        .iter()
        .enumerate()
        .map(|(source_index, argument)| CallSourceArgumentFact {
            source_index,
            name: argument.name.clone(),
            value: argument.value,
            span: argument.origin.span,
        })
        .collect()
}

fn resolve_arguments(
    signature: &CallableSignatureFact,
    arguments: &[CallSourceArgumentFact],
    call_span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<CallParameterSlotFact>> {
    let mut slots = vec![None; signature.parameters.len()];
    let mut slot_spans = vec![None; signature.parameters.len()];
    let mut next_positional = 0_usize;
    let mut seen_named = false;
    let diagnostics_start = diagnostics.len();

    for argument in arguments {
        let Some(index) = argument_index(
            signature,
            argument,
            &mut next_positional,
            &mut seen_named,
            diagnostics,
        ) else {
            continue;
        };
        if let Some(previous_span) = slot_spans[index] {
            diagnostics.push(duplicate_argument_diagnostic(
                &signature.parameters[index].name,
                previous_span,
                argument.span,
            ));
            continue;
        }
        slots[index] = Some(argument);
        slot_spans[index] = Some(argument.span);
    }

    for (slot, parameter) in slots.iter().zip(&signature.parameters) {
        if slot.is_none() && parameter.requirement.is_required() {
            diagnostics.push(missing_argument_diagnostic(parameter, call_span));
        }
    }
    if diagnostics.len() != diagnostics_start {
        return None;
    }

    Some(
        slots
            .into_iter()
            .zip(&signature.parameters)
            .enumerate()
            .map(
                |(parameter_index, (argument, parameter))| CallParameterSlotFact {
                    parameter_index,
                    name: parameter.name.clone(),
                    value: argument.map_or(
                        CallParameterSlotValueFact::MissingDefault,
                        |argument| CallParameterSlotValueFact::Explicit {
                            source_index: argument.source_index,
                            value: argument.value,
                        },
                    ),
                },
            )
            .collect(),
    )
}

fn argument_index(
    signature: &CallableSignatureFact,
    argument: &CallSourceArgumentFact,
    next_positional: &mut usize,
    seen_named: &mut bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<usize> {
    if let Some(name) = argument.name.as_deref() {
        *seen_named = true;
        return signature
            .parameters
            .iter()
            .position(|parameter| parameter.name == name)
            .or_else(|| {
                diagnostics.push(unknown_named_argument_diagnostic(
                    name,
                    argument.span,
                    signature,
                ));
                None
            });
    }
    if *seen_named {
        diagnostics.push(positional_after_named_diagnostic(argument.span));
        return None;
    }
    let index = *next_positional;
    *next_positional = index
        .checked_add(1)
        .expect("finite HIR call argument count must fit usize");
    if index >= signature.parameters.len() {
        diagnostics.push(too_many_arguments_diagnostic(
            argument.span,
            signature.parameters.len(),
        ));
        return None;
    }
    Some(index)
}

fn unknown_named_argument_diagnostic(
    name: &str,
    span: Span,
    signature: &CallableSignatureFact,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(format!("unknown named argument `{name}`"))
        .with_code("compiler::unknown_named_argument")
        .with_span(span)
        .with_label(span, "argument name does not match any parameter");
    let candidates = signature
        .parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<BTreeSet<_>>();
    if !candidates.is_empty() {
        diagnostic = diagnostic.with_label(
            span,
            format!(
                "available parameters: {}",
                candidates.into_iter().collect::<Vec<_>>().join(", ")
            ),
        );
    }
    diagnostic
}

fn reject_named_positional_arguments(
    arguments: &[CallSourceArgumentFact],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for argument in arguments {
        if let Some(name) = argument.name.as_deref() {
            diagnostics.push(
                Diagnostic::error(format!("unknown named argument `{name}`"))
                    .with_code("compiler::unknown_named_argument")
                    .with_span(argument.span)
                    .with_label(argument.span, "argument name does not match any parameter"),
            );
        }
    }
}

fn positional_after_named_diagnostic(span: Span) -> Diagnostic {
    Diagnostic::error("positional argument after named argument")
        .with_code("compiler::positional_after_named_argument")
        .with_span(span)
        .with_label(
            span,
            "positional arguments must appear before named arguments",
        )
}

fn too_many_arguments_diagnostic(span: Span, expected: usize) -> Diagnostic {
    Diagnostic::error("too many arguments")
        .with_code("compiler::too_many_arguments")
        .with_span(span)
        .with_label(
            span,
            format!("call accepts {expected} positional argument(s)"),
        )
}

fn duplicate_argument_diagnostic(name: &str, previous_span: Span, span: Span) -> Diagnostic {
    Diagnostic::error(format!("duplicate argument for parameter `{name}`"))
        .with_code("compiler::duplicate_argument")
        .with_span(span)
        .with_label(previous_span, "previous argument is here")
        .with_label(span, "duplicate argument is here")
}

fn missing_argument_diagnostic(parameter: &CallableParameterFact, call_span: Span) -> Diagnostic {
    let diagnostic = Diagnostic::error(format!("missing required argument `{}`", parameter.name))
        .with_code("compiler::missing_required_argument")
        .with_span(call_span)
        .with_label(call_span, "call does not provide this required parameter");
    if let Some(span) = parameter.declaration_span {
        diagnostic.with_label(span, "required parameter is declared here")
    } else {
        diagnostic
    }
}

fn async_call_requires_await_diagnostic(call_span: Span) -> Diagnostic {
    Diagnostic::error("async call requires `.await`")
        .with_code("analysis::async_call_requires_await")
        .with_span(call_span)
        .with_label(
            call_span,
            "append `.await` to suspend until this call completes",
        )
}
