use vela_analysis::completion::{
    CompletionItem as AnalysisCompletionItem, CompletionKind as AnalysisCompletionKind,
};
use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{FunctionSignature, HirTypeHint, ImplMetadataKind};

use crate::CompletionSymbol;
use crate::callable_context::query_type_fact_from_hint;
use crate::symbol_ref::{source_impl_method_symbol, source_member_symbol};

pub(super) fn source_member_completion_candidates(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    receiver: &TypeFact,
) -> Vec<(AnalysisCompletionItem, CompletionSymbol)> {
    source_field_completion_items(graph, schema, receiver)
        .into_iter()
        .chain(source_method_completion_items(graph, schema, receiver))
        .collect()
}

fn source_field_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    receiver: &TypeFact,
) -> Vec<(AnalysisCompletionItem, CompletionSymbol)> {
    let owner_names = record_owner_names(receiver);
    graph
        .declarations()
        .filter_map(|declaration| {
            if declaration.kind != DeclarationKind::Struct
                || !owner_names
                    .iter()
                    .any(|owner| declaration_name_matches(graph, declaration.id, owner))
            {
                return None;
            }
            let shape = graph.struct_shape(declaration.id)?;
            Some((declaration.id, shape))
        })
        .flat_map(|(declaration, shape)| {
            shape.fields.iter().filter_map(move |field| {
                Some((
                    AnalysisCompletionItem {
                        label: field.name.clone(),
                        kind: AnalysisCompletionKind::Field,
                        fact: type_fact_from_optional_hint(graph, schema, field.type_hint.as_ref()),
                    },
                    source_member_symbol(graph, declaration, &field.name)?,
                ))
            })
        })
        .collect()
}

fn source_method_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    receiver: &TypeFact,
) -> Vec<(AnalysisCompletionItem, CompletionSymbol)> {
    let mut items = source_impl_method_completion_items(graph, schema, receiver);
    items.extend(source_trait_receiver_method_completion_items(
        graph, schema, receiver,
    ));
    items.extend(source_trait_default_method_completion_items(
        graph, schema, receiver,
    ));
    items
}

fn source_impl_method_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    receiver: &TypeFact,
) -> Vec<(AnalysisCompletionItem, CompletionSymbol)> {
    let owner_names = record_owner_names(receiver);
    graph
        .declarations()
        .filter_map(|declaration| {
            if declaration.kind != DeclarationKind::Impl {
                return None;
            }
            let metadata = graph.impl_metadata(declaration.id)?;
            let matches_owner = owner_names
                .iter()
                .any(|owner| impl_target_matches(&metadata.target_path, owner));
            matches_owner.then_some((declaration.id, metadata))
        })
        .flat_map(|(declaration, metadata)| {
            metadata.methods.iter().filter_map(move |method| {
                Some((
                    AnalysisCompletionItem {
                        label: method.name.clone(),
                        kind: AnalysisCompletionKind::Method,
                        fact: function_fact_from_signature(graph, schema, &method.signature, true),
                    },
                    source_impl_method_symbol(graph, declaration, &method.name)?,
                ))
            })
        })
        .collect()
}

fn source_trait_receiver_method_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    receiver: &TypeFact,
) -> Vec<(AnalysisCompletionItem, CompletionSymbol)> {
    let owner_names = trait_owner_names(receiver);
    graph
        .declarations()
        .filter_map(|declaration| {
            if declaration.kind != DeclarationKind::Trait
                || !owner_names
                    .iter()
                    .any(|owner| declaration_name_matches(graph, declaration.id, owner))
            {
                return None;
            }
            let shape = graph.trait_shape(declaration.id)?;
            Some((declaration.id, shape))
        })
        .flat_map(|(declaration, shape)| {
            shape.methods.iter().filter_map(move |method| {
                Some((
                    AnalysisCompletionItem {
                        label: method.name.clone(),
                        kind: AnalysisCompletionKind::Method,
                        fact: function_fact_from_signature(graph, schema, &method.signature, true),
                    },
                    source_member_symbol(graph, declaration, &method.name)?,
                ))
            })
        })
        .collect()
}

