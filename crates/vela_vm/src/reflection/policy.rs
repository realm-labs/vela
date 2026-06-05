use std::sync::Arc;

use vela_reflect as reflect;

use crate::owned_value::OwnedValue;
use crate::{Vm, expect_arity, expect_string};

use super::common::check_reflect_policy;

pub(super) fn register(
    vm: &mut Vm,
    policy: &reflect::permissions::ReflectPolicy,
    lookup_budget: &Arc<reflect::permissions::ReflectLookupBudget>,
) {
    let permissions_policy = policy.clone();
    let permissions_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect::permissions", move |args, _host| {
        check_reflect_policy(
            &permissions_policy,
            &permissions_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect::permissions", args, 0)?;
        Ok(OwnedValue::Array(
            reflect::permissions::permission_names(&permissions_policy)
                .into_iter()
                .map(|permission| OwnedValue::String(permission.to_owned()))
                .collect(),
        ))
    });

    let has_permission_policy = policy.clone();
    let has_permission_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect::has_permission", move |args, _host| {
        check_reflect_policy(
            &has_permission_policy,
            &has_permission_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect::has_permission", args, 1)?;
        let permission = expect_string(&args[0], "reflect::has_permission")?;
        Ok(OwnedValue::Bool(reflect::permissions::has_permission(
            &has_permission_policy,
            permission,
        )?))
    });
}
