use vela_common::PrimitiveTag;
use vela_hir::body::{HirBinaryOp, HirLiteral};
use vela_hir::ids::HirNodeId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::ImplMetadataKind;

use super::source_declaration_for_path;
use super::targets::{
    ScriptTypeTargetFact, registry_callable_owner, registry_field_owner, source_field_fact,
};
use crate::hints::type_fact_from_hint_in_module;
use crate::literals::{LiteralResult, ResolvedLiteralFact};
use crate::registry::{RegistryEffectFact, RegistryFacts};
use crate::stdlib::stdlib_method_fact;
use crate::type_fact::TypeFact;

pub(super) struct SourceMethodFact {
    pub(super) node: HirNodeId,
    pub(super) returns: TypeFact,
    pub(super) return_target: Option<ScriptTypeTargetFact>,
}

pub(super) fn source_method(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    name: &str,
) -> Option<SourceMethodFact> {
    let owner = type_owner(receiver)?;
    for declaration in graph.declarations_by_kind(DeclarationKind::Impl) {
        let Some(metadata) = graph.impl_metadata(declaration.id) else {
            continue;
        };
        let target = metadata.target_path.join("::");
        if target != owner && !owner.ends_with(&format!("::{target}")) {
            continue;
        }
        if let Some(method) = metadata.methods.iter().find(|method| method.name == name) {
            let returns = method
                .signature
                .return_type
                .as_ref()
                .map_or(TypeFact::Unknown, |hint| {
                    type_fact_from_hint_in_module(graph, declaration.module, hint)
                });
            let return_target = method.signature.return_type.as_ref().and_then(|hint| {
                crate::hints::schema_declaration_from_hint_in_module(
                    graph,
                    declaration.module,
                    hint,
                )
                .map(ScriptTypeTargetFact::declaration)
            });
            return Some(SourceMethodFact {
                node: method.node,
                returns,
                return_target,
            });
        }
        let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
            continue;
        };
        let Some(trait_declaration) = source_declaration_for_path(graph, trait_path) else {
            continue;
        };
        let Some(shape) = graph.trait_shape(trait_declaration.id) else {
            continue;
        };
        if let Some(method) = shape.methods.iter().find(|method| method.name == name)
            && let Some(node) = method.default_body_node
        {
            let returns = method
                .signature
                .return_type
                .as_ref()
                .map_or(TypeFact::Unknown, |hint| {
                    type_fact_from_hint_in_module(graph, trait_declaration.module, hint)
                });
            let return_target = method.signature.return_type.as_ref().and_then(|hint| {
                crate::hints::schema_declaration_from_hint_in_module(
                    graph,
                    trait_declaration.module,
                    hint,
                )
                .map(ScriptTypeTargetFact::declaration)
            });
            return Some(SourceMethodFact {
                node,
                returns,
                return_target,
            });
        }
    }
    None
}

pub(super) fn literal_fact(literal: &HirLiteral) -> TypeFact {
    match literal {
        HirLiteral::Bool(_) => TypeFact::BOOL,
        HirLiteral::Integer(value) => {
            TypeFact::primitive(crate::literals::integer_suffix_primitive(value.suffix))
        }
        HirLiteral::Float(value) => {
            TypeFact::primitive(crate::literals::float_suffix_primitive(value.suffix))
        }
        HirLiteral::Char(_) => TypeFact::CHAR,
        HirLiteral::String(_) | HirLiteral::Interpolated { .. } => TypeFact::STRING,
        HirLiteral::Bytes(_) => TypeFact::BYTES,
        HirLiteral::Invalid { .. } => TypeFact::Unknown,
    }
}

pub(super) fn resolved_literal_type(result: &LiteralResult) -> TypeFact {
    match result {
        Ok(ResolvedLiteralFact::Scalar(value)) => TypeFact::primitive(value.primitive()),
        Ok(ResolvedLiteralFact::Deferred(_)) => TypeFact::Any,
        Err(_) => TypeFact::Unknown,
    }
}

pub(super) fn try_payload_fact(fact: TypeFact) -> TypeFact {
    match fact {
        TypeFact::Option { some } | TypeFact::OptionSome { some } => *some,
        TypeFact::Result { ok, .. } | TypeFact::ResultOk { ok } => *ok,
        TypeFact::OptionNone | TypeFact::ResultErr { .. } | TypeFact::Never => TypeFact::Never,
        TypeFact::Union(facts) => TypeFact::union(facts.into_iter().map(try_payload_fact)),
        TypeFact::Unknown => TypeFact::Unknown,
        TypeFact::Any => TypeFact::Any,
        _ => TypeFact::Unknown,
    }
}

pub(super) fn binary_fact(
    op: Option<HirBinaryOp>,
    lhs: Option<TypeFact>,
    rhs: Option<TypeFact>,
) -> TypeFact {
    match op {
        Some(
            HirBinaryOp::Equal
            | HirBinaryOp::NotEqual
            | HirBinaryOp::IdentityEqual
            | HirBinaryOp::IdentityNotEqual
            | HirBinaryOp::Less
            | HirBinaryOp::LessEqual
            | HirBinaryOp::Greater
            | HirBinaryOp::GreaterEqual
            | HirBinaryOp::And
            | HirBinaryOp::Or,
        ) => TypeFact::BOOL,
        Some(HirBinaryOp::Range | HirBinaryOp::RangeInclusive) => TypeFact::Range,
        _ => lhs
            .filter(|fact| !matches!(fact, TypeFact::Unknown))
            .or(rhs)
            .unwrap_or(TypeFact::Unknown),
    }
}

