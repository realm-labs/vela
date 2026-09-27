use vela_hir::{
    body::HirBodyOwner,
    module_graph::{DeclarationKind, ModuleGraph},
    type_hint::HirTypeHint,
};

use crate::{
    facts::AnalysisFacts,
    hints::{
        declaration_schema_fact, schema_declaration_from_hint_in_module,
        type_fact_from_hint_with_schema,
    },
    registry::RegistryFacts,
    semantic_facts::ScriptTypeTargetFact,
};

pub(super) fn seed(graph: &ModuleGraph, schema: Option<&RegistryFacts>, facts: &mut AnalysisFacts) {
    for body in graph.bodies().filter(|body| {
        matches!(
            body.owner,
            HirBodyOwner::ImplMethod(_) | HirBodyOwner::TraitDefaultMethod(_)
        )
    }) {
        let Some(local) = body.self_binding else {
            continue;
        };
        let Some(bindings) = graph.bindings_for_body(body.id) else {
            continue;
        };
        let Some(binding) = bindings.local(local) else {
            continue;
        };
        // Explicit annotations and externally supplied contracts retain their
        // precedence, including deliberate Any/unknown receiver boundaries.
        if binding.type_hint.is_some() || facts.locals.contains_key(&local) {
            continue;
        }
        let Some(owner) = graph.declaration(bindings.declaration) else {
            continue;
        };
        let (fact, script_type) = match owner.kind {
            DeclarationKind::Trait => {
                let Some(fact) = declaration_schema_fact(graph, owner) else {
                    continue;
                };
                (fact, Some(ScriptTypeTargetFact::declaration(owner.id)))
            }
            DeclarationKind::Impl => {
                let Some(metadata) = graph.impl_metadata(owner.id) else {
                    continue;
                };
                let hint = HirTypeHint {
                    path: metadata.target_path.clone(),
                    args: Vec::new(),
                    span: binding.span,
                };
                let fact = type_fact_from_hint_with_schema(graph, owner.module, &hint, schema);
                let script_type =
                    schema_declaration_from_hint_in_module(graph, owner.module, &hint)
                        .map(ScriptTypeTargetFact::declaration);
                (fact, script_type)
            }
            _ => continue,
        };
        facts.locals.insert(local, fact);
        if let Some(script_type) = script_type {
            facts.local_script_types.insert(local, script_type);
        }
    }
}
