use std::collections::BTreeMap;

use vela_analysis::{
    facts::AnalysisFacts, registry::RegistryFacts, stdlib::stdlib_method_fact, type_fact::TypeFact,
};
use vela_hir::{
    binding::{BindingMap, BindingResolution},
    ids::HirLocalId,
    module_graph::{Declaration, DeclarationKind, ModuleGraph},
    type_hint::HirTypeHint,
};

use crate::TextRange;

use super::{
    SemanticTokenClassification, SemanticTokenModifiers, SemanticTokenType, next_non_whitespace,
    span_for_range,
};

pub(super) struct MemberUseContext<'a> {
    pub(super) graph: &'a ModuleGraph,
    pub(super) bindings: &'a BindingMap,
    pub(super) facts: &'a AnalysisFacts,
    pub(super) schema: &'a RegistryFacts,
    pub(super) text: &'a str,
    pub(super) member_receivers: &'a BTreeMap<(usize, usize), TextRange>,
    pub(super) inferred_local_facts: &'a BTreeMap<HirLocalId, TypeFact>,
}

pub(super) fn classify(
    context: &MemberUseContext<'_>,
    name: &str,
    range: TextRange,
) -> Option<SemanticTokenClassification> {
    let receiver_range = *context.member_receivers.get(&(range.start, range.end))?;
    let receiver_span = span_for_range(
        context
            .graph
            .declaration(context.bindings.declaration)?
            .span
            .source,
        receiver_range,
    )?;
    let receiver = context
        .bindings
        .resolution_at_span(receiver_span)
        .and_then(|resolution| {
            type_fact_for_resolution(
                resolution,
                context.bindings,
                context.facts,
                context.schema,
                context.inferred_local_facts,
            )
        })?;
    let is_call = next_non_whitespace(context.text, range.end) == Some('(');

    if is_call
        && let Some(classification) =
            method_use_classification(context.graph, context.schema, &receiver, name)
    {
        return Some(classification);
    }

    field_use_classification(context.graph, context.schema, &receiver, name).or_else(|| {
        is_call
            .then(|| stdlib_method_fact(&receiver, name, None))
            .flatten()
            .map(|_| {
                SemanticTokenClassification::new(
                    SemanticTokenType::Method,
                    SemanticTokenModifiers::BUILTIN,
                )
            })
    })
}

fn method_use_classification(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    receiver: &TypeFact,
    name: &str,
) -> Option<SemanticTokenClassification> {
    if let Some(modifiers) = schema_method_modifiers(schema, receiver, name) {
        return Some(SemanticTokenClassification::new(
            SemanticTokenType::Method,
            modifiers,
        ));
    }
    if stdlib_method_fact(receiver, name, None).is_some() {
        return Some(SemanticTokenClassification::new(
            SemanticTokenType::Method,
            SemanticTokenModifiers::BUILTIN,
        ));
    }
    script_method_exists(graph, receiver, name).then(|| {
        SemanticTokenClassification::new(SemanticTokenType::Method, SemanticTokenModifiers::SOURCE)
    })
}

fn field_use_classification(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    receiver: &TypeFact,
    name: &str,
) -> Option<SemanticTokenClassification> {
    if schema_field_exists(schema, receiver, name) {
        return Some(SemanticTokenClassification::new(
            SemanticTokenType::Property,
            schema_host_modifier(receiver),
        ));
    }
    script_field_exists(graph, receiver, name).then(|| {
        SemanticTokenClassification::new(
            SemanticTokenType::Property,
            SemanticTokenModifiers::SOURCE,
        )
    })
}

