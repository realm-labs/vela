use vela_bytecode::{LinkedMethodDispatchKind, ProviderReceiverPlan};
use vela_def::MethodId;
use vela_hir::provider::ProviderKey;
use vela_host::access::HostAccess;
use vela_host::adapter::ScriptStateAdapter;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::heap_execution::HeapExecution;
use vela_vm::{
    HostExecution, LinkedRuntimeCodeCall, PersistentHeapExecution, allocate_zero_field_record,
};

use super::call_args::{CallArgsAdapter, EmptyStateAdapter};
use super::global_store::GlobalStoreAdapter;
use super::{
    CallArgs, CallOptions, RuntimeImageStorage, RuntimeImpl, VelaValue, runtime_vm, unknown_method,
};

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

    pub fn call_provider(
        &mut self,
        key: &ProviderKey,
        method: MethodId,
        args: CallArgs<'_>,
        options: CallOptions,
    ) -> VmResult<VelaValue> {
        let mut adapter = EmptyStateAdapter;
        self.call_provider_with_adapter(key, method, args, options, &mut adapter)
    }

    pub fn call_provider_handle(
        &mut self,
        handle: &ProviderHandle,
        method: MethodId,
        args: CallArgs<'_>,
        options: CallOptions,
    ) -> VmResult<VelaValue> {
        if handle.runtime_id != self.state.id {
            return Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "provider handle belongs to another runtime",
            }));
        }
        self.call_provider(&handle.key, method, args, options)
    }

    pub fn call_provider_with_adapter(
        &mut self,
        key: &ProviderKey,
        method: MethodId,
        mut args: CallArgs<'_>,
        options: CallOptions,
        adapter: &mut dyn ScriptStateAdapter,
    ) -> VmResult<VelaValue> {
        let target = self.resolve_provider_method(key, method)?;
        let mut budget = options.budget();
        let state = &mut self.state;
        let receiver = {
            let mut heap = HeapExecution::new(&mut state.script_globals.heap);
            let value = allocate_zero_field_record(
                target.type_name,
                target.type_id,
                target.shape,
                &mut heap,
                Some(&mut budget),
            )?;
            state.script_globals.retain(state.id, value)
        };
        let resolved = args.resolve_values(
            &target.debug_name,
            &target.params,
            &target.param_defaults,
            state.id,
            &mut state.script_globals.heap,
            &mut budget,
        )?;
        let mut access = HostAccess::new();
        let mut adapter = CallArgsAdapter::new(&mut args, adapter);
        let mut adapter = GlobalStoreAdapter::new(&mut state.globals, &mut adapter);
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut access,
            script_globals: Some(&state.script_globals.values),
        };
        let vm = runtime_vm(
            self.image.engine(),
            self.image.program_image(),
            self.hot_reload.as_ref(),
        );
        let roots = state.script_globals.roots();
        let mut method_args = Vec::with_capacity(resolved.len().saturating_add(1));
        method_args.push(receiver.value);
        method_args.extend_from_slice(&resolved);
        let result = vm.run_linked_runtime_code_call(LinkedRuntimeCodeCall {
            artifact: self.image.linked_artifact(),
            function: target.function,
            args: &method_args,
            host: &mut host,
            persistent: PersistentHeapExecution {
                heap: &mut state.script_globals.heap,
                roots: &roots,
            },
            budget: &mut budget,
            inline_caches: Some(&state.sidecars),
            bytecode_profiler: Some(&state.sidecars),
        })?;
        Ok(state.script_globals.retain(state.id, result))
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
