//! Detached values passed across erased Host method calls.
//!
//! This module connects the VM's owned Value representation to the
//! lifetime-neutral [`HostCallValue`] representation used by
//! [`vela_host::object::ScriptHostObject`]. Generated and handwritten Host
//! adapters can use the typed helpers to retain the same Rust `Value`
//! conversion experience as statically registered native methods.

use vela_host::call_value::{HostCallField, HostCallMapEntry, HostCallValue};
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::path::HostPath;
use vela_host::resolved::{HostAccessOp, HostAccessSpec};
use vela_host::target::{HostTargetInstance, HostTargetPlan};
use vela_vm::HostExecution;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::{OwnedMapEntry, OwnedValue};

use crate::args::{FromScriptArg, IntoScriptArg};

/// Decodes one detached Host method argument into its Rust Value type.
pub fn decode_host_call_arg<T>(value: &HostCallValue) -> HostResult<T>
where
    T: FromScriptArg,
{
    let value = host_call_to_owned_value(value.clone());
    T::from_script_arg(&value).map_err(|_| invalid_argument(T::TYPE_NAME))
}

/// Encodes a Rust Value type as a detached Host method return value.
pub fn encode_host_call_return<T>(value: T) -> HostResult<HostCallValue>
where
    T: IntoScriptArg,
{
    owned_to_host_call_value(&value.into_script_arg())
        .map_err(|_| invalid_argument("detached Host method return value"))
}

/// Invokes the controlled adapter method vtable when a registered receiver is
/// represented by a HostAccess path rather than a directly leased Rust object.
///
/// Generated method adapters call this only after proving that the receiver
/// itself cannot supply a typed lease. Other lease, permission, and invocation
/// errors remain terminal.
#[doc(hidden)]
pub fn call_registered_host_method_through_adapter(
    receiver: &HostPath,
    args: &[OwnedValue],
    method: vela_common::HostMethodId,
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let plan = HostTargetPlan::from(receiver);
    let target = HostTargetInstance::new(receiver.root, &plan, &[]);
    let access = host
        .adapter
        .resolve_host_access(HostAccessSpec::new(HostAccessOp::Call(method), &plan))?;
    let args = args
        .iter()
        .map(owned_to_host_call_value)
        .collect::<VmResult<Vec<_>>>()?;
    let result = host.adapter.call_host(access, target, method, &args)?;
    Ok(host_call_to_owned_value(result))
}

pub(crate) fn owned_to_host_call_value(value: &OwnedValue) -> VmResult<HostCallValue> {
    match value {
        OwnedValue::Unit => Ok(HostCallValue::Unit),
        OwnedValue::Bool(value) => Ok(HostCallValue::Bool(*value)),
        OwnedValue::Char(value) => Ok(HostCallValue::Char(*value)),
        OwnedValue::Scalar(value) => Ok(HostCallValue::Scalar(*value)),
        OwnedValue::String(value) => Ok(HostCallValue::String(value.clone())),
        OwnedValue::Bytes(value) => Ok(HostCallValue::Bytes(value.clone())),
        OwnedValue::Tuple(values) => values
            .iter()
            .map(owned_to_host_call_value)
            .collect::<VmResult<Vec<_>>>()
            .map(HostCallValue::Tuple),
        OwnedValue::Array(values) => values
            .iter()
            .map(owned_to_host_call_value)
            .collect::<VmResult<Vec<_>>>()
            .map(HostCallValue::Array),
        OwnedValue::Map(entries) => entries
            .iter()
            .map(|entry| {
                Ok(HostCallMapEntry::new(
                    owned_to_host_call_value(&entry.key)?,
                    owned_to_host_call_value(&entry.value)?,
                ))
            })
            .collect::<VmResult<Vec<_>>>()
            .map(HostCallValue::Map),
        OwnedValue::Set(values) => values
            .iter()
            .map(owned_to_host_call_value)
            .collect::<VmResult<Vec<_>>>()
            .map(HostCallValue::Set),
        OwnedValue::Record { type_name, fields } => Ok(HostCallValue::Record {
            type_name: type_name.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| Ok(HostCallField::new(name, owned_to_host_call_value(value)?)))
                .collect::<VmResult<Vec<_>>>()?,
        }),
        OwnedValue::Enum {
            enum_name,
            variant,
            fields,
        } => Ok(HostCallValue::Enum {
            enum_name: enum_name.clone(),
            variant: variant.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| Ok(HostCallField::new(name, owned_to_host_call_value(value)?)))
                .collect::<VmResult<Vec<_>>>()?,
        }),
        OwnedValue::HostRef(value) => Ok(HostCallValue::HostRef(*value)),
        OwnedValue::Closure(_)
        | OwnedValue::Range(_)
        | OwnedValue::PathProxy(_)
        | OwnedValue::Iterator(_) => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "non-detached Host method boundary value",
        })),
    }
}

pub(crate) fn host_call_to_owned_value(value: HostCallValue) -> OwnedValue {
    match value {
        HostCallValue::Unit => OwnedValue::Unit,
        HostCallValue::Bool(value) => OwnedValue::Bool(value),
        HostCallValue::Char(value) => OwnedValue::Char(value),
        HostCallValue::Scalar(value) => OwnedValue::Scalar(value),
        HostCallValue::String(value) => OwnedValue::String(value),
        HostCallValue::Bytes(value) => OwnedValue::Bytes(value),
        HostCallValue::Tuple(values) => {
            OwnedValue::Tuple(values.into_iter().map(host_call_to_owned_value).collect())
        }
        HostCallValue::Array(values) => {
            OwnedValue::Array(values.into_iter().map(host_call_to_owned_value).collect())
        }
        HostCallValue::Map(entries) => OwnedValue::Map(
            entries
                .into_iter()
                .map(|entry| {
                    OwnedMapEntry::new(
                        host_call_to_owned_value(entry.key),
                        host_call_to_owned_value(entry.value),
                    )
                })
                .collect(),
        ),
        HostCallValue::Set(values) => {
            OwnedValue::Set(values.into_iter().map(host_call_to_owned_value).collect())
        }
        HostCallValue::Record { type_name, fields } => OwnedValue::record(
            type_name,
            fields
                .into_iter()
                .map(|field| (field.name, host_call_to_owned_value(field.value))),
        ),
        HostCallValue::Enum {
            enum_name,
            variant,
            fields,
        } => OwnedValue::enum_variant(
            enum_name,
            variant,
            fields
                .into_iter()
                .map(|field| (field.name, host_call_to_owned_value(field.value))),
        ),
        HostCallValue::HostRef(value) => OwnedValue::HostRef(value),
    }
}

fn invalid_argument(expected: &'static str) -> HostError {
    HostError {
        kind: HostErrorKind::InvalidArgument { expected },
        source_span: None,
    }
}
