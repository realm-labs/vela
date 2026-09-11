use super::ConstructorTargetFact;
use crate::registry::RegistryFacts;
use vela_hir::{
    binding::{BindingResolution, ConstructorResolution},
    body::HirBody,
    module_graph::{Declaration, DeclarationKind, ModuleGraph, Visibility},
};

pub(super) fn constructor_target(
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    path: &[String],
    resolution: Option<ConstructorResolution>,
) -> ConstructorTargetFact {
    if path.is_empty() {
        return ConstructorTargetFact::Unresolved;
    }
    if let Some(ConstructorResolution::Declaration(declaration)) = resolution {
        let Some(metadata) = graph.declaration(declaration) else {
            return ConstructorTargetFact::Unresolved;
        };
        return match metadata.kind {
            DeclarationKind::Struct => ConstructorTargetFact::Declaration(declaration),
            DeclarationKind::Enum if path.len() > 1 => ConstructorTargetFact::Variant {
                enum_declaration: declaration,
                variant: path.last().cloned().expect("non-empty constructor path"),
            },
            DeclarationKind::Enum => ConstructorTargetFact::Declaration(declaration),
            DeclarationKind::Const
            | DeclarationKind::State
            | DeclarationKind::Function
            | DeclarationKind::Trait
            | DeclarationKind::Impl => ConstructorTargetFact::Unresolved,
        };
    }
    let Some(ConstructorResolution::Dynamic(dynamic_path)) = resolution else {
        return ConstructorTargetFact::Unresolved;
    };
    if dynamic_path.len() > 1 {
        let (variant, owner_path) = dynamic_path
            .split_last()
            .expect("non-empty dynamic constructor path");
        let owner = owner_path.join("::");
        if let Some(target) =
            schema.and_then(|schema| schema.variant_for_owner_or_unique_short_name(&owner, variant))
        {
            return ConstructorTargetFact::RegistryVariant {
                owner: target.owner,
                variant: target.name,
            };
        }
    }
    let qualified = dynamic_path.join("::");
    if schema.is_some_and(|schema| {
        schema.type_fact(&qualified).is_some()
            || dynamic_path
                .last()
                .is_some_and(|name| schema.type_fact(name).is_some())
    }) {
        return ConstructorTargetFact::RegistryType { path: qualified };
    }
    ConstructorTargetFact::Dynamic
}

pub(super) fn unit_variant_constructor_target(
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    body: &HirBody,
    path: &[String],
    resolution: Option<&BindingResolution>,
) -> Option<ConstructorTargetFact> {
    let (variant, owner_path) = path.split_last()?;
    if owner_path.is_empty() {
        return None;
    }
    let declaration = match resolution {
        Some(BindingResolution::Declaration(declaration)) => graph.declaration(*declaration),
        _ => source_enum_for_path(graph, body, owner_path),
    };
    if let Some(declaration) = declaration
        && declaration.kind == DeclarationKind::Enum
    {
        return Some(ConstructorTargetFact::Variant {
            enum_declaration: declaration.id,
            variant: variant.clone(),
        });
    }
    schema
        .and_then(|schema| {
            schema.variant_for_owner_or_unique_short_name(&owner_path.join("::"), variant)
        })
        .map(|target| ConstructorTargetFact::RegistryVariant {
            owner: target.owner,
            variant: target.name,
        })
}

// A globally unique short name is insufficient evidence of a visible owner.
pub(super) fn source_enum_for_path<'a>(
    graph: &'a ModuleGraph,
    body: &HirBody,
    path: &[String],
) -> Option<&'a Declaration> {
    let module = graph
        .declaration(graph.bindings_for_body(body.id)?.declaration)?
        .module;
    graph
        .resolve_visible_declaration_path(module, path, DeclarationKind::Enum)
        .filter(|declaration| {
            declaration.module == module || declaration.visibility == Visibility::Public
        })
}
