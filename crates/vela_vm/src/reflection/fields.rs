use std::sync::Arc;

use vela_reflect::registry::TypeRegistry;
use vela_reflect::{self as reflect};

use crate::{Value, Vm, expect_arity, expect_string, value_from_reflect, value_to_reflect};

use super::common::{check_host_ref_inspection, check_reflect_policy};

pub(super) fn register(
    vm: &mut Vm,
    registry: &Arc<TypeRegistry>,
    policy: &reflect::permissions::ReflectPolicy,
    lookup_budget: &Arc<reflect::permissions::ReflectLookupBudget>,
) {
    let fields_registry = Arc::clone(registry);
    let fields_policy = policy.clone();
    let fields_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.fields", move |args, _host| {
        check_reflect_policy(
            &fields_policy,
            &fields_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        if args.is_empty() {
            return value_from_reflect(reflect::members::all_fields_with_policy(
                &fields_registry,
                &fields_policy,
            ));
        }
        expect_arity("reflect.fields", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.fields")?;
        check_host_ref_inspection(&fields_policy, &target)?;
        value_from_reflect(reflect::members::fields_with_policy(
            &fields_registry,
            &target,
            &fields_policy,
        )?)
    });

    let field_registry = Arc::clone(registry);
    let field_policy = policy.clone();
    let field_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.field", move |args, _host| {
        check_reflect_policy(
            &field_policy,
            &field_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.field", args, 2)?;
        let target = value_to_reflect(&args[0], "reflect.field")?;
        check_host_ref_inspection(&field_policy, &target)?;
        let field_name = expect_string(&args[1], "reflect.field")?;
        value_from_reflect(reflect::members::field_with_policy(
            &field_registry,
            &target,
            field_name,
            &field_policy,
        )?)
    });

    let has_field_registry = Arc::clone(registry);
    let has_field_policy = policy.clone();
    let has_field_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.has_field", move |args, _host| {
        check_reflect_policy(
            &has_field_policy,
            &has_field_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.has_field", args, 2)?;
        let target = value_to_reflect(&args[0], "reflect.has_field")?;
        check_host_ref_inspection(&has_field_policy, &target)?;
        let field_name = expect_string(&args[1], "reflect.has_field")?;
        Ok(Value::Bool(reflect::members::has_field_with_policy(
            &has_field_registry,
            &target,
            field_name,
            &has_field_policy,
        )?))
    });
}
