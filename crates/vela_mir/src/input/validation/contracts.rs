use crate::{CompileSignature, MirBuildError, MirSourceOrigin, MirTypeContract};

use super::SnapshotValidator;

const MAX_CONTRACT_DEPTH: usize = 128;

pub(super) fn validate_signature(
    validator: &SnapshotValidator<'_>,
    signature: &CompileSignature,
    origin: MirSourceOrigin,
    context: &str,
) -> Result<(), MirBuildError> {
    for (index, parameter) in signature.parameters.iter().enumerate() {
        if let Some(contract) = &parameter.contract {
            validate_contract(
                validator,
                contract,
                origin,
                &format!("{context} parameter {index}"),
            )?;
        }
    }
    if let Some(contract) = &signature.return_contract {
        validate_contract(validator, contract, origin, &format!("{context} return"))?;
    }
    Ok(())
}

pub(super) fn validate_contract(
    validator: &SnapshotValidator<'_>,
    contract: &MirTypeContract,
    origin: MirSourceOrigin,
    context: &str,
) -> Result<(), MirBuildError> {
    validate_contract_at(validator, contract, origin, context, 0)
}

fn validate_contract_at(
    validator: &SnapshotValidator<'_>,
    contract: &MirTypeContract,
    origin: MirSourceOrigin,
    context: &str,
    depth: usize,
) -> Result<(), MirBuildError> {
    if depth >= MAX_CONTRACT_DEPTH {
        return Err(validator.error(
            origin,
            format!("{context} exceeds the maximum nested contract depth"),
        ));
    }
    let nested = |contract: &MirTypeContract, suffix: &str| {
        validate_contract_at(
            validator,
            contract,
            origin,
            &format!("{context} {suffix}"),
            depth + 1,
        )
    };
    match contract {
        MirTypeContract::Any
        | MirTypeContract::Primitive(_)
        | MirTypeContract::Range
        | MirTypeContract::Callable { .. } => {}
        MirTypeContract::Array(element)
        | MirTypeContract::Set(element)
        | MirTypeContract::Iterator(element)
        | MirTypeContract::Option(element) => {
            if let Some(element) = element {
                nested(element, "element")?;
            }
        }
        MirTypeContract::Map { key, value } => {
            if let Some(key) = key {
                nested(key, "key")?;
            }
            if let Some(value) = value {
                nested(value, "value")?;
            }
        }
        MirTypeContract::Tuple(elements) => {
            for (index, element) in elements.iter().enumerate() {
                if let Some(element) = element {
                    nested(element, &format!("element {index}"))?;
                }
            }
        }
        MirTypeContract::Result { ok, err } => {
            if let Some(ok) = ok {
                nested(ok, "ok")?;
            }
            if let Some(err) = err {
                nested(err, "err")?;
            }
        }
        MirTypeContract::Definition(type_id) => {
            validator.require_type(*type_id, origin, context)?;
        }
        MirTypeContract::Shape { type_id, shape } => {
            let descriptor = validator.require_type(*type_id, origin, context)?;
            if descriptor.shape != Some(*shape) {
                return Err(validator.error(
                    origin,
                    format!(
                        "{context} references shape #{}, but type #{} owns {:?}",
                        shape.get(),
                        type_id.get(),
                        descriptor.shape
                    ),
                ));
            }
        }
        MirTypeContract::Variant { type_id, variant } => {
            let type_descriptor = validator.require_type(*type_id, origin, context)?;
            let variant_descriptor =
                validator
                    .snapshot
                    .variant_descriptor(*variant)
                    .ok_or_else(|| {
                        validator.error(
                            origin,
                            format!("{context} references missing variant #{}", variant.get()),
                        )
                    })?;
            if variant_descriptor.owner != *type_id || !type_descriptor.variants.contains(variant) {
                return Err(validator.error(
                    origin,
                    format!(
                        "{context} variant #{} does not belong to type #{}",
                        variant.get(),
                        type_id.get()
                    ),
                ));
            }
        }
        MirTypeContract::Host(target) => {
            validator.require_host_type(*target, origin, context)?;
        }
    }
    Ok(())
}
