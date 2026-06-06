use std::collections::HashMap;
use std::sync::Arc;

use vela_reflect::registry::TypeRegistry;
use vela_reflect::{self as reflect};

use crate::owned_value::OwnedValue;
use crate::{
    ExecutionBudget, HostExecution, HostNativeFunction, NativeFunction, Vm, VmError, VmErrorKind,
    VmResult, expect_arity, expect_string, value_from_reflect, value_to_reflect,
};

use super::common::check_reflect_policy;

#[derive(Clone, Default)]
pub(super) struct ReflectedFunctionCalls {
    natives: HashMap<String, NativeFunction>,
    host_natives: HashMap<String, HostNativeFunction>,
}

impl ReflectedFunctionCalls {
    pub(super) fn new(
        natives: HashMap<String, NativeFunction>,
        host_natives: HashMap<String, HostNativeFunction>,
    ) -> Self {
        Self {
            natives,
            host_natives,
        }
    }

    fn call(
        &self,
        name: &str,
        args: &[OwnedValue],
        host: &mut HostExecution<'_>,
        budget: Option<&mut ExecutionBudget>,
    ) -> VmResult<OwnedValue> {
        if let Some(native) = self.natives.get(name) {
            return native(args);
        }
        if let Some(native) = self.host_natives.get(name) {
            return native(args, host, budget);
        }
        Err(VmError::new(VmErrorKind::UnknownNative {
            name: name.to_owned(),
        }))
    }
}

pub(super) fn register(
    vm: &mut Vm,
    registry: &Arc<TypeRegistry>,
    policy: &reflect::permissions::ReflectPolicy,
    lookup_budget: &Arc<reflect::permissions::ReflectLookupBudget>,
    function_calls: ReflectedFunctionCalls,
) {
    let get_registry = Arc::clone(registry);
    let get_policy = policy.clone();
    let get_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect::get", move |args, host| {
        check_reflect_policy(
            &get_policy,
            &get_budget,
            reflect::permissions::ReflectPermission::ReadValueFields,
        )?;
        expect_arity("reflect::get", args, 2)?;
        let target = value_to_reflect(&args[0], "reflect::get")?;
        let field = expect_string(&args[1], "reflect::get")?;
        let mut ctx = reflect::value::ReflectContext {
            registry: &get_registry,
            adapter: host.adapter,
            tx: &mut *host.tx,
        };
        let value = reflect::value::get_with_policy(&mut ctx, &target, field, &get_policy)?;
        value_from_reflect(value)
    });

    let set_registry = Arc::clone(registry);
    let set_policy = policy.clone();
    let set_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect::set", move |args, host| {
        check_reflect_policy(
            &set_policy,
            &set_budget,
            reflect::permissions::ReflectPermission::WriteValueFields,
        )?;
        expect_arity("reflect::set", args, 3)?;
        let target = value_to_reflect(&args[0], "reflect::set")?;
        let field = expect_string(&args[1], "reflect::set")?;
        let value = value_to_reflect(&args[2], "reflect::set")?;
        let mut ctx = reflect::value::ReflectContext {
            registry: &set_registry,
            adapter: host.adapter,
            tx: &mut *host.tx,
        };
        value_from_reflect(reflect::value::set_with_policy(
            &mut ctx,
            &target,
            field,
            value,
            &set_policy,
        )?)
    });

    let call_registry = Arc::clone(registry);
    let call_policy = policy.clone();
    let call_budget = Arc::clone(lookup_budget);
    vm.register_budgeted_host_native("reflect::call", move |args, host, mut budget| {
        check_reflect_policy(
            &call_policy,
            &call_budget,
            reflect::permissions::ReflectPermission::CallMethods,
        )?;
        if args.is_empty() {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: "reflect::call".to_owned(),
                expected: 1,
                actual: args.len(),
            }));
        }
        let target = value_to_reflect(&args[0], "reflect::call")?;
        if let Some(function_name) = reflect::modules::callable_function_name_with_policy(
            &call_registry,
            &target,
            &call_policy,
        )? {
            return function_calls.call(&function_name, &args[1..], host, budget.as_deref_mut());
        }
        if args.len() < 2 {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: "reflect::call".to_owned(),
                expected: 2,
                actual: args.len(),
            }));
        }
        let method = expect_string(&args[1], "reflect::call")?;
        let call_args = args[2..]
            .iter()
            .map(|arg| value_to_reflect(arg, "reflect::call"))
            .collect::<VmResult<Vec<_>>>()?;
        let mut ctx = reflect::value::ReflectContext {
            registry: &call_registry,
            adapter: host.adapter,
            tx: &mut *host.tx,
        };
        let value =
            reflect::value::call_with_policy(&mut ctx, &target, method, call_args, &call_policy)?;
        value_from_reflect(value)
    });
}