fn source_trait_default_method_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    receiver: &TypeFact,
) -> Vec<(AnalysisCompletionItem, CompletionSymbol)> {
    let owner_names = record_owner_names(receiver);
    graph
        .declarations()
        .filter_map(|declaration| {
            if declaration.kind != DeclarationKind::Impl {
                return None;
            }
            let metadata = graph.impl_metadata(declaration.id)?;
            let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
                return None;
            };
            let matches_owner = owner_names
                .iter()
                .any(|owner| impl_target_matches(&metadata.target_path, owner));
            if !matches_owner {
                return None;
            }
            let trait_declaration = trait_declaration_for_path(graph, trait_path)?;
            Some((metadata, trait_declaration))
        })
        .flat_map(|(metadata, trait_declaration)| {
            let implemented = metadata
                .methods
                .iter()
                .map(|method| method.name.as_str())
                .collect::<Vec<_>>();
            graph
                .trait_shape(trait_declaration)
                .into_iter()
                .flat_map(move |shape| {
                    shape.methods.iter().filter_map({
                        let implemented = implemented.clone();
                        move |method| {
                            if !method.has_default
                                || implemented
                                    .iter()
                                    .any(|implemented| *implemented == method.name)
                            {
                                return None;
                            }
                            Some((
                                AnalysisCompletionItem {
                                    label: method.name.clone(),
                                    kind: AnalysisCompletionKind::Method,
                                    fact: function_fact_from_signature(
                                        graph,
                                        schema,
                                        &method.signature,
                                        true,
                                    ),
                                },
                                source_member_symbol(graph, trait_declaration, &method.name)?,
                            ))
                        }
                    })
                })
        })
        .collect()
}

fn function_fact_from_signature(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    signature: &FunctionSignature,
    skip_self: bool,
) -> TypeFact {
    let params = signature
        .params
        .iter()
        .filter(|param| !skip_self || param.name != "self")
        .map(|param| type_fact_from_optional_hint(graph, schema, param.type_hint.as_ref()))
        .collect();
    let returns = type_fact_from_optional_hint(graph, schema, signature.return_type.as_ref());
    TypeFact::function(params, returns)
}

fn type_fact_from_optional_hint(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    hint: Option<&HirTypeHint>,
) -> TypeFact {
    hint.map_or(TypeFact::Unknown, |hint| {
        query_type_fact_from_hint(graph, hint, schema)
    })
}

fn declaration_name_matches(
    graph: &ModuleGraph,
    declaration: vela_hir::ids::HirDeclId,
    owner: &str,
) -> bool {
    let Some(declaration) = graph.declaration(declaration) else {
        return false;
    };
    declaration.name == owner
        || graph.module_path(declaration.module).is_some_and(|module| {
            let qualified = module
                .segments()
                .iter()
                .chain(std::iter::once(&declaration.name))
                .cloned()
                .collect::<Vec<_>>()
                .join("::");
            qualified == owner
        })
}

fn trait_declaration_for_path(
    graph: &ModuleGraph,
    trait_path: &[String],
) -> Option<vela_hir::ids::HirDeclId> {
    let owner = trait_path.join("::");
    graph
        .declarations()
        .find(|declaration| {
            declaration.kind == DeclarationKind::Trait
                && declaration_name_matches(graph, declaration.id, &owner)
        })
        .map(|declaration| declaration.id)
}

fn impl_target_matches(path: &[String], owner: &str) -> bool {
    path.last().is_some_and(|name| name == owner) || path.join("::") == owner
}

fn record_owner_names(receiver: &TypeFact) -> Vec<String> {
    let mut owners = Vec::new();
    collect_record_owner_names(receiver, &mut owners);
    owners
}

fn collect_record_owner_names(receiver: &TypeFact, owners: &mut Vec<String>) {
    match receiver {
        TypeFact::Record { name } => push_owner_names(owners, name),
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_record_owner_names(fact, owners);
            }
        }
        _ => {}
    }
}

fn trait_owner_names(receiver: &TypeFact) -> Vec<String> {
    let mut owners = Vec::new();
    collect_trait_owner_names(receiver, &mut owners);
    owners
}

fn collect_trait_owner_names(receiver: &TypeFact, owners: &mut Vec<String>) {
    match receiver {
        TypeFact::Trait { name } => push_owner_names(owners, name),
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_trait_owner_names(fact, owners);
            }
        }
        _ => {}
    }
}

fn push_owner_names(owners: &mut Vec<String>, name: &str) {
    push_owner_name(owners, name);
    if let Some(short) = name.rsplit("::").next()
        && short != name
    {
        push_owner_name(owners, short);
    }
}

fn push_owner_name(owners: &mut Vec<String>, name: &str) {
    if !owners.iter().any(|owner| owner == name) {
        owners.push(name.to_owned());
    }
}
