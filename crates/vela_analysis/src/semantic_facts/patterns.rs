use vela_hir::body::{HirBody, HirPatternKind};
use vela_hir::ids::{HirLocalId, HirPatternId};
use vela_hir::module_graph::ModuleGraph;

use crate::registry::RegistryFacts;
use crate::type_fact::TypeFact;

use super::targets::{ScriptTypeTargetFact, source_field_fact};
use super::{ConstructorTargetFact, constructor_target};

#[derive(Clone, Debug)]
pub(super) struct PatternLocalFact {
    pub(super) local: HirLocalId,
    pub(super) fact: TypeFact,
    pub(super) script_type: Option<ScriptTypeTargetFact>,
}

#[derive(Clone, Debug)]
struct PatternValueFact {
    fact: TypeFact,
    script_type: Option<ScriptTypeTargetFact>,
}

pub(super) fn pattern_local_facts(
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    body: &HirBody,
    pattern: HirPatternId,
    fact: &TypeFact,
    script_type: Option<&ScriptTypeTargetFact>,
) -> Vec<PatternLocalFact> {
    let Some(pattern_data) = body.patterns.get(&pattern) else {
        return Vec::new();
    };
    match &pattern_data.kind {
        HirPatternKind::Binding { local } => local
            .iter()
            .map(|local| PatternLocalFact {
                local: *local,
                fact: fact.clone(),
                script_type: script_type.cloned(),
            })
            .collect(),
        HirPatternKind::TupleVariant { fields, .. } => {
            let target = pattern_constructor_target(graph, schema, body, pattern);
            let tuple_elements = match fact {
                TypeFact::Tuple { elements } => Some(elements.as_slice()),
                _ => None,
            };
            fields
                .iter()
                .enumerate()
                .flat_map(|(index, pattern)| {
                    let field = target
                        .as_ref()
                        .and_then(|target| {
                            variant_field_fact(graph, schema, target, &index.to_string())
                        })
                        .or_else(|| {
                            tuple_elements.and_then(|elements| {
                                elements.get(index).cloned().map(|fact| PatternValueFact {
                                    fact,
                                    script_type: None,
                                })
                            })
                        })
                        .unwrap_or_else(unknown_value_fact);
                    pattern_local_facts(
                        graph,
                        schema,
                        body,
                        *pattern,
                        &field.fact,
                        field.script_type.as_ref(),
                    )
                })
                .collect()
        }
        HirPatternKind::RecordVariant { fields, .. } => {
            let target = pattern_constructor_target(graph, schema, body, pattern);
            fields
                .iter()
                .filter_map(|field| field.pattern.map(|pattern| (field, pattern)))
                .flat_map(|(field, pattern)| {
                    let value = target
                        .as_ref()
                        .and_then(|target| variant_field_fact(graph, schema, target, &field.name))
                        .unwrap_or_else(unknown_value_fact);
                    pattern_local_facts(
                        graph,
                        schema,
                        body,
                        pattern,
                        &value.fact,
                        value.script_type.as_ref(),
                    )
                })
                .collect()
        }
        HirPatternKind::Path { .. }
        | HirPatternKind::Wildcard
        | HirPatternKind::Literal(_)
        | HirPatternKind::Missing => Vec::new(),
    }
}

pub(super) fn pattern_constructor_target(
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    body: &HirBody,
    pattern: HirPatternId,
) -> Option<ConstructorTargetFact> {
    let pattern = body.patterns.get(&pattern)?;
    let path = match &pattern.kind {
        HirPatternKind::TupleVariant { path, .. }
        | HirPatternKind::RecordVariant { path, .. }
        | HirPatternKind::Path { path } => path.and_then(|path| body.paths.get(&path)),
        HirPatternKind::Binding { .. }
        | HirPatternKind::Wildcard
        | HirPatternKind::Literal(_)
        | HirPatternKind::Missing => None,
    }?;
    let resolution = graph
        .bindings_for_body(body.id)
        .and_then(|bindings| bindings.pattern_constructor_resolution(&path.path));
    Some(constructor_target(graph, schema, &path.path, resolution))
}

fn variant_field_fact(
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    target: &ConstructorTargetFact,
    field: &str,
) -> Option<PatternValueFact> {
    match target {
        ConstructorTargetFact::Variant {
            enum_declaration,
            variant,
        } => source_field_fact(
            graph,
            &ScriptTypeTargetFact {
                declaration: *enum_declaration,
                variant: Some(variant.clone()),
            },
            field,
        )
        .map(|field| PatternValueFact {
            fact: field.fact,
            script_type: field.target,
        }),
        ConstructorTargetFact::RegistryVariant { owner, variant } => schema
            .and_then(|schema| schema.field_fact(&format!("{owner}::{variant}"), field))
            .cloned()
            .map(|fact| PatternValueFact {
                fact,
                script_type: None,
            }),
        ConstructorTargetFact::Declaration(_)
        | ConstructorTargetFact::RegistryType { .. }
        | ConstructorTargetFact::Dynamic
        | ConstructorTargetFact::Unresolved => None,
    }
}

fn unknown_value_fact() -> PatternValueFact {
    PatternValueFact {
        fact: TypeFact::Unknown,
        script_type: None,
    }
}
