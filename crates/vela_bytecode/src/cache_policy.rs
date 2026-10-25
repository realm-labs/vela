use crate::linked::InstructionKind;
use crate::{CacheSiteId, CacheSiteKind, UnlinkedInstructionKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheSiteStorage {
    Sidecar,
    OptionalOperand,
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

macro_rules! impl_cache_sites {
    (
        $instruction:ty;

        cache {
            $(
                $policy_pattern:pat => ($kind:ident, $storage:ident);
                $access_pattern:pat => $read:expr, $write:expr;
            )+
        }
        none { $($none_pattern:pat_param)|+ $(|)? }
    ) => {
        impl CacheSiteInstruction for $instruction {
            fn cache_site_policy(&self) -> Option<CacheSitePolicy> {
                match self {
                    $(
                        $policy_pattern => Some(CacheSitePolicy {
                            kind: CacheSiteKind::$kind,
                            storage: CacheSiteStorage::$storage,
                        }),
                    )+
                    $($none_pattern)|+ => None,
                }
            }

            fn cache_site(&self) -> Option<CacheSiteId> {
                match self {
                    $($access_pattern => $read,)+
                    $($none_pattern)|+ => None,
                }
            }

            fn set_cache_site(&mut self, site: CacheSiteId) {
                match self {
                    $($access_pattern => ($write)(site),)+
                    $($none_pattern)|+ => {}
                }
            }
        }
    };
}

macro_rules! non_cache_instructions {
    () => {
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
            | Self::RunScalarBlock { .. }
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
            | Self::StoreState { .. }
            | Self::Return { .. }
    };
}

impl_cache_sites! {
    UnlinkedInstructionKind;
    cache {
        Self::LoadState { .. } => (StateRead, OptionalOperand);
        Self::LoadState { cache_site, .. } => *cache_site, |site| { *cache_site = Some(site) };
        Self::CallNative { .. } => (NativeCall, OptionalOperand);
        Self::CallNative { cache_site, .. } => *cache_site, |site| { *cache_site = Some(site) };
        Self::CallDynamicMethod { .. } | Self::CallMethodId { .. } => (MethodCall, Sidecar);
        Self::CallDynamicMethod { .. } | Self::CallMethodId { .. } => None, |_| {};
        Self::GetRecordSlot { .. } => (RecordFieldRead, Sidecar);
        Self::GetRecordSlot { .. } => None, |_| {};
        Self::SetRecordSlot { .. } => (RecordFieldWrite, Sidecar);
        Self::SetRecordSlot { .. } => None, |_| {};
    }
    none { non_cache_instructions!() }
}

impl_cache_sites! {
    InstructionKind;
    cache {
        Self::LoadState { .. } => (StateRead, OptionalOperand);
        Self::LoadState { cache_site, .. } => *cache_site, |site| { *cache_site = Some(site) };
        Self::CallNative { .. } => (NativeCall, OptionalOperand);
        Self::CallNative { cache_site, .. } => *cache_site, |site| { *cache_site = Some(site) };
        Self::CallDynamicMethod { .. } | Self::CallMethod { .. } => (MethodCall, OptionalOperand);
        Self::CallDynamicMethod { cache_site, .. } | Self::CallMethod { cache_site, .. } => *cache_site, |site| { *cache_site = Some(site) };
        Self::GetRecordSlot { .. } => (RecordFieldRead, OptionalOperand);
        Self::GetRecordSlot { cache_site, .. } => *cache_site, |site| { *cache_site = Some(site) };
        Self::SetRecordSlot { .. } => (RecordFieldWrite, OptionalOperand);
        Self::SetRecordSlot { cache_site, .. } => *cache_site, |site| { *cache_site = Some(site) };
    }
    none { non_cache_instructions!() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DebugNameId, FieldSlot, MethodDispatchHandle, NativeHandle, Register, StateSlot};

    use vela_def::{FunctionId, MethodId};

    #[test]
    fn every_cache_bearing_family_uses_the_shared_policy_surface() {
        use CacheSiteKind as K;
        use CacheSiteStorage as S;
        let rows = vec![
            (
                UnlinkedInstructionKind::LoadState {
                    dst: Register(0),
                    state: "g".into(),
                    slot: None,
                    cache_site: None,
                },
                InstructionKind::LoadState {
                    dst: Register(0),
                    slot: StateSlot::new(0),
                    debug_name: DebugNameId::new(0),
                    cache_site: None,
                },
                K::StateRead,
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
