use vela_common::{HostMethodId, Span};

use crate::{
    adapter::ScriptStateAdapter,
    error::{HostError, HostErrorKind, HostResult},
    path::{HostPath, HostRef},
    resolved::{HostAccessOp, HostAccessSpec, HostMutationOp, ResolvedHostAccess},
    target::{HostTargetInstance, HostTargetPlan},
    value::HostValue,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostObjectSnapshot {
    pub type_id: vela_common::HostTypeId,
    pub object_id: vela_common::HostObjectId,
    pub generation: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HostAccess;

impl HostAccess {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    pub fn read_diagnostic_path(
        &self,
        adapter: &(impl ScriptStateAdapter + ?Sized),
        path: &HostPath,
    ) -> HostResult<HostValue> {
        self.read_diagnostic_path_at(adapter, path, None)
    }

    pub fn read_diagnostic_path_at(
        &self,
        adapter: &(impl ScriptStateAdapter + ?Sized),
        path: &HostPath,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        let plan = HostTargetPlan::from(path);
        let target = HostTargetInstance::new(path.root, &plan, &[]);
        self.read(adapter, target, source_span)
    }

    pub fn read(
        &self,
        adapter: &(impl ScriptStateAdapter + ?Sized),
        target: HostTargetInstance<'_>,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        let access = adapter
            .resolve_host_access(HostAccessSpec::new(HostAccessOp::Read, target.plan))
            .map_err(|error| error.with_source_span_if_absent(source_span))?;
        self.read_resolved(adapter, access, target, source_span)
    }

    pub fn read_resolved(
        &self,
        adapter: &(impl ScriptStateAdapter + ?Sized),
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        adapter
            .read_host(access, target)
            .map_err(|error| error.with_source_span_if_absent(source_span))
    }

    pub fn write_diagnostic_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let plan = HostTargetPlan::from(&path);
        let target = HostTargetInstance::new(path.root, &plan, &[]);
        self.write(adapter, target, value, source_span)
    }

    pub fn write(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        target: HostTargetInstance<'_>,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let access = adapter
            .resolve_host_access(HostAccessSpec::new(HostAccessOp::Write, target.plan))
            .map_err(|error| error.with_source_span_if_absent(source_span))?;
        self.write_resolved(adapter, access, target, value, source_span)
    }

    pub fn write_resolved(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        adapter
            .write_host(access, target, value)
            .map_err(|error| error.with_source_span_if_absent(source_span))
    }

    pub fn add_diagnostic_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.mutate_diagnostic_path(adapter, path, value, source_span, HostMutationOp::Add)
    }

    pub fn sub_diagnostic_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.mutate_diagnostic_path(adapter, path, value, source_span, HostMutationOp::Sub)
    }

    pub fn mul_diagnostic_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.mutate_diagnostic_path(adapter, path, value, source_span, HostMutationOp::Mul)
    }

    pub fn div_diagnostic_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.mutate_diagnostic_path(adapter, path, value, source_span, HostMutationOp::Div)
    }

    pub fn rem_diagnostic_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.mutate_diagnostic_path(adapter, path, value, source_span, HostMutationOp::Rem)
    }

    pub fn push_diagnostic_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.mutate_diagnostic_path(adapter, path, value, source_span, HostMutationOp::Push)
    }

    fn mutate_diagnostic_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
        op: HostMutationOp,
    ) -> HostResult<()> {
        let plan = HostTargetPlan::from(&path);
        let target = HostTargetInstance::new(path.root, &plan, &[]);
        self.mutate(adapter, target, op, value, source_span)
    }

    pub fn mutate(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let access = adapter
            .resolve_host_access(HostAccessSpec::new(HostAccessOp::Mutate(op), target.plan))
            .map_err(|error| error.with_source_span_if_absent(source_span))?;
        self.mutate_resolved(adapter, access, target, op, rhs, source_span)
    }

    pub fn mutate_resolved(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        adapter
            .mutate_host(access, target, op, rhs)
            .map_err(|error| error.with_source_span_if_absent(source_span))
    }

    pub fn remove_diagnostic_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let plan = HostTargetPlan::from(&path);
        let target = HostTargetInstance::new(path.root, &plan, &[]);
        self.remove(adapter, target, source_span)
    }

    pub fn remove(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        target: HostTargetInstance<'_>,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let access = adapter
            .resolve_host_access(HostAccessSpec::new(HostAccessOp::Remove, target.plan))
            .map_err(|error| error.with_source_span_if_absent(source_span))?;
        self.remove_resolved(adapter, access, target, source_span)
    }

    pub fn remove_resolved(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        adapter
            .remove_host(access, target)
            .map_err(|error| error.with_source_span_if_absent(source_span))
    }

    pub fn call_diagnostic_path_method(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        method: HostMethodId,
        args: Vec<HostValue>,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        let plan = HostTargetPlan::from(&path);
        let target = HostTargetInstance::new(path.root, &plan, &[]);
        self.call(adapter, target, method, &args, source_span)
    }

    pub fn call(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        let access = adapter
            .resolve_host_access(HostAccessSpec::new(HostAccessOp::Call(method), target.plan))
            .map_err(|error| error.with_source_span_if_absent(source_span))?;
        self.call_resolved(adapter, access, target, method, args, source_span)
    }

    pub fn call_resolved(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        adapter
            .call_host(access, target, method, args)
            .map_err(|error| error.with_source_span_if_absent(source_span))
    }

    pub fn require_fresh_ref(host_ref: HostRef, snapshot: &HostObjectSnapshot) -> HostResult<()> {
        if host_ref.type_id != snapshot.type_id {
            return Err(HostError::new(HostErrorKind::TypeMismatch {
                expected: host_ref.type_id,
                actual: snapshot.type_id,
            }));
        }
        if host_ref.object_id != snapshot.object_id {
            return Err(HostError::new(HostErrorKind::ObjectMismatch {
                expected: host_ref.object_id,
                actual: snapshot.object_id,
            }));
        }
        if host_ref.generation != snapshot.generation {
            return Err(HostError::new(HostErrorKind::StaleGeneration {
                expected: host_ref.generation,
                actual: snapshot.generation,
            }));
        }
        Ok(())
    }
}
