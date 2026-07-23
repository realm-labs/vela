use std::cell::Cell;

use vela_common::{HostMethodId, HostObjectId, HostTypeId, ScalarValue};
use vela_def::FieldId;
use vela_host::{
    error::{HostErrorKind, HostResult},
    object::ScriptHostObject,
    path::HostRef,
    resolved::{HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess},
    target::{HostTargetInstance, HostTargetPlan},
    value::HostValue,
};

struct CursorAdapter {
    resolved_offset: Cell<Option<usize>>,
    written_offset: Option<usize>,
}

impl ScriptHostObject for CursorAdapter {
    fn host_type_id(&self) -> HostTypeId {
        HostTypeId::new(1)
    }

    fn resolve_host_target(&self, spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        self.resolved_offset.set(Some(spec.offset));
        Ok(ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)))
    }

    fn read_resolved_host(
        &self,
        _access: ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        Ok(HostValue::Scalar(ScalarValue::I64(2)))
    }

    fn write_resolved_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        _value: HostValue,
    ) -> HostResult<()> {
        self.written_offset = Some(target.offset);
        Ok(())
    }
}

fn nested_leaf<'a>(plan: &'a HostTargetPlan) -> HostTargetInstance<'a> {
    HostTargetInstance::new(
        HostRef::new(HostTypeId::new(1), HostObjectId::new(2), 3),
        plan,
        &[],
    )
    .at_offset(1)
}

#[test]
fn default_mutation_preserves_the_nested_target_cursor() {
    let plan = HostTargetPlan::new(HostTypeId::new(1)).field(FieldId::new(4));
    let target = nested_leaf(&plan);
    let mut adapter = CursorAdapter {
        resolved_offset: Cell::new(None),
        written_offset: None,
    };

    adapter
        .mutate_resolved_host(
            ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)),
            target,
            HostMutationOp::Add,
            HostValue::Scalar(ScalarValue::I64(3)),
        )
        .expect("default mutation should resolve and write at the nested leaf");

    assert_eq!(adapter.resolved_offset.get(), Some(1));
    assert_eq!(adapter.written_offset, Some(1));
}

#[test]
fn default_method_errors_classify_a_nested_cursor_at_the_leaf() {
    let plan = HostTargetPlan::new(HostTypeId::new(1)).field(FieldId::new(4));
    let target = nested_leaf(&plan);
    let mut adapter = CursorAdapter {
        resolved_offset: Cell::new(None),
        written_offset: None,
    };

    let error = adapter
        .call_resolved_host(
            ResolvedHostAccess::generic_target(HostSchemaEpoch::new(0)),
            target,
            HostMethodId::new(5),
            &[],
        )
        .expect_err("the adapter does not implement methods");

    assert!(matches!(
        error.kind,
        HostErrorKind::UnsupportedMethod { method } if method == HostMethodId::new(5)
    ));
}
