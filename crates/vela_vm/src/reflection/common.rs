use vela_reflect as reflect;

use crate::VmResult;

pub(super) fn check_reflect_policy(
    policy: &reflect::ReflectPolicy,
    lookup_budget: &reflect::ReflectLookupBudget,
    permission: reflect::ReflectPermission,
) -> VmResult<()> {
    policy.require(permission)?;
    lookup_budget.consume()?;
    Ok(())
}

pub(super) fn check_host_ref_inspection(
    policy: &reflect::ReflectPolicy,
    target: &reflect::ReflectValue,
) -> VmResult<()> {
    if matches!(target, reflect::ReflectValue::HostRef(_)) {
        policy.require(reflect::ReflectPermission::InspectHostPath)?;
    }
    Ok(())
}
