use crate::linked::InstructionKind;
use crate::{CacheSiteId, CacheSiteKind, UnlinkedInstructionKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheSiteStorage {
    Sidecar,
    OptionalOperand,
    RequiredOperand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheSitePolicy {
    pub kind: CacheSiteKind,
    pub storage: CacheSiteStorage,
}

pub trait CacheSiteInstruction {
    fn cache_site_policy(&self) -> Option<CacheSitePolicy>;
    fn cache_site(&self) -> Option<CacheSiteId>;
    fn set_cache_site(&mut self, site: CacheSiteId);
}

macro_rules! policy {
    ($kind:ident, $storage:ident) => {
        Some(CacheSitePolicy {
            kind: CacheSiteKind::$kind,
            storage: CacheSiteStorage::$storage,
        })
    };
}

impl CacheSiteInstruction for UnlinkedInstructionKind {
    fn cache_site_policy(&self) -> Option<CacheSitePolicy> {
        match self {
            Self::LoadGlobal { .. } => policy!(GlobalRead, OptionalOperand),
            Self::CallNative { .. } => policy!(NativeCall, OptionalOperand),
            Self::CallDynamicMethod { .. } | Self::CallMethodId { .. } => {
                policy!(MethodCall, Sidecar)
            }
            Self::GetRecordSlot { .. } => policy!(RecordFieldRead, Sidecar),
            Self::SetRecordSlot { .. } => policy!(RecordFieldWrite, Sidecar),
            Self::HostRead { .. } => policy!(HostPathRead, RequiredOperand),
            Self::HostWrite { .. } => policy!(HostPathWrite, RequiredOperand),
            Self::HostMutate { .. } => policy!(HostPathMutate, RequiredOperand),
            Self::HostRemove { .. } => policy!(HostPathRemove, RequiredOperand),
            Self::HostCall { .. } => policy!(HostPathCall, RequiredOperand),
            Self::ChargeExecutionUnits { .. }
            | Self::LoadConst { .. }
            | Self::Move { .. }
            | Self::Not { .. }
            | Self::Truthy { .. }
            | Self::Negate { .. }
            | Self::Add { .. }
            | Self::Sub { .. }
            | Self::Mul { .. }
            | Self::Div { .. }
            | Self::Rem { .. }
            | Self::Equal { .. }
            | Self::NotEqual { .. }
            | Self::IdentityEqual { .. }
            | Self::IdentityNotEqual { .. }
            | Self::Less { .. }
            | Self::LessEqual { .. }
            | Self::Greater { .. }
            | Self::GreaterEqual { .. }
            | Self::I64Add { .. }
            | Self::I64Sub { .. }
            | Self::I64Mul { .. }
            | Self::I64Rem { .. }
            | Self::I64AddImm { .. }
            | Self::I64SubImm { .. }
            | Self::I64MulImm { .. }
            | Self::I64RemImm { .. }
            | Self::I64CmpImm { .. }
            | Self::I64CmpImmJumpIfFalse { .. }
            | Self::BinaryIntLiteral { .. }
            | Self::BinaryFloatLiteral { .. }
            | Self::GuardType { .. }
            | Self::JumpIfFalse { .. }
            | Self::JumpIfNotMissing { .. }
            | Self::Jump { .. }
            | Self::CallFunction { .. }
            | Self::MakeClosure { .. }
            | Self::CallClosure { .. }
            | Self::TryPropagate { .. }
            | Self::MakeArray { .. }
            | Self::MakeTuple { .. }
            | Self::MakeSetFromArray { .. }
            | Self::FormatString { .. }
            | Self::MakeMap { .. }
            | Self::MakeRange { .. }
            | Self::MakeRecord { .. }
            | Self::MakeEnum { .. }
            | Self::GetRecordField { .. }
            | Self::SetRecordField { .. }
            | Self::GetEnumField { .. }
            | Self::GetEnumSlot { .. }
            | Self::TupleArityEqual { .. }
            | Self::GuardTupleArity { .. }
            | Self::GetTupleField { .. }
            | Self::GetIndex { .. }
            | Self::GetStringKeyIndex { .. }
            | Self::SetIndex { .. }
            | Self::SetStringKeyIndex { .. }
            | Self::IterInit { .. }
            | Self::IterNext { .. }
            | Self::RangeNext { .. }
            | Self::I64RangeNext { .. }
            | Self::EnumTagEqual { .. }
            | Self::Return { .. } => None,
        }
    }

    fn cache_site(&self) -> Option<CacheSiteId> {
        match self {
            Self::LoadGlobal { cache_site, .. } | Self::CallNative { cache_site, .. } => {
                *cache_site
            }
            Self::HostRead { cache_site, .. }
            | Self::HostWrite { cache_site, .. }
            | Self::HostMutate { cache_site, .. }
            | Self::HostRemove { cache_site, .. }
            | Self::HostCall { cache_site, .. } => Some(*cache_site),
            _ => None,
        }
    }

    fn set_cache_site(&mut self, site: CacheSiteId) {
        match self {
            Self::LoadGlobal { cache_site, .. } | Self::CallNative { cache_site, .. } => {
                *cache_site = Some(site);
            }
            Self::HostRead { cache_site, .. }
            | Self::HostWrite { cache_site, .. }
            | Self::HostMutate { cache_site, .. }
            | Self::HostRemove { cache_site, .. }
            | Self::HostCall { cache_site, .. } => *cache_site = site,
            _ => {}
        }
    }
}

impl CacheSiteInstruction for InstructionKind {
    fn cache_site_policy(&self) -> Option<CacheSitePolicy> {
        match self {
            Self::LoadGlobal { .. } => policy!(GlobalRead, OptionalOperand),
            Self::CallNative { .. } => policy!(NativeCall, OptionalOperand),
            Self::CallDynamicMethod { .. } | Self::CallMethod { .. } => {
                policy!(MethodCall, OptionalOperand)
            }
            Self::GetRecordSlot { .. } => policy!(RecordFieldRead, OptionalOperand),
            Self::SetRecordSlot { .. } => policy!(RecordFieldWrite, OptionalOperand),
            Self::HostRead { .. } => policy!(HostPathRead, RequiredOperand),
            Self::HostWrite { .. } => policy!(HostPathWrite, RequiredOperand),
            Self::HostMutate { .. } => policy!(HostPathMutate, RequiredOperand),
            Self::HostRemove { .. } => policy!(HostPathRemove, RequiredOperand),
            Self::HostCall { .. } => policy!(HostPathCall, RequiredOperand),
            Self::ChargeExecutionUnits { .. }
            | Self::LoadConst { .. }
            | Self::Move { .. }
            | Self::Not { .. }
            | Self::Truthy { .. }
            | Self::Negate { .. }
            | Self::Add { .. }
            | Self::Sub { .. }
            | Self::Mul { .. }
            | Self::Div { .. }
            | Self::Rem { .. }
            | Self::Equal { .. }
            | Self::NotEqual { .. }
            | Self::IdentityEqual { .. }
            | Self::IdentityNotEqual { .. }
            | Self::Less { .. }
            | Self::LessEqual { .. }
            | Self::Greater { .. }
            | Self::GreaterEqual { .. }
            | Self::I64Add { .. }
            | Self::I64Sub { .. }
            | Self::I64Mul { .. }
            | Self::I64Rem { .. }
            | Self::I64AddImm { .. }
            | Self::I64SubImm { .. }
            | Self::I64MulImm { .. }
            | Self::I64RemImm { .. }
            | Self::I64CmpImm { .. }
            | Self::I64CmpImmJumpIfFalse { .. }
            | Self::BinaryIntLiteral { .. }
            | Self::BinaryFloatLiteral { .. }
            | Self::GuardType { .. }
            | Self::JumpIfFalse { .. }
            | Self::JumpIfNotMissing { .. }
            | Self::Jump { .. }
            | Self::CallFunction { .. }
            | Self::MakeClosure { .. }
            | Self::CallClosure { .. }
            | Self::TryPropagate { .. }
            | Self::MakeArray { .. }
            | Self::MakeTuple { .. }
            | Self::MakeSetFromArray { .. }
            | Self::FormatString { .. }
            | Self::MakeMap { .. }
            | Self::MakeRange { .. }
            | Self::MakeRecord { .. }
            | Self::MakeEnum { .. }
            | Self::GetRecordField { .. }
            | Self::SetRecordField { .. }
            | Self::GetEnumField { .. }
            | Self::GetEnumSlot { .. }
            | Self::TupleArityEqual { .. }
            | Self::GuardTupleArity { .. }
            | Self::GetTupleField { .. }
            | Self::GetIndex { .. }
            | Self::GetStringKeyIndex { .. }
            | Self::SetIndex { .. }
            | Self::SetStringKeyIndex { .. }
            | Self::IterInit { .. }
            | Self::IterNext { .. }
            | Self::RangeNext { .. }
            | Self::I64RangeNext { .. }
            | Self::EnumTagEqual { .. }
            | Self::Return { .. } => None,
        }
    }

    fn cache_site(&self) -> Option<CacheSiteId> {
        match self {
            Self::LoadGlobal { cache_site, .. }
            | Self::CallNative { cache_site, .. }
            | Self::CallDynamicMethod { cache_site, .. }
            | Self::CallMethod { cache_site, .. }
            | Self::GetRecordSlot { cache_site, .. }
            | Self::SetRecordSlot { cache_site, .. } => *cache_site,
            Self::HostRead { cache_site, .. }
            | Self::HostWrite { cache_site, .. }
            | Self::HostMutate { cache_site, .. }
            | Self::HostRemove { cache_site, .. }
            | Self::HostCall { cache_site, .. } => Some(*cache_site),
            _ => None,
        }
    }

    fn set_cache_site(&mut self, site: CacheSiteId) {
        match self {
            Self::LoadGlobal { cache_site, .. }
            | Self::CallNative { cache_site, .. }
            | Self::CallDynamicMethod { cache_site, .. }
            | Self::CallMethod { cache_site, .. }
            | Self::GetRecordSlot { cache_site, .. }
            | Self::SetRecordSlot { cache_site, .. } => *cache_site = Some(site),
            Self::HostRead { cache_site, .. }
            | Self::HostWrite { cache_site, .. }
            | Self::HostMutate { cache_site, .. }
            | Self::HostRemove { cache_site, .. }
            | Self::HostCall { cache_site, .. } => *cache_site = site,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DebugNameId, FieldSlot, GlobalSlot, HostTargetPlanId, MethodDispatchHandle, NativeHandle,
        Register,
    };
    use vela_common::HostMethodId;
    use vela_def::{FunctionId, MethodId};

    #[test]
    fn every_cache_bearing_family_uses_the_shared_policy_surface() {
        use CacheSiteKind as K;
        use CacheSiteStorage as S;
        let rows = vec![
            (
                UnlinkedInstructionKind::LoadGlobal {
                    dst: Register(0),
                    global: "g".into(),
                    slot: None,
                    cache_site: None,
                },
                InstructionKind::LoadGlobal {
                    dst: Register(0),
                    slot: GlobalSlot::new(0),
                    debug_name: DebugNameId::new(0),
                    cache_site: None,
                },
                K::GlobalRead,
                S::OptionalOperand,
                S::OptionalOperand,
            ),
            (
                UnlinkedInstructionKind::CallNative {
                    dst: None,
                    name: "n".into(),
                    native: FunctionId::new(1),
                    cache_site: None,
                    args: vec![],
                },
                InstructionKind::CallNative {
                    dst: None,
                    native: NativeHandle::new(0),
                    debug_name: DebugNameId::new(0),
                    cache_site: None,
                    args: vec![],
                },
                K::NativeCall,
                S::OptionalOperand,
                S::OptionalOperand,
            ),
            (
                UnlinkedInstructionKind::CallDynamicMethod {
                    dst: Register(0),
                    receiver: Register(1),
                    method: "m".into(),
                    args: vec![],
                },
                InstructionKind::CallDynamicMethod {
                    dst: Register(0),
                    receiver: Register(1),
                    method_name: DebugNameId::new(0),
                    cache_site: None,
                    args: vec![],
                },
                K::MethodCall,
                S::Sidecar,
                S::OptionalOperand,
            ),
            (
                UnlinkedInstructionKind::CallMethodId {
                    dst: Register(0),
                    receiver: Register(1),
                    method: "m".into(),
                    method_id: MethodId::new(1),
                    args: vec![],
                },
                InstructionKind::CallMethod {
                    dst: Register(0),
                    receiver: Register(1),
                    dispatch: MethodDispatchHandle::new(0),
                    debug_name: DebugNameId::new(0),
                    cache_site: None,
                    args: vec![],
                },
                K::MethodCall,
                S::Sidecar,
                S::OptionalOperand,
            ),
            (
                UnlinkedInstructionKind::GetRecordSlot {
                    dst: Register(0),
                    record: Register(1),
                    field: "f".into(),
                    slot: 0,
                },
                InstructionKind::GetRecordSlot {
                    dst: Register(0),
                    record: Register(1),
                    field: FieldSlot::new(0),
                    debug_name: DebugNameId::new(0),
                    cache_site: None,
                },
                K::RecordFieldRead,
                S::Sidecar,
                S::OptionalOperand,
            ),
            (
                UnlinkedInstructionKind::SetRecordSlot {
                    record: Register(0),
                    field: "f".into(),
                    slot: 0,
                    src: Register(1),
                },
                InstructionKind::SetRecordSlot {
                    record: Register(0),
                    field: FieldSlot::new(0),
                    debug_name: DebugNameId::new(0),
                    cache_site: None,
                    src: Register(1),
                },
                K::RecordFieldWrite,
                S::Sidecar,
                S::OptionalOperand,
            ),
            (
                UnlinkedInstructionKind::HostRead {
                    dst: Register(0),
                    root: Register(1),
                    target: HostTargetPlanId::new(0),
                    dynamic_args: vec![],
                    cache_site: CacheSiteId::new(0),
                },
                InstructionKind::HostRead {
                    dst: Register(0),
                    root: Register(1),
                    target: HostTargetPlanId::new(0),
                    dynamic_args: vec![],
                    cache_site: CacheSiteId::new(0),
                },
                K::HostPathRead,
                S::RequiredOperand,
                S::RequiredOperand,
            ),
            (
                UnlinkedInstructionKind::HostWrite {
                    root: Register(0),
                    target: HostTargetPlanId::new(0),
                    dynamic_args: vec![],
                    src: Register(1),
                    cache_site: CacheSiteId::new(0),
                },
                InstructionKind::HostWrite {
                    root: Register(0),
                    target: HostTargetPlanId::new(0),
                    dynamic_args: vec![],
                    src: Register(1),
                    cache_site: CacheSiteId::new(0),
                },
                K::HostPathWrite,
                S::RequiredOperand,
                S::RequiredOperand,
            ),
            (
                UnlinkedInstructionKind::HostMutate {
                    root: Register(0),
                    target: HostTargetPlanId::new(0),
                    dynamic_args: vec![],
                    op: vela_host::resolved::HostMutationOp::Add,
                    rhs: Register(1),
                    cache_site: CacheSiteId::new(0),
                },
                InstructionKind::HostMutate {
                    root: Register(0),
                    target: HostTargetPlanId::new(0),
                    dynamic_args: vec![],
                    op: vela_host::resolved::HostMutationOp::Add,
                    rhs: Register(1),
                    cache_site: CacheSiteId::new(0),
                },
                K::HostPathMutate,
                S::RequiredOperand,
                S::RequiredOperand,
            ),
            (
                UnlinkedInstructionKind::HostRemove {
                    root: Register(0),
                    target: HostTargetPlanId::new(0),
                    dynamic_args: vec![],
                    cache_site: CacheSiteId::new(0),
                },
                InstructionKind::HostRemove {
                    root: Register(0),
                    target: HostTargetPlanId::new(0),
                    dynamic_args: vec![],
                    cache_site: CacheSiteId::new(0),
                },
                K::HostPathRemove,
                S::RequiredOperand,
                S::RequiredOperand,
            ),
            (
                UnlinkedInstructionKind::HostCall {
                    dst: None,
                    root: Register(0),
                    target: HostTargetPlanId::new(0),
                    dynamic_args: vec![],
                    method: HostMethodId::new(1),
                    args: vec![],
                    cache_site: CacheSiteId::new(0),
                },
                InstructionKind::HostCall {
                    dst: None,
                    root: Register(0),
                    target: HostTargetPlanId::new(0),
                    dynamic_args: vec![],
                    method: MethodDispatchHandle::new(0),
                    debug_name: DebugNameId::new(0),
                    args: vec![],
                    cache_site: CacheSiteId::new(0),
                },
                K::HostPathCall,
                S::RequiredOperand,
                S::RequiredOperand,
            ),
        ];

        for (mut unlinked, mut linked, kind, unlinked_storage, linked_storage) in rows {
            assert_eq!(
                unlinked.cache_site_policy(),
                Some(CacheSitePolicy {
                    kind,
                    storage: unlinked_storage
                })
            );
            assert_eq!(
                linked.cache_site_policy(),
                Some(CacheSitePolicy {
                    kind,
                    storage: linked_storage
                })
            );
            unlinked.set_cache_site(CacheSiteId::new(7));
            linked.set_cache_site(CacheSiteId::new(7));
            assert_eq!(
                unlinked.cache_site(),
                (unlinked_storage != S::Sidecar).then_some(CacheSiteId::new(7))
            );
            assert_eq!(linked.cache_site(), Some(CacheSiteId::new(7)));
        }
    }
}
