use vela_reflect as reflect;

use crate::VmResult;

pub(super) fn check_reflect_policy(
    policy: &reflect::permissions::ReflectPolicy,
    lookup_budget: &reflect::permissions::ReflectLookupBudget,
    permission: reflect::permissions::ReflectPermission,
) -> VmResult<()> {
    policy.require(permission)?;
    lookup_budget.consume()?;
    Ok(())
}

pub(super) fn check_host_ref_inspection(
    policy: &reflect::permissions::ReflectPolicy,
    target: &reflect::value::ReflectValue,
) -> VmResult<()> {
    if matches!(target, reflect::value::ReflectValue::HostRef(_)) {
        policy.require(reflect::permissions::ReflectPermission::InspectHostPath)?;
    }
    Ok(())
}