fn type_fact_for_resolution(
    resolution: &BindingResolution,
    bindings: &BindingMap,
    facts: &AnalysisFacts,
    schema: &RegistryFacts,
    inferred_local_facts: &BTreeMap<HirLocalId, TypeFact>,
) -> Option<TypeFact> {
    match resolution {
        BindingResolution::Local(local) => {
            let binding = bindings.local(*local)?;
            facts
                .local(*local)
                .cloned()
                .filter(|fact| !matches!(fact, TypeFact::Unknown))
                .or_else(|| inferred_local_facts.get(local).cloned())
                .or_else(|| schema_fact_for_local_hint(binding.type_hint.as_ref(), schema))
        }
        BindingResolution::Declaration(declaration) => facts.declaration(*declaration).cloned(),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn schema_fact_for_local_hint(
    hint: Option<&HirTypeHint>,
    schema: &RegistryFacts,
) -> Option<TypeFact> {
    let hint = hint?;
    if hint.args.is_empty() {
        let qualified = hint.path.join("::");
        schema
            .type_fact(&qualified)
            .or_else(|| hint.path.last().and_then(|name| schema.type_fact(name)))
            .or_else(|| schema.trait_fact(&qualified))
            .or_else(|| hint.path.last().and_then(|name| schema.trait_fact(name)))
            .cloned()
    } else {
        None
    }
}

fn schema_method_modifiers(
    schema: &RegistryFacts,
    receiver: &TypeFact,
    method: &str,
) -> Option<SemanticTokenModifiers> {
    owner_names(receiver).iter().find_map(|owner| {
        if schema.method_fact(owner, method).is_some() {
            Some(schema_host_modifier(receiver))
        } else {
            schema
                .trait_method_fact(owner, method)
                .map(|_| SemanticTokenModifiers::HOST.union(SemanticTokenModifiers::SCHEMA))
        }
    })
}

fn schema_field_exists(schema: &RegistryFacts, receiver: &TypeFact, field: &str) -> bool {
    owner_names(receiver)
        .iter()
        .any(|owner| schema.field_fact(owner, field).is_some())
}

fn script_method_exists(graph: &ModuleGraph, receiver: &TypeFact, method: &str) -> bool {
    let owner_names = owner_names(receiver);
    graph
        .declarations()
        .any(|declaration| match declaration.kind {
            DeclarationKind::Impl => {
                let Some(metadata) = graph.impl_metadata(declaration.id) else {
                    return false;
                };
                let targets = impl_target_names(graph, declaration, &metadata.target_path);
                targets.iter().any(|target| owner_names.contains(target))
                    && metadata.methods.iter().any(|entry| entry.name == method)
            }
            DeclarationKind::Trait => {
                owner_names
                    .iter()
                    .any(|owner| declaration_name_matches(graph, declaration, owner))
                    && graph
                        .trait_shape(declaration.id)
                        .is_some_and(|shape| shape.methods.iter().any(|entry| entry.name == method))
            }
            DeclarationKind::Const
            | DeclarationKind::Enum
            | DeclarationKind::Function
            | DeclarationKind::Global
            | DeclarationKind::Struct => false,
        })
}

fn script_field_exists(graph: &ModuleGraph, receiver: &TypeFact, field: &str) -> bool {
    let owner_names = owner_names(receiver);
    graph.declarations().any(|declaration| {
        if !matches!(declaration.kind, DeclarationKind::Struct) {
            return false;
        }
        owner_names
            .iter()
            .any(|owner| declaration_name_matches(graph, declaration, owner))
            && graph
                .struct_shape(declaration.id)
                .is_some_and(|shape| shape.fields.iter().any(|entry| entry.name == field))
    })
}

fn owner_names(receiver: &TypeFact) -> Vec<String> {
    let Some(owner) = receiver_owner_name(receiver) else {
        return Vec::new();
    };
    let mut names = vec![owner.clone()];
    if let Some(short) = owner.rsplit("::").next()
        && short != owner
    {
        names.push(short.to_owned());
    }
    names
}

fn receiver_owner_name(receiver: &TypeFact) -> Option<String> {
    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } | TypeFact::Trait { name } => {
            Some(name.clone())
        }
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => Some(format!("{name}::{variant}")),
        TypeFact::Enum {
            name,
            variant: None,
        } => Some(name.clone()),
        _ => None,
    }
}

fn host_modifier(receiver: &TypeFact) -> SemanticTokenModifiers {
    if matches!(receiver, TypeFact::Host { .. }) {
        SemanticTokenModifiers::HOST
    } else {
        SemanticTokenModifiers::NONE
    }
}

fn schema_host_modifier(receiver: &TypeFact) -> SemanticTokenModifiers {
    host_modifier(receiver).union(SemanticTokenModifiers::SCHEMA)
}

fn impl_target_names(
    graph: &ModuleGraph,
    declaration: &Declaration,
    target_path: &[String],
) -> Vec<String> {
    let raw = target_path.join("::");
    let mut names = vec![raw.clone()];
    if target_path.len() == 1
        && let Some(module_path) = graph.module_path(declaration.module)
    {
        let qualified = module_path
            .segments()
            .iter()
            .chain(target_path.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join("::");
        if qualified != raw {
            names.push(qualified);
        }
    }
    names
}

fn declaration_name_matches(graph: &ModuleGraph, declaration: &Declaration, owner: &str) -> bool {
    declaration.name == owner || qualified_declaration_name(graph, declaration) == owner
}

fn qualified_declaration_name(graph: &ModuleGraph, declaration: &Declaration) -> String {
    graph
        .module_path(declaration.module)
        .map(|path| {
            path.segments()
                .iter()
                .chain(std::iter::once(&declaration.name))
                .cloned()
                .collect::<Vec<_>>()
                .join("::")
        })
        .unwrap_or_else(|| declaration.name.clone())
}
