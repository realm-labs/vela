use vela_common::HostTypeId;
use vela_host::lease::{HostLeaseKind, HostLeaseRequestSet};
use vela_host::path::HostRef;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use super::{CallableContract, CallableIdentity};

/// Provenance category for an exact-object lease request. Borrowed-return and
/// nested-reborrow sources extend this enum without changing the root export
/// adapter contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostLeaseSource {
    RootBinding,
}

/// One named host-parameter lease request built by a low-level export adapter.
/// It contains no Rust pointer and is safe to use in deterministic diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostParamLeaseRequest {
    pub callable_identity: CallableIdentity,
    pub callable: String,
    pub parameter_identity: u64,
    pub parameter: String,
    pub argument_index: usize,
    pub canonical_host_identity: HostRef,
    pub expected_concrete_type: HostTypeId,
    pub mode: HostLeaseKind,
    pub source: HostLeaseSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostLeaseArgumentSource {
    Receiver,
    Argument(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostLeaseParameterPlan {
    parameter_index: usize,
    source: HostLeaseArgumentSource,
    expected_concrete_type: HostTypeId,
    mode: HostLeaseKind,
}

impl HostLeaseParameterPlan {
    #[must_use]
    pub const fn argument(
        parameter_index: usize,
        argument_index: usize,
        expected_concrete_type: HostTypeId,
        mode: HostLeaseKind,
    ) -> Self {
        Self {
            parameter_index,
            source: HostLeaseArgumentSource::Argument(argument_index),
            expected_concrete_type,
            mode,
        }
    }

    #[must_use]
    pub const fn receiver(
        parameter_index: usize,
        expected_concrete_type: HostTypeId,
        mode: HostLeaseKind,
    ) -> Self {
        Self {
            parameter_index,
            source: HostLeaseArgumentSource::Receiver,
            expected_concrete_type,
            mode,
        }
    }
}

/// Registration-time host boundary plan used by generated Rust thunks.
///
/// Stable callable and parameter metadata stays in this plan. Successful calls
/// only inspect arguments and build the inline canonical request set; owned
/// diagnostics are materialized only on an error path.
#[derive(Clone, Debug)]
pub struct PreparedHostLeasePlan {
    contract: CallableContract,
    expected_arity: usize,
    parameters: Box<[HostLeaseParameterPlan]>,
}

impl PreparedHostLeasePlan {
    #[must_use]
    pub fn new(
        contract: CallableContract,
        expected_arity: usize,
        parameters: impl IntoIterator<Item = HostLeaseParameterPlan>,
    ) -> Self {
        let parameters = parameters.into_iter().collect::<Box<[_]>>();
        debug_assert!(
            parameters
                .iter()
                .all(|parameter| parameter.parameter_index < contract.parameters.len())
        );
        debug_assert!(parameters.iter().all(|parameter| match parameter.source {
            HostLeaseArgumentSource::Receiver => true,
            HostLeaseArgumentSource::Argument(index) => index < expected_arity,
        }));
        Self {
            contract,
            expected_arity,
            parameters,
        }
    }

    #[must_use]
    pub fn callable(&self) -> &str {
        &self.contract.public_path
    }

    pub fn prepare(&self, args: &[OwnedValue]) -> VmResult<HostLeaseRequestSet> {
        self.prepare_inner(None, args)
    }

    pub fn prepare_method(
        &self,
        receiver: HostRef,
        args: &[OwnedValue],
    ) -> VmResult<HostLeaseRequestSet> {
        self.prepare_inner(Some(receiver), args)
    }

    fn prepare_inner(
        &self,
        receiver: Option<HostRef>,
        args: &[OwnedValue],
    ) -> VmResult<HostLeaseRequestSet> {
        if args.len() != self.expected_arity {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: self.contract.public_path.clone(),
                expected: self.expected_arity,
                actual: args.len(),
            }));
        }

        let mut requests = HostLeaseRequestSet::new();
        for (request_index, parameter) in self.parameters.iter().enumerate() {
            let parameter_contract = self
                .contract
                .parameters
                .get(parameter.parameter_index)
                .ok_or_else(invalid_parameter_plan)?;
            let root = match parameter.source {
                HostLeaseArgumentSource::Receiver => receiver.ok_or_else(|| {
                    VmError::new(VmErrorKind::TypeMismatch {
                        operation: "prepared host method receiver",
                    })
                })?,
                HostLeaseArgumentSource::Argument(index) => match args.get(index) {
                    Some(OwnedValue::HostRef(root)) => *root,
                    _ => {
                        return Err(VmError::new(VmErrorKind::TypeMismatch {
                            operation: "exported Rust host parameter",
                        }));
                    }
                },
            };
            if root.type_id != parameter.expected_concrete_type {
                return Err(VmError::new(VmErrorKind::HostArgumentTypeMismatch {
                    callable: self.contract.public_path.clone(),
                    parameter: parameter_contract.name.clone(),
                    expected: parameter.expected_concrete_type,
                    actual: root.type_id,
                }));
            }
            for (previous_index, (previous_root, previous_mode)) in requests.iter().enumerate() {
                if *previous_root != root
                    || (*previous_mode == HostLeaseKind::Shared
                        && parameter.mode == HostLeaseKind::Shared)
                {
                    continue;
                }
                let previous_parameter = self
                    .contract
                    .parameters
                    .get(self.parameters[previous_index].parameter_index)
                    .ok_or_else(invalid_parameter_plan)?;
                return Err(VmError::new(VmErrorKind::AliasedMutableHostArguments {
                    callable: self.contract.public_path.clone(),
                    first_parameter: previous_parameter.name.clone(),
                    second_parameter: parameter_contract.name.clone(),
                }));
            }
            debug_assert_eq!(request_index, requests.len());
            requests.push((root, parameter.mode));
        }
        Ok(requests)
    }
}

