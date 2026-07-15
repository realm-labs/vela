use vela_common::PrimitiveTag;
use vela_hir::body::{HirAssignOp, HirBinaryOp, HirBody, HirExprKind, HirUnaryOp};
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirLocalId, HirNodeId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::EnumVariantFieldsHint;

use crate::hints::{schema_declaration_from_hint_in_module, type_fact_from_hint_in_module};
use crate::logical_records::LogicalRecordFieldTargetFact;
use crate::registry::{
    RegistryFieldTargetFact, RegistryIndexCapabilityFact, RegistryTypeTargetFact,
};
use crate::type_fact::TypeFact;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptTypeTargetFact {
    pub declaration: HirDeclId,
    pub variant: Option<String>,
}

impl ScriptTypeTargetFact {
    #[must_use]
    pub const fn declaration(declaration: HirDeclId) -> Self {
        Self {
            declaration,
            variant: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallTargetFact {
    Declaration(HirDeclId),
    Variant {
        enum_declaration: HirDeclId,
        variant: String,
    },
    RegistryVariant {
        owner: String,
        variant: String,
    },
    ScriptMethod {
        method: HirNodeId,
    },
    Local(HirLocalId),
    Lambda(HirBodyId),
    RegistryFunction {
        path: String,
    },
    NativeFunction {
        path: String,
    },
    HostMethod {
        owner: String,
        name: String,
    },
    RegistryMethod {
        owner: String,
        name: String,
    },
    StdlibFunction {
        path: String,
    },
    StdlibMethod {
        name: String,
    },
    KnownReceiverMiss {
        receiver: TypeFact,
        script_type: Option<ScriptTypeTargetFact>,
        method: String,
    },
    Dynamic,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemberTargetFact {
    ScriptField {
        owner: HirDeclId,
        variant: Option<String>,
        name: String,
    },
    HostField(RegistryFieldTargetFact),
    LogicalRecordField(LogicalRecordFieldTargetFact),
    RegistryField {
        owner: String,
        name: String,
    },
    RegistryMethod {
        owner: String,
        name: String,
    },
    StdlibMethod {
        name: String,
    },
    TupleIndex(usize),
    Dynamic,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorTargetFact {
    Unary(HirUnaryOp),
    Binary(HirBinaryOp),
    Assignment(HirAssignOp),
    Dynamic,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstructorTargetFact {
    Declaration(HirDeclId),
    Variant {
        enum_declaration: HirDeclId,
        variant: String,
    },
    RegistryType {
        path: String,
    },
    RegistryVariant {
        owner: String,
        variant: String,
    },
    Dynamic,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostPathTargetFact {
    pub root: HirExprId,
    pub root_type: RegistryTypeTargetFact,
    pub segments: Vec<HostPathSegmentFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostPathSegmentFact {
    Field(RegistryFieldTargetFact),
    Index {
        expression: HirExprId,
        owner: RegistryTypeTargetFact,
        kind: HostPathIndexKindFact,
        capability: RegistryIndexCapabilityFact,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostPathIndexKindFact {
    Index,
    Key,
}

pub(super) fn direct_lambda_body(body: &HirBody, expression: HirExprId) -> Option<HirBodyId> {
    match &body.expressions.get(&expression)?.kind {
        HirExprKind::Lambda { body } => Some(*body),
        HirExprKind::Paren {
            expression: Some(inner),
        } => direct_lambda_body(body, *inner),
        _ => None,
    }
}

pub(super) struct SourceFieldFact {
    pub owner: HirDeclId,
    pub variant: Option<String>,
    pub name: String,
    pub fact: TypeFact,
    pub target: Option<ScriptTypeTargetFact>,
}

pub(super) fn source_field_fact(
    graph: &ModuleGraph,
    receiver: &ScriptTypeTargetFact,
    name: &str,
) -> Option<SourceFieldFact> {
    let declaration = graph.declaration(receiver.declaration)?;
    match declaration.kind {
        DeclarationKind::Struct if receiver.variant.is_none() => {
            let field = graph
                .struct_shape(declaration.id)?
                .fields
                .iter()
                .find(|field| field.name == name)?;
            let fact = field.type_hint.as_ref().map_or(TypeFact::Unknown, |hint| {
                type_fact_from_hint_in_module(graph, declaration.module, hint)
            });
            let target = field.type_hint.as_ref().and_then(|hint| {
                schema_declaration_from_hint_in_module(graph, declaration.module, hint)
                    .map(ScriptTypeTargetFact::declaration)
            });
            Some(SourceFieldFact {
                owner: declaration.id,
                variant: None,
                name: field.name.clone(),
                fact,
                target,
            })
        }
        DeclarationKind::Enum => {
            let variant_name = receiver.variant.as_ref()?;
            let variant = graph
                .enum_shape(declaration.id)?
                .variants
                .iter()
                .find(|variant| variant.name == *variant_name)?;
            let hint = match &variant.fields {
                EnumVariantFieldsHint::Unit => return None,
                EnumVariantFieldsHint::Tuple(fields) => {
                    fields.get(name.parse::<usize>().ok()?)?.type_hint.as_ref()
                }
                EnumVariantFieldsHint::Record(fields) => fields
                    .iter()
                    .find(|field| field.name == name)?
                    .type_hint
                    .as_ref(),
            };
            let fact = hint.map_or(TypeFact::Unknown, |hint| {
                type_fact_from_hint_in_module(graph, declaration.module, hint)
            });
            let target = hint.and_then(|hint| {
                schema_declaration_from_hint_in_module(graph, declaration.module, hint)
                    .map(ScriptTypeTargetFact::declaration)
            });
            Some(SourceFieldFact {
                owner: declaration.id,
                variant: Some(variant.name.clone()),
                name: name.to_owned(),
                fact,
                target,
            })
        }
        _ => None,
    }
}

/*
 * Host and registry owner names are schema keys, not a fallback for source
 * identities. Source fields are resolved exclusively through
 * `ScriptTypeTargetFact` above.
 */
pub(super) fn registry_field_owner(fact: &TypeFact) -> Option<String> {
    match fact {
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => Some(format!("{name}::{variant}")),
        _ => super::type_owner(fact).map(str::to_owned),
    }
}

pub(crate) fn registry_callable_owner(fact: &TypeFact) -> Option<&str> {
    match fact {
        TypeFact::Primitive(primitive) => Some(primitive_registry_owner(*primitive)),
        TypeFact::Range => Some("Range"),
        TypeFact::Array { .. } => Some("Array"),
        TypeFact::Map { .. } => Some("Map"),
        TypeFact::Set { .. } => Some("Set"),
        TypeFact::Iterator { .. } => Some("Iterator"),
        TypeFact::Option { .. } | TypeFact::OptionSome { .. } | TypeFact::OptionNone => {
            Some("Option")
        }
        TypeFact::Result { .. } | TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. } => {
            Some("Result")
        }
        TypeFact::Function { .. } => Some("Function"),
        TypeFact::Closure => Some("Closure"),
        TypeFact::Record { name }
        | TypeFact::Enum { name, .. }
        | TypeFact::Host { name }
        | TypeFact::Trait { name } => Some(name),
        TypeFact::LogicalRecord(record) => Some(record.runtime_name()),
        TypeFact::Unknown
        | TypeFact::Never
        | TypeFact::Any
        | TypeFact::Tuple { .. }
        | TypeFact::Module { .. }
        | TypeFact::Union(_) => None,
    }
}

const fn primitive_registry_owner(primitive: PrimitiveTag) -> &'static str {
    match primitive {
        PrimitiveTag::Unit => "Unit",
        PrimitiveTag::Bool => "Bool",
        PrimitiveTag::Char => "Char",
        PrimitiveTag::I8 => "I8",
        PrimitiveTag::I16 => "I16",
        PrimitiveTag::I32 => "I32",
        PrimitiveTag::I64 => "I64",
        PrimitiveTag::U8 => "U8",
        PrimitiveTag::U16 => "U16",
        PrimitiveTag::U32 => "U32",
        PrimitiveTag::U64 => "U64",
        PrimitiveTag::F32 => "F32",
        PrimitiveTag::F64 => "F64",
        PrimitiveTag::String => "String",
        PrimitiveTag::Bytes => "Bytes",
    }
}
