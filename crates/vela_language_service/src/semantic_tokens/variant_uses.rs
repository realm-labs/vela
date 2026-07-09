use std::collections::BTreeMap;

use vela_analysis::registry::RegistryFacts;
use vela_hir::{
    binding::{BindingMap, BindingResolution},
    ids::HirDeclId,
    module_graph::{DeclarationKind, ModuleGraph},
};

use crate::{TextRange, query_context::binding_resolution_for_source_range};

use super::{SemanticTokenClassification, SemanticTokenModifiers, SemanticTokenType};

pub(super) fn classification(
    graph: &ModuleGraph,
    bindings: &BindingMap,
    schema: &RegistryFacts,
    path_expressions: &BTreeMap<(usize, usize), Vec<String>>,
    pattern_paths: &BTreeMap<(usize, usize), Vec<String>>,
    range: TextRange,
) -> Option<SemanticTokenClassification> {
    let path = path_expressions
        .get(&(range.start, range.end))
        .or_else(|| pattern_paths.get(&(range.start, range.end)))?;

    if source_variant_owner(graph, bindings, path, range).is_some() {
        return Some(SemanticTokenClassification::new(
            SemanticTokenType::EnumMember,
            SemanticTokenModifiers::SOURCE,
        ));
    }

    schema_variant_exists(schema, path).then(|| {
        SemanticTokenClassification::new(
            SemanticTokenType::EnumMember,
            SemanticTokenModifiers::HOST.union(SemanticTokenModifiers::SCHEMA),
        )
    })
}

fn source_variant_owner(
    graph: &ModuleGraph,
    bindings: &BindingMap,
    path: &[String],
    range: TextRange,
) -> Option<HirDeclId> {
    let variant = path.last()?;
    if let Some(BindingResolution::Declaration(owner)) = bindings.pattern_resolution(path)
        && enum_variant_exists(graph, *owner, variant)
    {
        return Some(*owner);
    }

    match binding_resolution_for_source_range(graph, bindings, range)? {
        BindingResolution::Declaration(owner) if enum_variant_exists(graph, *owner, variant) => {
            Some(*owner)
        }
        BindingResolution::Declaration(_)
        | BindingResolution::Local(_)
        | BindingResolution::Import(_)
        | BindingResolution::QualifiedPath(_) => None,
    }
}

fn enum_variant_exists(graph: &ModuleGraph, owner: HirDeclId, variant: &str) -> bool {
    graph
        .declaration(owner)
        .is_some_and(|declaration| declaration.kind == DeclarationKind::Enum)
        && graph
            .enum_shape(owner)
            .is_some_and(|shape| shape.variants.iter().any(|entry| entry.name == variant))
}

fn schema_variant_exists(schema: &RegistryFacts, path: &[String]) -> bool {
    let Some((variant, owner_segments)) = path.split_last() else {
        return false;
    };
    if owner_segments.is_empty() {
        return false;
    }

    let owner = owner_segments.join("::");
    if schema.variant_fact(&owner, variant).is_some() {
        return true;
    }

    if owner.contains("::") {
        return false;
    }

    let mut matches = schema.variants().filter(|candidate| {
        candidate.name == *variant
            && candidate
                .owner
                .rsplit("::")
                .next()
                .is_some_and(|short| short == owner)
    });
    matches.next().is_some() && matches.next().is_none()
}
