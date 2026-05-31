use std::sync::Arc;

use vela_reflect::{self as reflect, TypeRegistry};

use crate::{Value, Vm, expect_arity, expect_string, value_from_reflect, value_to_reflect};

use super::common::check_reflect_policy;

pub(super) fn register(
    vm: &mut Vm,
    registry: &Arc<TypeRegistry>,
    policy: &reflect::ReflectPolicy,
    lookup_budget: &Arc<reflect::ReflectLookupBudget>,
) {
    register_module_natives(vm, registry, policy, lookup_budget);
    register_function_natives(vm, registry, policy, lookup_budget);
}

fn register_module_natives(
    vm: &mut Vm,
    registry: &Arc<TypeRegistry>,
    policy: &reflect::ReflectPolicy,
    lookup_budget: &Arc<reflect::ReflectLookupBudget>,
) {
    let module_registry = Arc::clone(registry);
    let module_policy = policy.clone();
    let module_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.module", move |args, _host| {
        check_reflect_policy(
            &module_policy,
            &module_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.module", args, 1)?;
        let module_name = expect_string(&args[0], "reflect.module")?;
        value_from_reflect(reflect::module_metadata_with_policy(
            &module_registry,
            module_name,
            &module_policy,
        )?)
    });

    let has_module_registry = Arc::clone(registry);
    let has_module_policy = policy.clone();
    let has_module_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.has_module", move |args, _host| {
        check_reflect_policy(
            &has_module_policy,
            &has_module_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.has_module", args, 1)?;
        let module_name = expect_string(&args[0], "reflect.has_module")?;
        Ok(Value::Bool(reflect::has_module_with_policy(
            &has_module_registry,
            module_name,
            &has_module_policy,
        )))
    });

    let modules_registry = Arc::clone(registry);
    let modules_policy = policy.clone();
    let modules_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.modules", move |args, _host| {
        check_reflect_policy(
            &modules_policy,
            &modules_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.modules", args, 0)?;
        value_from_reflect(reflect::module_metadata_list_with_policy(
            &modules_registry,
            &modules_policy,
        ))
    });

    let exports_registry = Arc::clone(registry);
    let exports_policy = policy.clone();
    let exports_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.exports", move |args, _host| {
        check_reflect_policy(
            &exports_policy,
            &exports_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.exports", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.exports")?;
        value_from_reflect(reflect::module_exports_for_target_with_policy(
            &exports_registry,
            &target,
            &exports_policy,
        )?)
    });
}

fn register_function_natives(
    vm: &mut Vm,
    registry: &Arc<TypeRegistry>,
    policy: &reflect::ReflectPolicy,
    lookup_budget: &Arc<reflect::ReflectLookupBudget>,
) {
    let function_registry = Arc::clone(registry);
    let function_policy = policy.clone();
    let function_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.function", move |args, _host| {
        check_reflect_policy(
            &function_policy,
            &function_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.function", args, 1)?;
        let function_name = expect_string(&args[0], "reflect.function")?;
        value_from_reflect(reflect::function_metadata_with_policy(
            &function_registry,
            function_name,
            &function_policy,
        )?)
    });

    let has_function_registry = Arc::clone(registry);
    let has_function_policy = policy.clone();
    let has_function_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.has_function", move |args, _host| {
        check_reflect_policy(
            &has_function_policy,
            &has_function_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.has_function", args, 1)?;
        let function_name = expect_string(&args[0], "reflect.has_function")?;
        Ok(Value::Bool(reflect::has_function_with_policy(
            &has_function_registry,
            function_name,
            &has_function_policy,
        )))
    });

    let functions_registry = Arc::clone(registry);
    let functions_policy = policy.clone();
    let functions_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.functions", move |args, _host| {
        check_reflect_policy(
            &functions_policy,
            &functions_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.functions", args, 0)?;
        value_from_reflect(reflect::function_metadata_list_with_policy(
            &functions_registry,
            &functions_policy,
        ))
    });
}
