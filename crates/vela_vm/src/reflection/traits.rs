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
    let traits_registry = Arc::clone(registry);
    let traits_policy = policy.clone();
    let traits_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.traits", move |args, _host| {
        check_reflect_policy(
            &traits_policy,
            &traits_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        if args.is_empty() {
            return value_from_reflect(reflect::trait_metadata_list(&traits_registry));
        }
        expect_arity("reflect.traits", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.traits")?;
        check_host_ref_inspection(&traits_policy, &target)?;
        value_from_reflect(reflect::trait_metadata(&traits_registry, &target)?)
    });

    let trait_registry = Arc::clone(registry);
    let trait_policy = policy.clone();
    let trait_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.trait_info", move |args, _host| {
        check_reflect_policy(
            &trait_policy,
            &trait_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.trait_info", args, 1)?;
        let trait_name = expect_string(&args[0], "reflect.trait_info")?;
        value_from_reflect(reflect::trait_metadata_by_name(
            &trait_registry,
            trait_name,
        )?)
    });

    let has_trait_registry = Arc::clone(registry);
    let has_trait_policy = policy.clone();
    let has_trait_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.has_trait", move |args, _host| {
        check_reflect_policy(
            &has_trait_policy,
            &has_trait_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.has_trait", args, 1)?;
        let trait_name = expect_string(&args[0], "reflect.has_trait")?;
        Ok(Value::Bool(reflect::has_trait(
            &has_trait_registry,
            trait_name,
        )))
    });

    let implements_registry = Arc::clone(registry);
    let implements_policy = policy.clone();
    let implements_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.implements", move |args, _host| {
        check_reflect_policy(
            &implements_policy,
            &implements_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.implements", args, 2)?;
        let target = value_to_reflect(&args[0], "reflect.implements")?;
        check_host_ref_inspection(&implements_policy, &target)?;
        let trait_target = value_to_reflect(&args[1], "reflect.implements")?;
        Ok(Value::Bool(reflect::implements(
            &implements_registry,
            &target,
            &trait_target,
        )?))
    });
}
