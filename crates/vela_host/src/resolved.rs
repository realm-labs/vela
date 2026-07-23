use vela_common::HostMethodId;

use crate::target::HostTargetPlan;

const INLINE_PREPARED_STEPS: usize = 4;

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
    pub offset: usize,
}

impl<'a> HostAccessSpec<'a> {
    #[must_use]
    pub const fn new(op: HostAccessOp, plan: &'a HostTargetPlan) -> Self {
        Self {
            op,
            plan,
            offset: 0,
        }
    }

    #[must_use]
    pub const fn at_offset(self, offset: usize) -> Self {
        Self { offset, ..self }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResolvedHostAccess {
    pub adapter_kind: ResolvedHostAccessKind,
    pub schema_epoch: HostSchemaEpoch,
    prepared_steps: [PreparedHostStep; INLINE_PREPARED_STEPS],
    prepared_step_count: u8,
    prepared_step_offset: u8,
}

impl ResolvedHostAccess {
    #[must_use]
    pub const fn new(adapter_kind: ResolvedHostAccessKind, schema_epoch: HostSchemaEpoch) -> Self {
        Self {
            adapter_kind,
            schema_epoch,
            prepared_steps: [PreparedHostStep::Field(0); INLINE_PREPARED_STEPS],
            prepared_step_count: 0,
            prepared_step_offset: 0,
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

    /// Prepends one schema-local field slot to a prepared nested traversal.
    ///
    /// Paths beyond the inline threshold retain the generic validated
    /// traversal instead of allocating inside the copyable inline-cache entry.
    #[must_use]
    pub fn prepend_prepared_field(mut self, slot: u32) -> Self {
        if matches!(self.adapter_kind, ResolvedHostAccessKind::GenericTarget) {
            return self;
        }
        let count = usize::from(self.prepared_step_count);
        if count == INLINE_PREPARED_STEPS {
            return Self::generic_target(self.schema_epoch);
        }
        self.prepared_steps.copy_within(0..count, 1);
        self.prepared_steps[0] = PreparedHostStep::Field(slot);
        self.prepared_step_count += 1;
        self
    }

    /// Prepends one adapter-local traversal step to a prepared nested access.
    #[doc(hidden)]
    #[must_use]
    pub fn prepend_prepared_adapter(mut self, slot: u32) -> Self {
        if matches!(self.adapter_kind, ResolvedHostAccessKind::GenericTarget) {
            return self;
        }
        let count = usize::from(self.prepared_step_count);
        if count == INLINE_PREPARED_STEPS {
            return Self::generic_target(self.schema_epoch);
        }
        self.prepared_steps.copy_within(0..count, 1);
        self.prepared_steps[0] = PreparedHostStep::AdapterLocal(slot);
        self.prepared_step_count += 1;
        self
    }

    #[must_use]
    pub fn prepared_field_slot(self, offset: usize) -> Option<u32> {
        match self.prepared_step(offset) {
            Some(PreparedHostStep::Field(slot)) => Some(slot),
            Some(PreparedHostStep::AdapterLocal(_)) | None => None,
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn prepared_step(self, offset: usize) -> Option<PreparedHostStep> {
        (offset < usize::from(self.prepared_step_count)).then(|| self.prepared_steps[offset])
    }

    #[doc(hidden)]
    #[must_use]
    pub fn next_prepared_step(mut self) -> Option<(PreparedHostStep, Self)> {
        let offset = usize::from(self.prepared_step_offset);
        let step = self.prepared_step(offset)?;
        self.prepared_step_offset += 1;
        Some((step, self))
    }

    #[must_use]
    pub fn next_prepared_field(self) -> Option<(u32, Self)> {
        match self.next_prepared_step()? {
            (PreparedHostStep::Field(slot), access) => Some((slot, access)),
            (PreparedHostStep::AdapterLocal(_), _) => None,
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PreparedHostStep {
    Field(u32),
    AdapterLocal(u32),
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
        assert_eq!(read.offset, 0);
        assert_ne!(read.op, call.op);
        assert_eq!(read.plan, call.plan);

        let nested = call.at_offset(1);
        assert_eq!(nested.plan, &plan);
        assert_eq!(nested.offset, 1);
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

    #[test]
    fn prepared_field_slots_are_inline_and_fall_back_when_too_deep() {
        let epoch = HostSchemaEpoch::new(42);
        let resolved = ResolvedHostAccess::direct_method(9, epoch)
            .prepend_prepared_field(3)
            .prepend_prepared_field(2)
            .prepend_prepared_field(1)
            .prepend_prepared_field(0);

        assert_eq!(resolved.prepared_field_slot(0), Some(0));
        assert_eq!(resolved.prepared_field_slot(3), Some(3));
        assert_eq!(resolved.prepared_field_slot(4), None);

        let (first, remaining) = resolved
            .next_prepared_field()
            .expect("prepared traversal should have a first field");
        let (second, _) = remaining
            .next_prepared_field()
            .expect("prepared traversal should advance independently");
        assert_eq!((first, second), (0, 1));

        let fallback = resolved.prepend_prepared_field(99);
        assert_eq!(fallback.adapter_kind, ResolvedHostAccessKind::GenericTarget);
        assert_eq!(fallback.prepared_field_slot(0), None);
        assert_eq!(fallback.schema_epoch, epoch);
    }

    #[test]
    fn prepared_steps_distinguish_fields_from_adapter_boundaries() {
        let epoch = HostSchemaEpoch::new(42);
        let resolved = ResolvedHostAccess::direct_field(9, epoch)
            .prepend_prepared_field(3)
            .prepend_prepared_adapter(0)
            .prepend_prepared_field(1);

        assert_eq!(resolved.prepared_field_slot(0), Some(1));
        assert_eq!(resolved.prepared_field_slot(1), None);
        assert_eq!(
            resolved.prepared_step(1),
            Some(PreparedHostStep::AdapterLocal(0))
        );
        let (first, remaining) = resolved.next_prepared_step().expect("field step");
        let (second, _) = remaining.next_prepared_step().expect("adapter step");
        assert_eq!(first, PreparedHostStep::Field(1));
        assert_eq!(second, PreparedHostStep::AdapterLocal(0));
    }
}
