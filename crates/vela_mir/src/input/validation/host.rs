use crate::{
    CompileFieldDescriptor, CompileHostIndexCapability, CompileHostPathSegment,
    CompileHostPathTarget, CompileMethodClass, CompileTypeClass, CompileTypeDescriptor,
    HostFieldTarget, HostMethodTarget, HostTypeTarget, MirBuildError, MirSourceOrigin,
    MirTypeContract,
};

use super::SnapshotValidator;
use super::contracts::{validate_contract, validate_signature};

pub(super) fn require_host_type<'a>(
    validator: &SnapshotValidator<'a>,
    target: HostTypeTarget,
    origin: MirSourceOrigin,
    context: &str,
) -> Result<&'a CompileTypeDescriptor, MirBuildError> {
    let descriptor = validator.require_type(target.semantic, origin, context)?;
    match descriptor.class {
        CompileTypeClass::Host { runtime } if runtime == target.runtime => Ok(descriptor),
        CompileTypeClass::Host { runtime } => Err(validator.error(
            origin,
            format!(
                "{context} uses host runtime type #{}, but semantic type #{} owns runtime #{}",
                target.runtime.get(),
                target.semantic.get(),
                runtime.get()
            ),
        )),
        CompileTypeClass::ScriptRecord
        | CompileTypeClass::ScriptEnum
        | CompileTypeClass::OpaqueExternal
        | CompileTypeClass::Registry
        | CompileTypeClass::Standard => Err(validator.error(
            origin,
            format!(
                "{context} uses semantic type #{} as a host type",
                target.semantic.get()
            ),
        )),
    }
}

pub(super) fn validate_field<'a>(
    validator: &SnapshotValidator<'a>,
    target: &HostFieldTarget,
    origin: MirSourceOrigin,
    context: &str,
) -> Result<&'a CompileFieldDescriptor, MirBuildError> {
    require_host_type(validator, target.owner, origin, context)?;
    let descriptor = validator
        .snapshot
        .field_descriptor(target.semantic)
        .ok_or_else(|| {
            validator.error(
                origin,
                format!(
                    "{context} references missing host field #{}",
                    target.semantic.get()
                ),
            )
        })?;
    if descriptor.owner != target.owner.semantic {
        return Err(validator.error(
            origin,
            format!(
                "{context} host field #{} belongs to type #{}, not #{}",
                target.semantic.get(),
                descriptor.owner.get(),
                target.owner.semantic.get()
            ),
        ));
    }
    if descriptor.host_runtime != Some(target.runtime) {
        return Err(validator.error(
            origin,
            format!(
                "{context} host field #{} uses runtime #{}, but its descriptor owns {:?}",
                target.semantic.get(),
                target.runtime.get(),
                descriptor.host_runtime
            ),
        ));
    }
    if descriptor.access != target.access {
        return Err(validator.error(
            origin,
            format!(
                "{context} host field #{} access does not match its descriptor",
                target.semantic.get()
            ),
        ));
    }
    Ok(descriptor)
}

pub(super) fn validate_method<'a>(
    validator: &SnapshotValidator<'a>,
    target: &HostMethodTarget,
    origin: MirSourceOrigin,
    context: &str,
) -> Result<(), MirBuildError> {
    require_host_type(validator, target.owner, origin, context)?;
    let descriptor =
        validator.require_method(target.owner.semantic, target.semantic, origin, context)?;
    let CompileMethodClass::Host { runtime } = descriptor.class else {
        return Err(validator.error(
            origin,
            format!(
                "{context} uses non-host method #{} as a host method",
                target.semantic.get()
            ),
        ));
    };
    if runtime != target.runtime {
        return Err(validator.error(
            origin,
            format!(
                "{context} host method #{} uses runtime #{}, but its descriptor owns #{}",
                target.semantic.get(),
                target.runtime.get(),
                runtime.get()
            ),
        ));
    }
    if descriptor.signature != target.signature || descriptor.access != target.access {
        return Err(validator.error(
            origin,
            format!(
                "{context} host method #{} metadata does not match its descriptor",
                target.semantic.get()
            ),
        ));
    }
    validate_signature(
        validator,
        &target.signature,
        origin,
        &format!("{context} host method #{}", target.semantic.get()),
    )
}

