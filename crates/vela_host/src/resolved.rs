use vela_common::HostMethodId;

use crate::target::HostTargetPlan;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HostSchemaEpoch(pub u64);

impl HostSchemaEpoch {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HostAccessOp {
    Read,
    Write,
    Mutate(HostMutationOp),
    Remove,
    Call(HostMethodId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HostMutationOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Push,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HostAccessSpec<'a> {
    pub op: HostAccessOp,
    pub plan: &'a HostTargetPlan,
}

impl<'a> HostAccessSpec<'a> {
    #[must_use]
    pub const fn new(op: HostAccessOp, plan: &'a HostTargetPlan) -> Self {
        Self { op, plan }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResolvedHostAccess {
    pub adapter_kind: ResolvedHostAccessKind,
    pub schema_epoch: HostSchemaEpoch,
}

impl ResolvedHostAccess {
    #[must_use]
    pub const fn new(adapter_kind: ResolvedHostAccessKind, schema_epoch: HostSchemaEpoch) -> Self {
        Self {
            adapter_kind,
            schema_epoch,
        }
    }

    #[must_use]
    pub const fn generic_target(schema_epoch: HostSchemaEpoch) -> Self {
        Self::new(ResolvedHostAccessKind::GenericTarget, schema_epoch)
    }

    #[must_use]
    pub const fn direct_field(slot: u32, schema_epoch: HostSchemaEpoch) -> Self {
        Self::new(ResolvedHostAccessKind::DirectField(slot), schema_epoch)
    }

    #[must_use]
    pub const fn direct_method(slot: u32, schema_epoch: HostSchemaEpoch) -> Self {
        Self::new(ResolvedHostAccessKind::DirectMethod(slot), schema_epoch)
    }

    #[must_use]
    pub const fn adapter_local(slot: u32, schema_epoch: HostSchemaEpoch) -> Self {
        Self::new(ResolvedHostAccessKind::AdapterLocal(slot), schema_epoch)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResolvedHostAccessKind {
    GenericTarget,
    DirectField(u32),
    DirectMethod(u32),
    AdapterLocal(u32),
}

#[cfg(test)]
mod tests {
    use vela_common::{HostMethodId, HostTypeId};
    use vela_def::FieldId;

    use super::*;
    use crate::target::HostTargetPlan;

    #[test]
    fn access_specs_keep_operation_and_shape_separate() {
        let plan = HostTargetPlan::new(HostTypeId::new(1)).field(FieldId::new(2));
        let read = HostAccessSpec::new(HostAccessOp::Read, &plan);
        let call = HostAccessSpec::new(HostAccessOp::Call(HostMethodId::new(9)), &plan);

        assert_eq!(read.op, HostAccessOp::Read);
        assert_eq!(read.plan, &plan);
        assert_ne!(read.op, call.op);
        assert_eq!(read.plan, call.plan);
    }

    #[test]
    fn resolved_access_records_kind_and_schema_epoch() {
        let epoch = HostSchemaEpoch::new(42);
        let resolved = ResolvedHostAccess::direct_field(7, epoch);

        assert_eq!(resolved.schema_epoch.get(), 42);
        assert_eq!(
            resolved.adapter_kind,
            ResolvedHostAccessKind::DirectField(7)
        );
    }
}
