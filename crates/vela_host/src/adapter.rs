use vela_common::{GlobalSlot, HostMethodId};

use crate::{
    error::{HostError, HostErrorKind, HostResult},
    path::HostRef,
    resolved::{HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess},
    target::HostTargetInstance,
    value::HostValue,
};

#[derive(Clone, Copy, Debug)]
pub struct GlobalBinding<'a> {
    pub name: &'a str,
    pub slot: Option<GlobalSlot>,
}

pub trait ScriptStateAdapter {
    fn host_schema_epoch(&self) -> HostSchemaEpoch {
        HostSchemaEpoch::new(0)
    }

    fn global_ref(&self, global: GlobalBinding<'_>) -> HostResult<HostRef> {
        Err(HostError {
            kind: HostErrorKind::MissingGlobal {
                name: global.name.to_owned(),
            },
            source_span: None,
        })
    }

    fn resolve_host_access(&self, _spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        Ok(ResolvedHostAccess::generic_path(self.host_schema_epoch()))
    }

    fn read_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue>;

    fn write_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()>;

    fn mutate_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()>;

    fn remove_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<()>;

    fn call_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue>;
}