pub(super) fn field_fact(
    graph: &ModuleGraph,
    source: Option<&ScriptTypeTargetFact>,
    receiver: &TypeFact,
    name: &str,
    schema: Option<&RegistryFacts>,
) -> TypeFact {
    if let TypeFact::Tuple { elements } = receiver
        && let Ok(index) = name.parse::<usize>()
    {
        return elements.get(index).cloned().unwrap_or(TypeFact::Unknown);
    }
    if let TypeFact::LogicalRecord(record) = receiver {
        return record
            .field(name)
            .map(|field| field.fact().clone())
            .unwrap_or(TypeFact::Unknown);
    }
    if let Some(field) = source.and_then(|source| source_field_fact(graph, source, name)) {
        return field.fact;
    }
    if let Some(method) = stdlib_method_fact(receiver, name, None) {
        return TypeFact::function(method.params, method.returns);
    }
    registry_field_owner(receiver)
        .as_deref()
        .and_then(|owner| {
            let schema = schema?;
            if matches!(receiver, TypeFact::Host { .. }) {
                schema
                    .host_field_fact(owner, name)
                    .or_else(|| schema.field_fact(owner, name))
                    .or_else(|| schema.method_fact(owner, name))
            } else {
                schema
                    .field_fact(owner, name)
                    .or_else(|| schema.method_fact(owner, name))
            }
        })
        .cloned()
        .unwrap_or({
            if matches!(receiver, TypeFact::Any) {
                TypeFact::Any
            } else {
                TypeFact::Unknown
            }
        })
}

pub(super) fn call_return_fact(callee: TypeFact) -> TypeFact {
    match callee {
        TypeFact::Function { returns, .. } => *returns,
        TypeFact::Any => TypeFact::Any,
        _ => TypeFact::Unknown,
    }
}

pub(super) fn index_fact(receiver: &TypeFact, schema: Option<&RegistryFacts>) -> TypeFact {
    match receiver {
        TypeFact::Array { element }
        | TypeFact::ArrayView { element }
        | TypeFact::ArrayMut { element, .. }
        | TypeFact::Set { element }
        | TypeFact::SetView { element }
        | TypeFact::SetMut { element, .. } => (**element).clone(),
        TypeFact::Map { value, .. }
        | TypeFact::MapView { value, .. }
        | TypeFact::MapMut { value, .. } => (**value).clone(),
        TypeFact::Tuple { elements } => TypeFact::union(elements.clone()),
        TypeFact::Primitive(PrimitiveTag::String) => TypeFact::CHAR,
        TypeFact::Primitive(PrimitiveTag::Bytes) => TypeFact::U8,
        TypeFact::Any => TypeFact::Any,
        _ => type_owner(receiver)
            .and_then(|owner| schema?.index_capability_fact(owner))
            .map_or(TypeFact::Unknown, |capability| capability.value.clone()),
    }
}

pub(super) fn registry_method_fact<'a>(
    schema: &'a RegistryFacts,
    receiver: &TypeFact,
    method: &str,
) -> Option<&'a TypeFact> {
    if is_borrowed_collection(receiver) && stdlib_method_fact(receiver, method, None).is_none() {
        return None;
    }
    let owner = registry_callable_owner(receiver)?;
    match receiver {
        TypeFact::Trait { .. } => schema
            .trait_method_fact(owner, method)
            .or_else(|| schema.method_fact(owner, method)),
        _ => schema
            .method_fact(owner, method)
            .or_else(|| schema.trait_method_fact(owner, method)),
    }
}

pub(super) fn registry_method_effect<'a>(
    schema: &'a RegistryFacts,
    receiver: &TypeFact,
    method: &str,
) -> Option<&'a RegistryEffectFact> {
    if is_borrowed_collection(receiver) && stdlib_method_fact(receiver, method, None).is_none() {
        return None;
    }
    let owner = registry_callable_owner(receiver)?;
    match receiver {
        TypeFact::Trait { .. } => schema
            .trait_method_effect_fact(owner, method)
            .or_else(|| schema.method_effect_fact(owner, method)),
        _ => schema
            .method_effect_fact(owner, method)
            .or_else(|| schema.trait_method_effect_fact(owner, method)),
    }
}

fn is_borrowed_collection(fact: &TypeFact) -> bool {
    matches!(
        fact,
        TypeFact::ArrayView { .. }
            | TypeFact::ArrayMut { .. }
            | TypeFact::MapView { .. }
            | TypeFact::MapMut { .. }
            | TypeFact::SetView { .. }
            | TypeFact::SetMut { .. }
    )
}

pub(super) fn schema_knows_owner(schema: &RegistryFacts, owner: &str) -> bool {
    schema.type_fact(owner).is_some()
        || schema.trait_fact(owner).is_some()
        || !schema.fields_for_owner(owner).is_empty()
        || !schema.variants_for_owner(owner).is_empty()
        || schema.methods().any(|method| method.owner == owner)
        || schema.trait_methods().any(|method| method.owner == owner)
}

pub(super) fn type_owner(fact: &TypeFact) -> Option<&str> {
    match fact {
        TypeFact::Record { name }
        | TypeFact::Enum { name, .. }
        | TypeFact::Host { name }
        | TypeFact::Trait { name } => Some(name),
        TypeFact::LogicalRecord(record) => Some(record.runtime_name()),
        _ => None,
    }
}
