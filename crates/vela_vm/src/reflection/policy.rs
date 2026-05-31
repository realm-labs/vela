use std::sync::Arc;

use vela_reflect as reflect;

use crate::{Value, Vm, expect_arity, expect_string};

use super::common::check_reflect_policy;

pub(super) fn register(
    vm: &mut Vm,
    policy: &reflect::ReflectPolicy,
    lookup_budget: &Arc<reflect::ReflectLookupBudget>,
) {
    let permissions_policy = policy.clone();
    let permissions_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.permissions", move |args, _host| {
        check_reflect_policy(
            &permissions_policy,
            &permissions_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.permissions", args, 0)?;
        Ok(Value::Array(
            reflect::permission_names(&permissions_policy)
                .into_iter()
                .map(|permission| Value::String(permission.to_owned()))
                .collect(),
        ))
    });

    let has_permission_policy = policy.clone();
    let has_permission_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.has_permission", move |args, _host| {
        check_reflect_policy(
            &has_permission_policy,
            &has_permission_budget,
            reflect::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.has_permission", args, 1)?;
        let permission = expect_string(&args[0], "reflect.has_permission")?;
        Ok(Value::Bool(reflect::has_permission(
            &has_permission_policy,
            permission,
        )?))
    });
}
