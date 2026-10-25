use crate::native::EffectSet;
use crate::permission::CapabilitySet;
use vela_vm::error::{VmError, VmErrorKind, VmResult};

pub(crate) fn check_capabilities(
    native: &str,
    effects: &EffectSet,
    capabilities: CapabilitySet,
) -> VmResult<()> {
    let required = effects.required_capability_set();
    if capabilities.contains_all(required) {
        return Ok(());
    }

    if let Some(capability) = required.difference(capabilities).iter().next() {
        return Err(VmError::new(VmErrorKind::PermissionDenied {
            native: native.to_owned(),
            capability: capability.as_str().to_owned(),
        }));
    }
    Ok(())
}
