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
