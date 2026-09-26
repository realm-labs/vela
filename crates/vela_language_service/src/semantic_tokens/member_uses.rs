use std::collections::BTreeMap;

use vela_analysis::{
    facts::AnalysisFacts, registry::RegistryFacts, stdlib::stdlib_method_fact, type_fact::TypeFact,
};
use vela_hir::{
    binding::{BindingMap, BindingResolution},
    ids::{HirLocalId, ModuleId},
    module_graph::{Declaration, DeclarationKind, ModuleGraph},
    type_hint::{ImplMetadata, ImplMetadataKind},
};

use crate::{TextRange, expression_facts::ExpressionFacts};

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
    pub(super) receiver_facts: &'a ExpressionFacts,
    pub(super) inferred_local_facts: &'a BTreeMap<HirLocalId, TypeFact>,
}

pub(super) fn classify(
    context: &MemberUseContext<'_>,
    name: &str,
    range: TextRange,
) -> Option<SemanticTokenClassification> {
    let source = context
        .graph
        .declaration(context.bindings.declaration)?
        .span
        .source;
    let member_span = span_for_range(source, range)?;
    let field = context.graph.field_at_member_span(member_span)?;
    if field.name != name {
        return None;
    }
    let receiver = context
        .receiver_facts
        .get(field.receiver)
        .cloned()
        .filter(|fact| !matches!(fact, TypeFact::Unknown))
        .or_else(|| {
            context
                .bindings
                .resolution(field.receiver)
                .and_then(|resolution| {
                    type_fact_for_resolution(
                        resolution,
                        context.graph,
                        context.bindings,
                        context.facts,
                        context.schema,
                        context.inferred_local_facts,
                    )
                    .or_else(|| {
                        super::binding_scope::self_receiver(
                            context.graph,
                            context.bindings,
                            context.schema,
                            resolution,
                        )
                    })
                })
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
    if source_owner_exists(graph, receiver) {
        return script_method_exists(graph, receiver, name).then(|| {
            SemanticTokenClassification::new(
                SemanticTokenType::Method,
                SemanticTokenModifiers::SOURCE,
            )
        });
    }
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
    if source_owner_exists(graph, receiver) {
        return script_field_exists(graph, receiver, name).then(|| {
            SemanticTokenClassification::new(
                SemanticTokenType::Property,
                SemanticTokenModifiers::SOURCE,
            )
        });
    }
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
    graph: &ModuleGraph,
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
                .or_else(|| {
                    let module = graph.declaration(bindings.declaration)?.module;
                    let hint = binding.type_hint.as_ref()?;
                    let fact = vela_analysis::hints::type_fact_from_hint_with_schema(
                        graph,
                        module,
                        hint,
                        Some(schema),
                    );
                    (!matches!(fact, TypeFact::Unknown)).then_some(fact)
                })
        }
        BindingResolution::Declaration(declaration) => facts.declaration(*declaration).cloned(),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
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
    let owner_names = method_owner_names(receiver);
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
            | DeclarationKind::State
            | DeclarationKind::Struct => false,
        })
        || script_trait_default_method_exists(graph, receiver, method)
}

fn script_trait_default_method_exists(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    method: &str,
) -> bool {
    let owner_names = method_owner_names(receiver);
    graph.declarations().any(|declaration| {
        if !matches!(declaration.kind, DeclarationKind::Impl) {
            return false;
        }
        let Some(metadata) = graph.impl_metadata(declaration.id) else {
            return false;
        };
        let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
            return false;
        };
        let targets = impl_target_names(graph, declaration, &metadata.target_path);
        if !targets.iter().any(|target| owner_names.contains(target)) {
            return false;
        }
        if metadata.methods.iter().any(|entry| entry.name == method) {
            return true;
        }
        trait_default_method_exists(graph, declaration.module, metadata, trait_path, method)
    })
}

fn trait_default_method_exists(
    graph: &ModuleGraph,
    module: ModuleId,
    metadata: &ImplMetadata,
    trait_path: &[String],
    method: &str,
) -> bool {
    if metadata.methods.iter().any(|entry| entry.name == method) {
        return false;
    }
    let Some(trait_declaration) = trait_declaration_for_path(graph, module, trait_path) else {
        return false;
    };
    graph.trait_shape(trait_declaration).is_some_and(|shape| {
        shape
            .methods
            .iter()
            .any(|entry| entry.name == method && entry.has_default)
    })
}

fn trait_declaration_for_path(
    graph: &ModuleGraph,
    module: ModuleId,
    trait_path: &[String],
) -> Option<vela_hir::ids::HirDeclId> {
    let path = graph.expand_import_path(module, trait_path)?;
    graph
        .resolve_visible_declaration_path(module, &path, DeclarationKind::Trait)
        .map(|declaration| declaration.id)
}

fn script_field_exists(graph: &ModuleGraph, receiver: &TypeFact, field: &str) -> bool {
    if let TypeFact::Enum {
        name,
        variant: Some(variant),
    } = receiver
    {
        return graph
            .declarations()
            .filter(|declaration| {
                declaration.kind == DeclarationKind::Enum
                    && declaration_name_matches(graph, declaration, name)
            })
            .filter_map(|declaration| graph.enum_shape(declaration.id))
            .any(|shape| {
                shape.variants.iter().any(|entry| {
                    entry.name == *variant
                        && match &entry.fields {
                            vela_hir::type_hint::EnumVariantFieldsHint::Record(fields) => {
                                fields.iter().any(|entry| entry.name == field)
                            }
                            vela_hir::type_hint::EnumVariantFieldsHint::Tuple(fields) => {
                                fields.iter().any(|entry| entry.name == field)
                            }
                            vela_hir::type_hint::EnumVariantFieldsHint::Unit => false,
                        }
                })
            });
    }
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
    vec![owner]
}

fn method_owner_names(receiver: &TypeFact) -> Vec<String> {
    match receiver {
        TypeFact::Enum { name, .. } => vec![name.clone()],
        _ => owner_names(receiver),
    }
}

fn source_owner_exists(graph: &ModuleGraph, receiver: &TypeFact) -> bool {
    let name = match receiver {
        TypeFact::Record { name } | TypeFact::Trait { name } | TypeFact::Enum { name, .. } => name,
        _ => return false,
    };
    graph.declarations().any(|declaration| {
        matches!(
            declaration.kind,
            DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait
        ) && declaration_name_matches(graph, declaration, name)
    })
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
    let Some(path) = graph.expand_import_path(declaration.module, target_path) else {
        return Vec::new();
    };
    if let Some(current) = graph.module_key(declaration.module)
        && let Some(owner) = [
            DeclarationKind::Struct,
            DeclarationKind::Enum,
            DeclarationKind::Trait,
        ]
        .into_iter()
        .find_map(|kind| graph.declaration_by_type_path(&path, current, kind))
    {
        return vec![qualified_declaration_name(graph, owner)];
    }
    vec![path.join("::")]
}

fn declaration_name_matches(graph: &ModuleGraph, declaration: &Declaration, owner: &str) -> bool {
    qualified_declaration_name(graph, declaration) == owner
        || !owner.contains("::") && declaration.name == owner
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
