use std::sync::Arc;

use vela_reflect::registry::TypeRegistry;
use vela_reflect::{self as reflect};

use crate::owned_value::OwnedValue as Value;
use crate::{Vm, expect_arity, expect_string, value_from_reflect, value_to_reflect};

use super::common::{check_host_ref_inspection, check_reflect_policy};

pub(super) fn register(
    vm: &mut Vm,
    registry: &Arc<TypeRegistry>,
    policy: &reflect::permissions::ReflectPolicy,
    lookup_budget: &Arc<reflect::permissions::ReflectLookupBudget>,
) {
    let variants_registry = Arc::clone(registry);
    let variants_policy = policy.clone();
    let variants_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect::variants", move |args, _host| {
        check_reflect_policy(
            &variants_policy,
            &variants_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        if args.is_empty() {
            return value_from_reflect(reflect::members::all_variants_with_policy(
                &variants_registry,
                &variants_policy,
            ));
        }
        expect_arity("reflect::variants", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect::variants")?;
        check_host_ref_inspection(&variants_policy, &target)?;
        value_from_reflect(reflect::members::variants_with_policy(
            &variants_registry,
            &target,
            &variants_policy,
        )?)
    });

    let variant_info_registry = Arc::clone(registry);
    let variant_info_policy = policy.clone();
    let variant_info_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect::variant_info", move |args, _host| {
        check_reflect_policy(
            &variant_info_policy,
            &variant_info_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect::variant_info", args, 2)?;
        let target = value_to_reflect(&args[0], "reflect::variant_info")?;
        check_host_ref_inspection(&variant_info_policy, &target)?;
        let variant_name = expect_string(&args[1], "reflect::variant_info")?;
        value_from_reflect(reflect::members::variant_info_with_policy(
            &variant_info_registry,
            &target,
            variant_name,
            &variant_info_policy,
        )?)
    });

    let has_variant_registry = Arc::clone(registry);
    let has_variant_policy = policy.clone();
    let has_variant_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect::has_variant", move |args, _host| {
        check_reflect_policy(
            &has_variant_policy,
            &has_variant_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect::has_variant", args, 2)?;
        let target = value_to_reflect(&args[0], "reflect::has_variant")?;
        check_host_ref_inspection(&has_variant_policy, &target)?;
        let variant_name = expect_string(&args[1], "reflect::has_variant")?;
        Ok(Value::Bool(reflect::members::has_variant(
            &has_variant_registry,
            &target,
            variant_name,
        )?))
    });

    let variant_policy = policy.clone();
    let variant_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect::variant", move |args, _host| {
        check_reflect_policy(
            &variant_policy,
            &variant_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect::variant", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect::variant")?;
        check_host_ref_inspection(&variant_policy, &target)?;
        value_from_reflect(reflect::members::variant(&target)?)
    });

    let variant_is_registry = Arc::clone(registry);
    let variant_is_policy = policy.clone();
    let variant_is_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect::variant_is", move |args, _host| {
        check_reflect_policy(
            &variant_is_policy,
            &variant_is_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect::variant_is", args, 2)?;
        let target = value_to_reflect(&args[0], "reflect::variant_is")?;
        check_host_ref_inspection(&variant_is_policy, &target)?;
        let variant_name = expect_string(&args[1], "reflect::variant_is")?;
        Ok(Value::Bool(reflect::members::variant_is(
            &variant_is_registry,
            &target,
            variant_name,
        )?))
    });
}
