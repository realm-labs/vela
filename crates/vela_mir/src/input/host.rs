use vela_common::HostMethodId;
use vela_def::{FieldId, MethodId};
use vela_hir::ids::HirExprId;

use crate::{CompileFieldAccess, CompileMethodAccess, HostTypeTarget, MirTypeContract};

use super::CompileSignature;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostFieldTarget {
    pub owner: HostTypeTarget,
    pub semantic: FieldId,
    pub runtime: FieldId,
    /// Immutable policy snapshot; MIR and its backends do not query a registry.
    pub access: CompileFieldAccess,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostMethodTarget {
    pub owner: HostTypeTarget,
    pub semantic: MethodId,
    pub runtime: HostMethodId,
    pub signature: CompileSignature,
    /// Immutable policy snapshot; MIR and its backends do not query a registry.
    pub access: CompileMethodAccess,
    pub scoped_borrow_return: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileHostPathSegment {
    Field(HostFieldTarget),
    ConstantIndex {
        value: u32,
        capability: CompileHostIndexCapability,
    },
    ConstantKey {
        value: String,
        capability: CompileHostIndexCapability,
    },
    DynamicIndex {
        expression: HirExprId,
        capability: CompileHostIndexCapability,
    },
    DynamicKey {
        expression: HirExprId,
        capability: CompileHostIndexCapability,
    },
    VariantField(HostFieldTarget),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileHostIndexCapability {
    pub readable: bool,
    pub writable: bool,
    pub mutable: bool,
    pub removable: bool,
    pub key: Option<MirTypeContract>,
    pub value: Option<MirTypeContract>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileHostPathTarget {
    pub root: HirExprId,
    pub root_type: HostTypeTarget,
    pub segments: Vec<CompileHostPathSegment>,
}
