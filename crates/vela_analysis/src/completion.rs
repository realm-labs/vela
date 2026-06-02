use std::collections::BTreeSet;

use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};

use crate::facts::AnalysisFacts;
use crate::registry::RegistryFacts;
use crate::stdlib::{stdlib_function_completion_facts, stdlib_method_facts};
use crate::type_fact::TypeFact;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionKind {
    Binding,
    Const,
    Field,
    Method,
    Module,
    Variant,
    Function,
    Type,
    Trait,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub fact: TypeFact,
}

impl CompletionItem {
    fn new(label: impl Into<String>, kind: CompletionKind, fact: TypeFact) -> Self {
        Self {
            label: label.into(),
            kind,
            fact,
        }
    }
}

pub fn member_completions(facts: &RegistryFacts, receiver: &TypeFact) -> Vec<CompletionItem> {
    let mut completions = match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } => {
            owner_member_completions(facts, name)
        }
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => owner_field_completions(facts, &format!("{name}::{variant}")),
        TypeFact::Enum {
            name,
            variant: None,
        } => variant_completions(facts, name),
        TypeFact::Trait { name } => trait_method_completions(facts, name),
        _ => Vec::new(),
    };
    completions.extend(stdlib_method_completions(receiver));
    completions
}

pub fn global_completions(facts: &RegistryFacts) -> Vec<CompletionItem> {
    let mut completions = type_completions(facts);
    completions.extend(function_completions(facts));
    completions
}

pub fn type_completions(facts: &RegistryFacts) -> Vec<CompletionItem> {
    let mut completions = facts
        .types()
        .map(|(name, fact)| CompletionItem::new(name, CompletionKind::Type, fact.clone()))
        .collect::<Vec<_>>();
    completions.extend(
        facts
            .traits()
            .map(|(name, fact)| CompletionItem::new(name, CompletionKind::Trait, fact.clone())),
    );
    completions
}

pub fn declaration_completions(graph: &ModuleGraph, facts: &AnalysisFacts) -> Vec<CompletionItem> {
    graph
        .declarations()
        .filter_map(|declaration| {
            let kind = completion_kind_for_declaration(declaration.kind)?;
            let fact = facts
                .declaration(declaration.id)
                .cloned()
                .unwrap_or(TypeFact::Unknown);
            Some(CompletionItem::new(
                qualified_declaration_label(graph, declaration.id),
                kind,
                fact,
            ))
        })
        .collect()
}

pub fn module_completions(graph: &ModuleGraph) -> Vec<CompletionItem> {
    module_completion_labels(graph)
        .into_iter()
        .map(|label| {
            CompletionItem::new(
                label.clone(),
                CompletionKind::Module,
                TypeFact::module(label),
            )
        })
        .collect()
}

pub fn local_completions(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    declaration: HirDeclId,
) -> Vec<CompletionItem> {
    let Some(bindings) = graph.bindings(declaration) else {
        return Vec::new();
    };
    bindings
        .locals()
        .map(|local| {
            let fact = facts.local(local.id).cloned().unwrap_or(TypeFact::Unknown);
            CompletionItem::new(local.name.clone(), CompletionKind::Binding, fact)
        })
        .collect()
}

fn module_completion_labels(graph: &ModuleGraph) -> BTreeSet<String> {
    let mut modules = BTreeSet::new();
    for declaration in graph.declarations() {
        let Some(module_path) = graph.module_path(declaration.module) else {
            continue;
        };
        let segments = module_path.segments();
        for len in 1..=segments.len() {
            modules.insert(segments[..len].join("::"));
        }
    }
    modules
}

fn completion_kind_for_declaration(kind: DeclarationKind) -> Option<CompletionKind> {
    match kind {
        DeclarationKind::Const => Some(CompletionKind::Const),
        DeclarationKind::Function => Some(CompletionKind::Function),
        DeclarationKind::Struct | DeclarationKind::Enum => Some(CompletionKind::Type),
        DeclarationKind::Trait => Some(CompletionKind::Trait),
        DeclarationKind::Impl => None,
    }
}

fn qualified_declaration_label(graph: &ModuleGraph, declaration: HirDeclId) -> String {
    let Some(declaration) = graph.declaration(declaration) else {
        return String::new();
    };
    let Some(module_path) = graph.module_path(declaration.module) else {
        return declaration.name.clone();
    };
    let module = module_path.join();
    if module.is_empty() {
        declaration.name.clone()
    } else {
        format!("{module}::{}", declaration.name)
    }
}

fn owner_member_completions(facts: &RegistryFacts, owner: &str) -> Vec<CompletionItem> {
    let mut completions = owner_field_completions(facts, owner);
    completions.extend(
        facts
            .methods()
            .filter(|method| method.owner == owner)
            .map(|method| CompletionItem::new(method.name, CompletionKind::Method, method.fact)),
    );
    completions
}

fn owner_field_completions(facts: &RegistryFacts, owner: &str) -> Vec<CompletionItem> {
    facts
        .fields()
        .filter(|field| field.owner == owner)
        .map(|field| CompletionItem::new(field.name, CompletionKind::Field, field.fact))
        .collect()
}

fn variant_completions(facts: &RegistryFacts, owner: &str) -> Vec<CompletionItem> {
    facts
        .variants()
        .filter(|variant| variant.owner == owner)
        .map(|variant| CompletionItem::new(variant.name, CompletionKind::Variant, variant.fact))
        .collect()
}

fn trait_method_completions(facts: &RegistryFacts, owner: &str) -> Vec<CompletionItem> {
    facts
        .trait_methods()
        .filter(|method| method.owner == owner)
        .map(|method| CompletionItem::new(method.name, CompletionKind::Method, method.fact))
        .collect()
}

fn function_completions(facts: &RegistryFacts) -> Vec<CompletionItem> {
    let mut completions = facts
        .functions()
        .map(|function| CompletionItem::new(function.name, CompletionKind::Function, function.fact))
        .collect::<Vec<_>>();
    completions.extend(stdlib_function_completions());
    completions
}

fn stdlib_method_completions(receiver: &TypeFact) -> Vec<CompletionItem> {
    stdlib_method_facts(receiver, None)
        .into_iter()
        .map(|fact| {
            CompletionItem::new(
                fact.method,
                CompletionKind::Method,
                TypeFact::function(fact.params, fact.returns),
            )
        })
        .collect()
}

fn stdlib_function_completions() -> Vec<CompletionItem> {
    stdlib_function_completion_facts()
        .into_iter()
        .map(|fact| {
            CompletionItem::new(
                fact.name,
                CompletionKind::Function,
                TypeFact::function(fact.params, fact.returns),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
