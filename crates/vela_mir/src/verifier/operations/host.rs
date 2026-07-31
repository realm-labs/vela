use crate::verifier::{FunctionVerifier, MirVerifyError, MirVerifyErrorKind, MirVerifyTarget};
use crate::{
    CompileMethodClass, CompileTypeClass, HostFieldTarget, HostMethodTarget, HostTypeTarget,
    MirHostOperation, MirHostPath, MirHostPathSegment, MirOperand, MirSourceOrigin,
    MirTypeContract, MirValueType,
};

use super::support::{
    arity_accepts, bad_target, destination_accepts, destination_contract, error, method_target,
    missing_target, require_type, satisfies_contract, type_error, verify_contract,
};

#[derive(Clone, Copy)]
enum HostState {
    Exact(HostTypeTarget),
    Unknown,
    NonHost,
}

#[derive(Clone, Copy)]
enum TerminalAccess {
    Root,
    Field {
        readable: bool,
        writable: bool,
        variant: bool,
    },
    Index {
        readable: bool,
        writable: bool,
        mutable: bool,
        removable: bool,
    },
}

struct PathResult {
    state: HostState,
    contract: Option<MirTypeContract>,
    terminal: TerminalAccess,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_host(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    operation: &MirHostOperation,
    destination: Option<MirValueType>,
) -> Result<(), MirVerifyError> {
    if let MirHostOperation::ReleaseBorrowLease { root }
    | MirHostOperation::TryReleaseBorrowLease { root } = operation
    {
        let root_type = verifier.operand_type(root, block, Some(statement), origin)?;
        if !matches!(root_type, MirValueType::Dynamic | MirValueType::Host(_)) {
            return Err(host_error(
                verifier,
                block,
                statement,
                origin,
                "borrow lease release requires a host operand",
            ));
        }
        let result_type = if matches!(operation, MirHostOperation::TryReleaseBorrowLease { .. }) {
            MirValueType::Primitive(vela_common::PrimitiveTag::Bool)
        } else {
            MirValueType::Unit
        };
        return destination_accepts(verifier, block, statement, origin, destination, result_type);
    }
    let (root, path) = match operation {
        MirHostOperation::ReleaseBorrowLease { .. }
        | MirHostOperation::TryReleaseBorrowLease { .. } => unreachable!(),
        MirHostOperation::Read { root, path }
        | MirHostOperation::Write { root, path, .. }
        | MirHostOperation::Mutate { root, path, .. }
        | MirHostOperation::Remove { root, path }
        | MirHostOperation::Call { root, path, .. } => (root, path),
    };
    verify_host_type(verifier, origin, path.root_type)?;
    let root_type = verifier.operand_type(root, block, Some(statement), origin)?;
    if root_type != MirValueType::Dynamic && root_type != MirValueType::Host(path.root_type) {
        return Err(host_error(
            verifier,
            block,
            statement,
            origin,
            "host root operand disagrees with its path root type",
        ));
    }
    let result = verify_host_path(verifier, block, statement, origin, path)?;
    verify_prefix_access(verifier, block, statement, origin, path)?;

    match operation {
        MirHostOperation::ReleaseBorrowLease { .. }
        | MirHostOperation::TryReleaseBorrowLease { .. } => unreachable!(),
        MirHostOperation::Read { .. } => {
            require_terminal(
                verifier,
                block,
                statement,
                origin,
                result.terminal,
                HostAccessKind::Read,
            )?;
            destination_contract(
                verifier,
                block,
                statement,
                origin,
                destination,
                result.contract.as_ref(),
            )?;
        }
        MirHostOperation::Write { value, .. } => {
            require_terminal(
                verifier,
                block,
                statement,
                origin,
                result.terminal,
                HostAccessKind::Write,
            )?;
            verify_host_value(
                verifier,
                block,
                statement,
                origin,
                value,
                result.contract.as_ref(),
            )?;
        }
        MirHostOperation::Mutate { value, .. } => {
            require_terminal(
                verifier,
                block,
                statement,
                origin,
                result.terminal,
                HostAccessKind::Mutate,
            )?;
            verify_host_value(
                verifier,
                block,
                statement,
                origin,
                value,
                result.contract.as_ref(),
            )?;
        }
        MirHostOperation::Remove { .. } => require_terminal(
            verifier,
            block,
            statement,
            origin,
            result.terminal,
            HostAccessKind::Remove,
        )?,
        MirHostOperation::Call {
            target, arguments, ..
        } => {
            verify_host_method(verifier, origin, target)?;
            if !matches!(result.state, HostState::Exact(owner) if owner == target.owner)
                || !arity_accepts(&target.signature, arguments.len())
            {
                return Err(host_error(
                    verifier,
                    block,
                    statement,
                    origin,
                    "host call receiver type or arity violates its method signature",
                ));
            }
            for (argument, parameter) in arguments.iter().zip(&target.signature.parameters) {
                verify_host_value(
                    verifier,
                    block,
                    statement,
                    origin,
                    argument,
                    parameter.contract.as_ref(),
                )?;
            }
            destination_contract(
                verifier,
                block,
                statement,
                origin,
                destination,
                target.signature.return_contract.as_ref(),
            )?;
        }
    }
    Ok(())
}

fn verify_host_path(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    path: &MirHostPath,
) -> Result<PathResult, MirVerifyError> {
    let mut state = HostState::Exact(path.root_type);
    let mut contract = Some(MirTypeContract::Host(path.root_type));
    let mut terminal = TerminalAccess::Root;
    for segment in &path.segments {
        match segment {
            MirHostPathSegment::Field(target) | MirHostPathSegment::VariantField(target) => {
                if matches!(state, HostState::NonHost)
                    || matches!(state, HostState::Exact(owner) if owner != target.owner)
                {
                    return Err(host_error(
                        verifier,
                        block,
                        statement,
                        origin,
                        "host field owner contradicts the preceding path value",
                    ));
                }
                let variant = matches!(segment, MirHostPathSegment::VariantField(_));
                let field = verify_host_field(verifier, origin, target, variant)?;
                contract = field.contract.clone();
                state = host_state(verifier, contract.as_ref());
                terminal = TerminalAccess::Field {
                    readable: target.access.readable,
                    writable: target.access.writable,
                    variant,
                };
            }
            MirHostPathSegment::Index { value, capability }
            | MirHostPathSegment::Key { value, capability } => {
                verify_capability(verifier, origin, capability)?;
                if let Some(expected) = capability.key.as_ref() {
                    let actual = verifier.operand_type(value, block, Some(statement), origin)?;
                    if !satisfies_contract(actual, expected) {
                        return Err(type_error(
                            verifier,
                            block,
                            Some(statement),
                            origin,
                            "host path key/index",
                            actual,
                        ));
                    }
                }
                contract = capability.value.clone();
                state = host_state(verifier, contract.as_ref());
                terminal = index_access(capability);
            }
            MirHostPathSegment::ConstantIndex { capability, .. }
            | MirHostPathSegment::ConstantKey { capability, .. } => {
                verify_capability(verifier, origin, capability)?;
                contract = capability.value.clone();
                state = host_state(verifier, contract.as_ref());
                terminal = index_access(capability);
            }
        }
    }
    Ok(PathResult {
        state,
        contract,
        terminal,
    })
}

fn verify_prefix_access(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    path: &MirHostPath,
) -> Result<(), MirVerifyError> {
    for segment in path
        .segments
        .iter()
        .take(path.segments.len().saturating_sub(1))
    {
        let readable = match segment {
            MirHostPathSegment::Field(field) | MirHostPathSegment::VariantField(field) => {
                field.access.readable
            }
            MirHostPathSegment::Index { capability, .. }
            | MirHostPathSegment::Key { capability, .. }
            | MirHostPathSegment::ConstantIndex { capability, .. }
            | MirHostPathSegment::ConstantKey { capability, .. } => {
                capability.readable
                    || capability.writable
                    || capability.mutable
                    || capability.removable
                    || capability.value.is_some()
            }
        };
        if !readable {
            return Err(host_error(
                verifier,
                block,
                statement,
                origin,
                "host path traverses a non-readable prefix",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum HostAccessKind {
    Read,
    Write,
    Mutate,
    Remove,
}

fn require_terminal(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    terminal: TerminalAccess,
    operation: HostAccessKind,
) -> Result<(), MirVerifyError> {
    let allowed = match (operation, terminal) {
        (HostAccessKind::Read, TerminalAccess::Field { readable, .. })
        | (HostAccessKind::Read, TerminalAccess::Index { readable, .. }) => readable,
        (
            HostAccessKind::Write,
            TerminalAccess::Field {
                writable, variant, ..
            },
        ) => writable || variant,
        (HostAccessKind::Write, TerminalAccess::Index { writable, .. }) => writable,
        (
            HostAccessKind::Mutate,
            TerminalAccess::Field {
                writable, variant, ..
            },
        ) => writable || variant,
        (HostAccessKind::Mutate, TerminalAccess::Index { mutable, .. }) => mutable,
        (HostAccessKind::Remove, TerminalAccess::Index { removable, .. }) => removable,
        (HostAccessKind::Read, TerminalAccess::Root)
        | (HostAccessKind::Write, TerminalAccess::Root)
        | (HostAccessKind::Mutate, TerminalAccess::Root)
        | (HostAccessKind::Remove, TerminalAccess::Root)
        | (HostAccessKind::Remove, TerminalAccess::Field { .. }) => false,
    };
    if allowed {
        Ok(())
    } else {
        Err(host_error(
            verifier,
            block,
            statement,
            origin,
            "HostAccess operation bypasses its terminal path capability",
        ))
    }
}

fn verify_host_value(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    value: &MirOperand,
    contract: Option<&MirTypeContract>,
) -> Result<(), MirVerifyError> {
    if let Some(contract) = contract {
        let actual = verifier.operand_type(value, block, Some(statement), origin)?;
        if !satisfies_contract(actual, contract) {
            return Err(type_error(
                verifier,
                block,
                Some(statement),
                origin,
                "host write/mutate value",
                actual,
            ));
        }
    }
    Ok(())
}

fn verify_capability(
    verifier: &FunctionVerifier<'_>,
    origin: MirSourceOrigin,
    capability: &crate::CompileHostIndexCapability,
) -> Result<(), MirVerifyError> {
    if let Some(contract) = capability.key.as_ref() {
        verify_contract(verifier, origin, contract)?;
    }
    if let Some(contract) = capability.value.as_ref() {
        verify_contract(verifier, origin, contract)?;
    }
    Ok(())
}

fn index_access(capability: &crate::CompileHostIndexCapability) -> TerminalAccess {
    TerminalAccess::Index {
        readable: capability.readable,
        writable: capability.writable,
        mutable: capability.mutable,
        removable: capability.removable,
    }
}

fn host_state(verifier: &FunctionVerifier<'_>, contract: Option<&MirTypeContract>) -> HostState {
    match contract {
        Some(MirTypeContract::Host(target)) => HostState::Exact(*target),
        Some(MirTypeContract::Definition(type_id)) => verifier
            .program
            .targets()
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
            .map_or(HostState::NonHost, HostState::Exact),
        Some(MirTypeContract::Any) | None => HostState::Unknown,
        Some(
            MirTypeContract::Primitive(_)
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
        ) => HostState::NonHost,
    }
}

fn verify_host_field<'a>(
    verifier: &'a FunctionVerifier<'_>,
    origin: MirSourceOrigin,
    target: &HostFieldTarget,
    variant: bool,
) -> Result<&'a crate::CompileFieldDescriptor, MirVerifyError> {
    verify_host_type(verifier, origin, target.owner)?;
    let field = verifier
        .program
        .targets()
        .field(target.semantic)
        .ok_or_else(|| missing_target(verifier, origin, MirVerifyTarget::Field(target.semantic)))?;
    if field.owner != target.owner.semantic
        || field.variant.is_some() != variant
        || field.host_runtime != Some(target.runtime)
        || field.access != target.access
    {
        return Err(bad_target(
            verifier,
            origin,
            MirVerifyTarget::Field(target.semantic),
            "host field owner, variant, runtime identity, or access policy disagrees",
        ));
    }
    Ok(field)
}

fn verify_host_method(
    verifier: &FunctionVerifier<'_>,
    origin: MirSourceOrigin,
    target: &HostMethodTarget,
) -> Result<(), MirVerifyError> {
    verify_host_type(verifier, origin, target.owner)?;
    let method = method_target(verifier, target.owner.semantic, target.semantic, origin)?;
    if method.signature != target.signature
        || method.access != target.access
        || !matches!(method.class, CompileMethodClass::Host { runtime } if runtime == target.runtime)
    {
        return Err(bad_target(
            verifier,
            origin,
            MirVerifyTarget::Method {
                owner: target.owner.semantic,
                method: target.semantic,
            },
            "host method runtime identity, signature, or access policy disagrees",
        ));
    }
    Ok(())
}

pub(super) fn verify_host_type(
    verifier: &FunctionVerifier<'_>,
    origin: MirSourceOrigin,
    target: HostTypeTarget,
) -> Result<(), MirVerifyError> {
    let descriptor = require_type(verifier, target.semantic, origin)?;
    if !matches!(descriptor.class, CompileTypeClass::Host { runtime } if runtime == target.runtime)
    {
        return Err(bad_target(
            verifier,
            origin,
            MirVerifyTarget::Type(target.semantic),
            "host type runtime identity disagrees",
        ));
    }
    Ok(())
}

fn host_error(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    detail: &str,
) -> MirVerifyError {
    error(
        verifier,
        Some(block),
        Some(statement),
        origin,
        MirVerifyErrorKind::InvalidHostContract(detail.to_owned()),
    )
}