impl HostParamLeaseRequest {
    pub fn from_argument(
        contract: &CallableContract,
        parameter_index: usize,
        argument_index: usize,
        expected_concrete_type: HostTypeId,
        mode: HostLeaseKind,
        argument: &OwnedValue,
    ) -> VmResult<Self> {
        let parameter_contract = contract.parameters.get(parameter_index).ok_or_else(|| {
            VmError::new(VmErrorKind::ArityMismatch {
                name: contract.public_path.clone(),
                expected: parameter_index.saturating_add(1),
                actual: contract.parameters.len(),
            })
        })?;
        let callable = contract.public_path.clone();
        let parameter = parameter_contract.name.clone();
        let root = match argument {
            OwnedValue::HostRef(root) => *root,
            _ => {
                return Err(VmError::new(VmErrorKind::TypeMismatch {
                    operation: "exported Rust host parameter",
                }));
            }
        };
        if root.type_id != expected_concrete_type {
            return Err(VmError::new(VmErrorKind::HostArgumentTypeMismatch {
                callable,
                parameter,
                expected: expected_concrete_type,
                actual: root.type_id,
            }));
        }
        Ok(Self {
            callable_identity: contract.identity,
            callable,
            parameter_identity: parameter_contract.identity,
            parameter,
            argument_index,
            canonical_host_identity: root,
            expected_concrete_type,
            mode,
            source: HostLeaseSource::RootBinding,
        })
    }
}

/// Validates all pairwise alias relationships before any lease is acquired,
/// then returns the canonical request set consumed atomically by the runtime
/// adapter.
pub fn preflight_host_parameter_leases(
    requests: &[HostParamLeaseRequest],
) -> VmResult<HostLeaseRequestSet> {
    for (index, first) in requests.iter().enumerate() {
        for second in &requests[index + 1..] {
            if first.canonical_host_identity != second.canonical_host_identity {
                continue;
            }
            if first.mode == HostLeaseKind::Shared && second.mode == HostLeaseKind::Shared {
                continue;
            }
            return Err(VmError::new(VmErrorKind::AliasedMutableHostArguments {
                callable: first.callable.clone(),
                first_parameter: first.parameter.clone(),
                second_parameter: second.parameter.clone(),
            }));
        }
    }
    Ok(requests
        .iter()
        .map(|request| (request.canonical_host_identity, request.mode))
        .collect())
}

fn invalid_parameter_plan() -> VmError {
    VmError::new(VmErrorKind::TypeMismatch {
        operation: "prepared host lease parameter",
    })
}
