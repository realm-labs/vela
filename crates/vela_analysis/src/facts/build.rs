use std::collections::{BTreeMap, BTreeSet};

use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirLocalId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};

use super::AnalysisFacts;
use crate::hints::{
    declaration_schema_fact, schema_declaration_from_hint_in_module, type_fact_from_hint_in_module,
};
use crate::literals::{LiteralFacts, LiteralPrimitiveContext};
use crate::registry::RegistryFacts;
use crate::semantic_facts::{HirSemanticFacts, ScriptTypeTargetFact};
use crate::type_fact::TypeFact;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ExecutableReceiverSeed<'a> {
    pub(crate) local: HirLocalId,
    pub(crate) fact: &'a TypeFact,
    pub(crate) script_type: Option<&'a ScriptTypeTargetFact>,
}

impl AnalysisFacts {
    #[must_use]
    pub fn from_module_graph(graph: &ModuleGraph) -> Self {
        Self::from_module_graph_with_schema(graph, None)
    }

    #[must_use]
    pub fn from_module_graph_and_schema(graph: &ModuleGraph, schema: &RegistryFacts) -> Self {
        Self::from_module_graph_with_schema(graph, Some(schema))
    }

    fn from_module_graph_with_schema(graph: &ModuleGraph, schema: Option<&RegistryFacts>) -> Self {
        Self::build(graph, schema, None, None, None)
    }

    pub(crate) fn from_executable_scope(
        graph: &ModuleGraph,
        schema: Option<&RegistryFacts>,
        bodies: &BTreeSet<HirBodyId>,
        receiver: Option<ExecutableReceiverSeed<'_>>,
        literal_contexts: &BTreeMap<HirExprId, LiteralPrimitiveContext>,
    ) -> Self {
        Self::build(
            graph,
            schema,
            Some(bodies),
            receiver,
            Some(literal_contexts),
        )
    }

    fn build(
        graph: &ModuleGraph,
        schema: Option<&RegistryFacts>,
        bodies: Option<&BTreeSet<HirBodyId>>,
        receiver: Option<ExecutableReceiverSeed<'_>>,
        literal_contexts: Option<&BTreeMap<HirExprId, LiteralPrimitiveContext>>,
    ) -> Self {
        let mut facts = Self::default();

        for declaration in graph.declarations() {
            if let Some(fact) = declaration_fact(graph, declaration.id) {
                facts.declarations.insert(declaration.id, fact);
            }
        }

        let mut binding_roots = graph
            .bodies()
            .filter(|body| bodies.is_none_or(|bodies| bodies.contains(&body.id)))
            .filter_map(|body| graph.bindings_for_body(body.id))
            .collect::<Vec<_>>();
        binding_roots.sort_by_key(|bindings| bindings.body());
        binding_roots.dedup_by_key(|bindings| bindings.body());

        for bindings in &binding_roots {
            let Some(owner) = graph.declaration(bindings.declaration) else {
                continue;
            };
            for local in bindings.locals() {
                let Some(hint) = local.type_hint.as_ref() else {
                    continue;
                };
                if let Some(declaration) =
                    schema_declaration_from_hint_in_module(graph, owner.module, hint)
                    && graph.declaration(declaration).is_some_and(|declaration| {
                        matches!(
                            declaration.kind,
                            DeclarationKind::Struct | DeclarationKind::Enum
                        )
                    })
                {
                    facts
                        .local_script_types
                        .insert(local.id, ScriptTypeTargetFact::declaration(declaration));
                }
                let fact = type_fact_from_hint_in_module(graph, owner.module, hint);
                let fact = if matches!(fact, TypeFact::Unknown) {
                    schema
                        .and_then(|schema| schema_fact_for_hint(schema, &hint.path))
                        .unwrap_or(fact)
                } else {
                    fact
                };
                facts.locals.insert(local.id, fact);
            }
        }

        if let Some(receiver) = receiver {
            facts.locals.insert(receiver.local, receiver.fact.clone());
            match receiver.script_type {
                Some(script_type) => {
                    facts
                        .local_script_types
                        .insert(receiver.local, script_type.clone());
                }
                None => {
                    facts.local_script_types.remove(&receiver.local);
                }
            }
        }

        for bindings in binding_roots {
            for (expression, resolution) in bindings.resolutions() {
                facts.resolutions.insert(expression, resolution.clone());
                if let Some(fact) = facts.fact_for_resolution(resolution).cloned() {
                    facts.expressions.insert(expression, fact);
                }
            }
        }

        facts.literals = match bodies {
            Some(bodies) => LiteralFacts::from_module_graph_for_bodies_with_contexts(
                graph,
                bodies,
                literal_contexts.expect("executable literal contexts are always supplied"),
            ),
            None => LiteralFacts::from_module_graph(graph),
        };
        facts.semantic = match bodies {
            Some(bodies) => {
                let mut semantic =
                    HirSemanticFacts::from_module_graph_for_bodies(graph, schema, &facts, bodies);
                semantic.totalize_executable_scope(graph, bodies);
                semantic
            }
            None => HirSemanticFacts::from_module_graph(graph, schema, &facts),
        };
        facts
    }
}

fn schema_fact_for_hint(schema: &RegistryFacts, path: &[String]) -> Option<TypeFact> {
    if path.is_empty() {
        return None;
    }
    let qualified = path.join("::");
    schema
        .type_fact(&qualified)
        .or_else(|| schema.trait_fact(&qualified))
        .or_else(|| path.last().and_then(|name| schema.type_fact(name)))
        .or_else(|| path.last().and_then(|name| schema.trait_fact(name)))
        .cloned()
}

fn declaration_fact(graph: &ModuleGraph, declaration: HirDeclId) -> Option<TypeFact> {
    let metadata = graph.declaration(declaration)?;
    if let Some(schema_fact) = declaration_schema_fact(graph, metadata) {
        return Some(schema_fact);
    }

    match metadata.kind {
        DeclarationKind::Const => graph
            .const_metadata(declaration)?
            .type_hint
            .as_ref()
            .map(|hint| type_fact_from_hint_in_module(graph, metadata.module, hint)),
        DeclarationKind::Global => graph
            .global_metadata(declaration)
            .map(|global| type_fact_from_hint_in_module(graph, metadata.module, &global.type_hint)),
        DeclarationKind::Function => graph.function_signature(declaration).map(|signature| {
            let params = signature
                .params
                .iter()
                .map(|param| {
                    param.type_hint.as_ref().map_or(TypeFact::Unknown, |hint| {
                        type_fact_from_hint_in_module(graph, metadata.module, hint)
                    })
                })
                .collect();
            let returns = signature
                .return_type
                .as_ref()
                .map_or(TypeFact::Unknown, |hint| {
                    type_fact_from_hint_in_module(graph, metadata.module, hint)
                });
            TypeFact::function(params, returns)
        }),
        DeclarationKind::Impl => None,
        DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait => None,
    }
}
