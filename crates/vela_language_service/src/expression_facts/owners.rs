use vela_analysis::{expression::ExprFactScope, facts::AnalysisFacts, type_fact::TypeFact};
use vela_hir::{
    ids::HirDeclId,
    module_graph::{DeclarationKind, ModuleGraph},
};

pub(super) fn record_owner_names(receiver: &TypeFact) -> Vec<String> {
    let mut owners = Vec::new();
    collect_record_owner_names(receiver, &mut owners);
    owners
}

pub(super) fn trait_owner_names(receiver: &TypeFact) -> Vec<String> {
    let mut owners = Vec::new();
    collect_trait_owner_names(receiver, &mut owners);
    owners
}

fn collect_record_owner_names(receiver: &TypeFact, owners: &mut Vec<String>) {
    match receiver {
        TypeFact::Record { name } => {
            push_owner_name(owners, name);
            if let Some(short) = name.rsplit("::").next()
                && short != name
            {
                push_owner_name(owners, short);
            }
        }
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_record_owner_names(fact, owners);
            }
        }
        TypeFact::Unknown
        | TypeFact::Never
        | TypeFact::Any
        | TypeFact::Primitive(_)
        | TypeFact::Range
        | TypeFact::Array { .. }
        | TypeFact::Map { .. }
        | TypeFact::Set { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Function { .. }
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Trait { .. }
        | TypeFact::Module { .. } => {}
    }
}

fn collect_trait_owner_names(receiver: &TypeFact, owners: &mut Vec<String>) {
    match receiver {
        TypeFact::Trait { name } => {
            push_owner_name(owners, name);
            if let Some(short) = name.rsplit("::").next()
                && short != name
            {
                push_owner_name(owners, short);
            }
        }
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_trait_owner_names(fact, owners);
            }
        }
        TypeFact::Unknown
        | TypeFact::Never
        | TypeFact::Any
        | TypeFact::Primitive(_)
        | TypeFact::Range
        | TypeFact::Array { .. }
        | TypeFact::Map { .. }
        | TypeFact::Set { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Function { .. }
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Record { .. }
        | TypeFact::Module { .. } => {}
    }
}

fn push_owner_name(owners: &mut Vec<String>, name: &str) {
    if !owners.iter().any(|owner| owner == name) {
        owners.push(name.to_owned());
    }
}

pub(super) fn impl_target_matches(path: &[String], owner: &str) -> bool {
    path.last().is_some_and(|name| name == owner) || path.join("::") == owner
}

pub(super) fn trait_declaration_for_path(
    graph: &ModuleGraph,
    trait_path: &[String],
) -> Option<HirDeclId> {
    let owner = trait_path.join("::");
    graph
        .declarations()
        .find(|declaration| {
            declaration.kind == DeclarationKind::Trait
                && declaration_name_matches(graph, declaration.id, &owner)
        })
        .map(|declaration| declaration.id)
}

pub(super) fn declaration_name_matches(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    owner: &str,
) -> bool {
    let Some(declaration) = graph.declaration(declaration) else {
        return false;
    };
    declaration.name == owner || qualified_declaration_label(graph, declaration.id) == owner
}

fn qualified_declaration_label(graph: &ModuleGraph, declaration: HirDeclId) -> String {
    let Some(declaration) = graph.declaration(declaration) else {
        return String::new();
    };
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

pub(super) fn declaration_scope(graph: &ModuleGraph) -> ExprFactScope {
    let mut scope = ExprFactScope::new();
    let facts = AnalysisFacts::from_module_graph(graph);
    for (declaration_id, fact) in facts.declarations() {
        let Some(declaration) = graph.declaration(declaration_id) else {
            continue;
        };
        scope.insert_path([declaration.name.clone()], fact.clone());
        if let Some(module_path) = graph.module_path(declaration.module) {
            let mut path = module_path.segments().to_vec();
            path.push(declaration.name.clone());
            scope.insert_path(path, fact.clone());
        }
    }
    scope
}
