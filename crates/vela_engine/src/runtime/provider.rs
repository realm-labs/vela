use vela_bytecode::{LinkedMethodDispatchKind, ProviderReceiverPlan};
use vela_def::MethodId;
use vela_hir::provider::ProviderKey;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::heap_execution::HeapExecution;
use vela_vm::{allocate_zero_field_record, budget::ExecutionBudget};

use super::{RuntimeImageStorage, RuntimeImpl, handles::EntryRequest, unknown_method};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderHandle {
    runtime_id: u64,
    key: ProviderKey,
}

impl ProviderHandle {
    #[must_use]
    pub const fn key(&self) -> &ProviderKey {
        &self.key
    }

    #[must_use]
    pub fn method(&self, method: MethodId) -> ProviderMethodTarget {
        ProviderMethodTarget {
            handle: self.clone(),
            method,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderMethodTarget {
    pub(super) handle: ProviderHandle,
    pub(super) method: MethodId,
}

impl<I> RuntimeImpl<I>
where
    I: RuntimeImageStorage,
{
    pub fn provider_handle(&self, key: &ProviderKey) -> VmResult<ProviderHandle> {
        self.resolve_provider(key)?;
        Ok(ProviderHandle {
            runtime_id: self.state.id,
            key: key.clone(),
        })
    }

    fn resolve_provider(&self, key: &ProviderKey) -> VmResult<&vela_bytecode::LinkedProviderEntry> {
        self.image
            .linked_artifact()
            .package_metadata()
            .and_then(|metadata| metadata.installed_providers().get(key))
            .ok_or_else(|| unknown_provider(key))
    }

    fn resolve_provider_method(
        &self,
        key: &ProviderKey,
        method: MethodId,
    ) -> VmResult<ResolvedProviderCall> {
        let provider = self.resolve_provider(key)?;
        let dispatch = provider
            .method(method)
            .ok_or_else(|| unknown_method(format!("provider method {method:?}")))?;
        let program = self.image.linked_program();
        let dispatch = program
            .method_dispatch(dispatch)
            .ok_or_else(|| unknown_method(format!("provider method {method:?}")))?;
        let LinkedMethodDispatchKind::Script { function, .. } = dispatch.kind else {
            return Err(unknown_method(format!("provider method {method:?}")));
        };
        let code = program
            .function(function)
            .ok_or_else(|| unknown_method(format!("provider method {method:?}")))?;
        let linked_type = program
            .ty(provider.provider_type())
            .ok_or_else(|| unknown_provider(key))?;
        let ProviderReceiverPlan::FreshZeroField { shape } = provider.receiver();
        Ok(ResolvedProviderCall {
            function,
            type_id: linked_type.id,
            shape,
            type_name: program.debug_name(linked_type.debug_name).to_owned(),
            debug_name: format!("provider {}::{method:?}", key.provider()),
            params: code
                .params
                .iter()
                .skip(1)
                .map(|param| program.debug_name(*param).to_owned())
                .collect(),
            param_defaults: code.param_defaults.iter().skip(1).copied().collect(),
        })
    }

    pub(super) fn resolve_provider_target(
        &mut self,
        target: ProviderMethodTarget,
        budget: &mut ExecutionBudget,
    ) -> VmResult<EntryRequest> {
        if target.handle.runtime_id != self.state.id {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "provider handle belongs to another runtime",
            }));
        }
        let resolved = self.resolve_provider_method(&target.handle.key, target.method)?;
        let state = &mut self.state;
        let receiver = {
            let mut heap = HeapExecution::new(&mut state.script_globals.heap);
            let value = allocate_zero_field_record(
                resolved.type_name,
                resolved.type_id,
                resolved.shape,
                &mut heap,
                Some(budget),
            )?;
            state.script_globals.retain(state.id, value)
        };
        Ok(EntryRequest {
            name: resolved.debug_name,
            function: resolved.function,
            params: resolved.params,
            param_defaults: resolved.param_defaults,
            receiver: Some(receiver),
        })
    }
}

struct ResolvedProviderCall {
    function: vela_bytecode::ScriptFunctionHandle,
    type_id: vela_def::TypeId,
    shape: vela_common::ShapeId,
    type_name: String,
    debug_name: String,
    params: Vec<String>,
    param_defaults: Vec<bool>,
}

fn unknown_provider(key: &ProviderKey) -> VmError {
    VmError::new(VmErrorKind::UnknownFunction {
        name: format!(
            "provider {}:{}:{}",
            key.package(),
            key.service().get(),
            key.provider()
        ),
    })
}