pub(super) fn validate_capability(
    validator: &SnapshotValidator<'_>,
    capability: &CompileHostIndexCapability,
    origin: MirSourceOrigin,
    context: &str,
) -> Result<(), MirBuildError> {
    if let Some(key) = &capability.key {
        validate_contract(validator, key, origin, &format!("{context} key"))?;
    }
    if let Some(value) = &capability.value {
        validate_contract(validator, value, origin, &format!("{context} value"))?;
    }
    Ok(())
}

pub(super) fn validate_path(
    validator: &SnapshotValidator<'_>,
    path: &CompileHostPathTarget,
    origin: MirSourceOrigin,
    context: &str,
) -> Result<(), MirBuildError> {
    require_host_type(validator, path.root_type, origin, context)?;
    let mut expected_owner = HostValueState::Exact(path.root_type);
    for (index, segment) in path.segments.iter().enumerate() {
        let segment_context = format!("{context} segment {index}");
        match segment {
            CompileHostPathSegment::Field(field) | CompileHostPathSegment::VariantField(field) => {
                match expected_owner {
                    HostValueState::Exact(expected) if field.owner != expected => {
                        return Err(validator.error(
                            origin,
                            format!(
                                "{segment_context} owner {:?} does not match preceding host type {:?}",
                                field.owner, expected
                            ),
                        ));
                    }
                    HostValueState::NonHost => {
                        return Err(validator.error(
                            origin,
                            format!("{segment_context} follows a proven non-host value"),
                        ));
                    }
                    HostValueState::Exact(_) | HostValueState::Unknown => {}
                }
                let descriptor = validate_field(validator, field, origin, &segment_context)?;
                match segment {
                    CompileHostPathSegment::Field(_) if descriptor.variant.is_some() => {
                        return Err(validator.error(
                            origin,
                            format!("{segment_context} uses a variant field as an ordinary field"),
                        ));
                    }
                    CompileHostPathSegment::VariantField(_) if descriptor.variant.is_none() => {
                        return Err(validator.error(
                            origin,
                            format!("{segment_context} uses an ordinary field as a variant field"),
                        ));
                    }
                    CompileHostPathSegment::Field(_) | CompileHostPathSegment::VariantField(_) => {}
                    CompileHostPathSegment::ConstantIndex { .. }
                    | CompileHostPathSegment::ConstantKey { .. }
                    | CompileHostPathSegment::DynamicIndex { .. }
                    | CompileHostPathSegment::DynamicKey { .. } => unreachable!(),
                }
                expected_owner = host_value_state(validator, descriptor.contract.as_ref());
            }
            CompileHostPathSegment::ConstantIndex { capability, .. }
            | CompileHostPathSegment::ConstantKey { capability, .. }
            | CompileHostPathSegment::DynamicIndex { capability, .. }
            | CompileHostPathSegment::DynamicKey { capability, .. } => {
                validate_capability(validator, capability, origin, &segment_context)?;
                expected_owner = host_value_state(validator, capability.value.as_ref());
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum HostValueState {
    Exact(HostTypeTarget),
    Unknown,
    NonHost,
}

fn host_value_state(
    validator: &SnapshotValidator<'_>,
    contract: Option<&MirTypeContract>,
) -> HostValueState {
    match contract {
        Some(MirTypeContract::Host(target)) => HostValueState::Exact(*target),
        Some(MirTypeContract::Definition(type_id)) => validator
            .snapshot
            .type_descriptor(*type_id)
            .and_then(|descriptor| match descriptor.class {
                CompileTypeClass::Host { runtime } => Some(HostTypeTarget {
                    semantic: *type_id,
                    runtime,
                }),
                CompileTypeClass::ScriptRecord
                | CompileTypeClass::ScriptEnum
                | CompileTypeClass::OpaqueExternal
                | CompileTypeClass::Registry
                | CompileTypeClass::Standard => None,
            })
            .map_or(HostValueState::NonHost, HostValueState::Exact),
        Some(MirTypeContract::Any) | None => HostValueState::Unknown,
        Some(
            MirTypeContract::TaskError
            | MirTypeContract::Primitive(_)
            | MirTypeContract::Range
            | MirTypeContract::Array(_)
            | MirTypeContract::Map { .. }
            | MirTypeContract::Set(_)
            | MirTypeContract::Iterator(_)
            | MirTypeContract::Tuple(_)
            | MirTypeContract::Option(_)
            | MirTypeContract::Result { .. }
            | MirTypeContract::Callable { .. }
            | MirTypeContract::Shape { .. }
            | MirTypeContract::Variant { .. },
        ) => HostValueState::NonHost,
    }
}
