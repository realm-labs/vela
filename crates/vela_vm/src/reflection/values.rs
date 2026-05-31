use std::sync::Arc;

use vela_host::ScriptStateAdapter;
use vela_reflect::{self as reflect, TypeRegistry};

use crate::{
    Vm, VmError, VmErrorKind, VmResult, expect_arity, expect_string, value_from_reflect,
    value_to_reflect,
};

use super::common::check_reflect_policy;

pub(super) fn register(
    vm: &mut Vm,
    registry: &Arc<TypeRegistry>,
    policy: &reflect::ReflectPolicy,
    lookup_budget: &Arc<reflect::ReflectLookupBudget>,
) {
    let get_registry = Arc::clone(registry);
    let get_policy = policy.clone();
    let get_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.get", move |args, host| {
        check_reflect_policy(
            &get_policy,
            &get_budget,
            reflect::ReflectPermission::ReadValueFields,
        )?;
        expect_arity("reflect.get", args, 2)?;
        let target = value_to_reflect(&args[0], "reflect.get")?;
        let field = expect_string(&args[1], "reflect.get")?;
        let adapter: &dyn ScriptStateAdapter = &*host.adapter;
        let mut ctx = reflect::ReflectContext {
            registry: &get_registry,
            adapter,
            tx: &mut *host.tx,
        };
        let value = reflect::get_with_policy(&mut ctx, &target, field, &get_policy)?;
        value_from_reflect(value)
    });

    let set_registry = Arc::clone(registry);
    let set_policy = policy.clone();
    let set_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.set", move |args, host| {
        check_reflect_policy(
            &set_policy,
            &set_budget,
            reflect::ReflectPermission::WriteValueFields,
        )?;
        expect_arity("reflect.set", args, 3)?;
        let target = value_to_reflect(&args[0], "reflect.set")?;
        let field = expect_string(&args[1], "reflect.set")?;
        let value = value_to_reflect(&args[2], "reflect.set")?;
        let adapter: &dyn ScriptStateAdapter = &*host.adapter;
        let mut ctx = reflect::ReflectContext {
            registry: &set_registry,
            adapter,
            tx: &mut *host.tx,
        };
        value_from_reflect(reflect::set_with_policy(
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
    vm.register_host_native("reflect.call", move |args, host| {
        check_reflect_policy(
            &call_policy,
            &call_budget,
            reflect::ReflectPermission::CallMethods,
        )?;
        if args.len() < 2 {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: "reflect.call".to_owned(),
                expected: 2,
                actual: args.len(),
            }));
        }
        let target = value_to_reflect(&args[0], "reflect.call")?;
        let method = expect_string(&args[1], "reflect.call")?;
        let call_args = args[2..]
            .iter()
            .map(|arg| value_to_reflect(arg, "reflect.call"))
            .collect::<VmResult<Vec<_>>>()?;
        let adapter: &dyn ScriptStateAdapter = &*host.adapter;
        let mut ctx = reflect::ReflectContext {
            registry: &call_registry,
            adapter,
            tx: &mut *host.tx,
        };
        let value = reflect::call_with_policy(&mut ctx, &target, method, call_args, &call_policy)?;
        value_from_reflect(value)
    });
}
