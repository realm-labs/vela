use std::sync::Arc;

use vela_reflect::{self as reflect, TypeRegistry};

use crate::{Value, Vm, expect_arity, expect_string, value_from_reflect, value_to_reflect};

use super::common::{check_host_ref_inspection, check_reflect_policy};

pub(super) fn register(
    vm: &mut Vm,
    registry: &Arc<TypeRegistry>,
    policy: &reflect::ReflectPolicy,
    lookup_budget: &Arc<reflect::ReflectLookupBudget>,
) {
    let methods_registry = Arc::clone(registry);
    let methods_policy = policy.clone();
    let methods_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.methods", move |args, _host| {
        check_reflect_policy(
            &methods_policy,
            &methods_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        if args.is_empty() {
            return value_from_reflect(reflect::method_metadata_list_with_policy(
                &methods_registry,
                &methods_policy,
            ));
        }
        expect_arity("reflect.methods", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.methods")?;
        check_host_ref_inspection(&methods_policy, &target)?;
        value_from_reflect(reflect::methods_with_policy(
            &methods_registry,
            &target,
            &methods_policy,
        )?)
    });

    let method_registry = Arc::clone(registry);
    let method_policy = policy.clone();
    let method_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.method", move |args, _host| {
        check_reflect_policy(
            &method_policy,
            &method_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.method", args, 2)?;
        let target = value_to_reflect(&args[0], "reflect.method")?;
        check_host_ref_inspection(&method_policy, &target)?;
        let method_name = expect_string(&args[1], "reflect.method")?;
        value_from_reflect(reflect::method_metadata_with_policy(
            &method_registry,
            &target,
            method_name,
            &method_policy,
        )?)
    });

    let has_method_registry = Arc::clone(registry);
    let has_method_policy = policy.clone();
    let has_method_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.has_method", move |args, _host| {
        check_reflect_policy(
            &has_method_policy,
            &has_method_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.has_method", args, 2)?;
        let target = value_to_reflect(&args[0], "reflect.has_method")?;
        check_host_ref_inspection(&has_method_policy, &target)?;
        let method_name = expect_string(&args[1], "reflect.has_method")?;
        Ok(Value::Bool(reflect::has_method_with_policy(
            &has_method_registry,
            &target,
            method_name,
            &has_method_policy,
        )?))
    });
}
