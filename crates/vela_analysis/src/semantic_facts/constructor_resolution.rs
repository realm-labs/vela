use super::{CallTargetFact, ConstructorTargetFact};
use crate::{registry::RegistryFacts, type_fact::TypeFact};
use vela_hir::{
    binding::{BindingResolution, ConstructorResolution},
    body::HirBody,
    module_graph::{DeclarationKind, ModuleGraph, Visibility},
};

pub(super) fn imported_constructor_target(
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    body: &HirBody,
    path: &[String],
    binding: Option<&BindingResolution>,
) -> Option<ConstructorTargetFact> {
    if path.len() > 1 && matches!(binding, Some(BindingResolution::Local(_))) {
        return Some(ConstructorTargetFact::Unresolved);
    }
    if !matches!(binding, Some(BindingResolution::Import(_))) {
        return None;
    }
    let module = graph
        .declaration(graph.bindings_for_body(body.id)?.declaration)?
        .module;
    let Some(expanded) = graph.expand_import_path(module, path) else {
        return Some(ConstructorTargetFact::Unresolved);
    };
    let source = [
        DeclarationKind::Struct,
        DeclarationKind::Enum,
        DeclarationKind::Trait,
        DeclarationKind::Function,
        DeclarationKind::Const,
        DeclarationKind::State,
    ]
    .into_iter()
    .find_map(|kind| graph.resolve_visible_declaration_path(module, &expanded, kind))
    .or_else(|| {
        let (_, owner) = expanded.split_last()?;
        [
            DeclarationKind::Enum,
            DeclarationKind::Struct,
            DeclarationKind::Trait,
            DeclarationKind::Function,
            DeclarationKind::Const,
            DeclarationKind::State,
        ]
        .into_iter()
        .find_map(|kind| graph.resolve_visible_declaration_path(module, owner, kind))
    });
    let resolution = if let Some(source) = source {
        if source.module != module && source.visibility != Visibility::Public {
            return Some(ConstructorTargetFact::Unresolved);
        }
        if source.kind != DeclarationKind::Enum
            && graph
                .resolve_visible_declaration_path(module, &expanded, DeclarationKind::Struct)
                .is_none()
        {
            return Some(ConstructorTargetFact::Unresolved);
        }
        ConstructorResolution::Declaration(source.id)
    } else {
        ConstructorResolution::Dynamic(expanded.clone())
    };
    Some(constructor_target(
        graph,
        schema,
        &expanded,
        Some(resolution),
    ))
}

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
    if matches!(resolution, Some(BindingResolution::Local(_))) {
        return None;
    }
    if path.len() < 2 {
        return None;
    }
    let module = graph
        .declaration(graph.bindings_for_body(body.id)?.declaration)?
        .module;
    let Some(path) = graph.expand_import_path(module, path) else {
        return Some(ConstructorTargetFact::Unresolved);
    };
    let (variant, owner_path) = path.split_last()?;
    if owner_path.is_empty() {
        return None;
    }
    let declaration = [
        DeclarationKind::Enum,
        DeclarationKind::Struct,
        DeclarationKind::Trait,
        DeclarationKind::Function,
        DeclarationKind::Const,
        DeclarationKind::State,
    ]
    .into_iter()
    .find_map(|kind| graph.resolve_visible_declaration_path(module, owner_path, kind));
    if let Some(declaration) = declaration {
        if declaration.kind != DeclarationKind::Enum
            || (declaration.module != module && declaration.visibility != Visibility::Public)
            || !graph
                .enum_shape(declaration.id)?
                .variants
                .iter()
                .any(|entry| entry.name == *variant)
        {
            return Some(ConstructorTargetFact::Unresolved);
        }
        return Some(ConstructorTargetFact::Variant {
            enum_declaration: declaration.id,
            variant: variant.clone(),
        });
    }
    schema
        .and_then(|schema| {
            schema
                .variant_for_owner_or_unique_short_name(&owner_path.join("::"), variant)
                .filter(|target| target.owner == owner_path.join("::"))
        })
        .map(|target| ConstructorTargetFact::RegistryVariant {
            owner: target.owner,
            variant: target.name,
        })
}

pub(super) fn imported_variant_call_target(
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    body: &HirBody,
    path: &[String],
) -> CallTargetFact {
    match unit_variant_constructor_target(graph, schema, body, path, None) {
        Some(ConstructorTargetFact::Variant {
            enum_declaration,
            variant,
        }) => CallTargetFact::Variant {
            enum_declaration,
            variant,
        },
        Some(ConstructorTargetFact::RegistryVariant { owner, variant }) => {
            CallTargetFact::RegistryVariant { owner, variant }
        }
        _ => CallTargetFact::Unresolved,
    }
}

pub(super) fn constructor_result_fact(
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    target: &ConstructorTargetFact,
) -> Option<TypeFact> {
    match target {
        ConstructorTargetFact::Variant {
            enum_declaration,
            variant,
        } => graph.declaration(*enum_declaration).map(|declaration| {
            if declaration.name == "Option" && variant == "None" {
                TypeFact::OptionNone
            } else {
                let name = graph
                    .qualified_declaration_name(*enum_declaration)
                    .unwrap_or_else(|| declaration.name.clone());
                TypeFact::enum_type(name, Some(variant.as_str()))
            }
        }),
        ConstructorTargetFact::RegistryVariant { owner, variant } => schema
            .and_then(|schema| schema.variant_for_owner_or_unique_short_name(owner, variant))
            .map(|target| target.fact),
        ConstructorTargetFact::RegistryType { path } => {
            schema.and_then(|schema| schema.type_fact(path)).cloned()
        }
        ConstructorTargetFact::Declaration(id) => {
            let declaration = graph.declaration(*id)?;
            let name = graph.qualified_declaration_name(*id)?;
            match declaration.kind {
                DeclarationKind::Struct => Some(TypeFact::record(name)),
                DeclarationKind::Enum => Some(TypeFact::enum_type(name, None::<String>)),
                _ => None,
            }
        }
        ConstructorTargetFact::Dynamic | ConstructorTargetFact::Unresolved => None,
    }
}
