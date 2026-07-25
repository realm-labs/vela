use vela_common::ReceiverCapability;
use vela_host::error::HostErrorKind;
use vela_host::lease::HostLeaseKind;
use vela_host::path::{HostPath, HostRef};
use vela_vm::HostExecution;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::native::EffectSet;
use crate::permission::CapabilitySet;

#[derive(Clone, Copy)]
pub(super) enum ScopedHostEnvelope {
    Direct,
    OptionSome,
    ResultOk,
    Tuple,
    OptionSomeTuple,
    ResultOkTuple,
}

impl ScopedHostEnvelope {
    pub(super) fn wrap(self, roots: Vec<HostRef>) -> OwnedValue {
        match self {
            Self::Direct => OwnedValue::HostRef(single_scoped_root(&roots)),
            Self::OptionSome => {
                let root = single_scoped_root(&roots);
                OwnedValue::enum_variant("Option", "Some", [("0", OwnedValue::HostRef(root))])
            }
            Self::ResultOk => {
                let root = single_scoped_root(&roots);
                OwnedValue::enum_variant("Result", "Ok", [("0", OwnedValue::HostRef(root))])
            }
            Self::Tuple => OwnedValue::tuple(roots.into_iter().map(OwnedValue::HostRef)),
            Self::OptionSomeTuple => OwnedValue::enum_variant(
                "Option",
                "Some",
                [(
                    "0",
                    OwnedValue::tuple(roots.into_iter().map(OwnedValue::HostRef)),
                )],
            ),
            Self::ResultOkTuple => OwnedValue::enum_variant(
                "Result",
                "Ok",
                [(
                    "0",
                    OwnedValue::tuple(roots.into_iter().map(OwnedValue::HostRef)),
                )],
            ),
        }
    }
}

fn single_scoped_root(roots: &[HostRef]) -> HostRef {
    let [root] = roots else {
        panic!("single scoped host return must retain exactly one root");
    };
    *root
}

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

pub(super) fn check_method_receiver(
    required: ReceiverCapability,
    receiver: &HostPath,
    host: &HostExecution<'_>,
) -> VmResult<()> {
    let available = host.adapter.host_receiver_access(receiver.root);
    let allowed = match required {
        ReceiverCapability::Shared => true,
        ReceiverCapability::Exclusive => available == HostLeaseKind::Exclusive,
        ReceiverCapability::Owned | ReceiverCapability::Construct => false,
    };
    if allowed {
        return Ok(());
    }
    let action = match required {
        ReceiverCapability::Owned => "call owned receiver method",
        ReceiverCapability::Shared => "call shared receiver method",
        ReceiverCapability::Exclusive => "call exclusive receiver method",
        ReceiverCapability::Construct => "call constructor as instance method",
    };
    Err(VmError::new(VmErrorKind::Host(
        HostErrorKind::PermissionDenied {
            path: receiver.clone(),
            action,
        },
    )))
}
