use crate::{
    CompileTryFamily, CompileTryLayoutTarget, CompileTryTarget, MirBuildError, MirSourceOrigin,
};

use super::SnapshotValidator;

pub(super) fn validate(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for ((function, expression), target) in &validator.snapshot.try_targets {
        let origin = validator.retained_origin(
            &validator.snapshot.origins.try_targets,
            &(*function, *expression),
        );
        validator.require_root(*function, origin, "try target")?;
        match target {
            CompileTryTarget::Expected(layout) => {
                validate_layout(validator, *layout, origin, "expected try layout")?;
            }
            CompileTryTarget::Dynamic { option, result } => {
                if option.family != CompileTryFamily::Option
                    || result.family != CompileTryFamily::Result
                {
                    return Err(validator.error(
                        origin,
                        "dynamic try layouts do not match their Option and Result slots",
                    ));
                }
                validate_layout(validator, *option, origin, "dynamic Option try layout")?;
                validate_layout(validator, *result, origin, "dynamic Result try layout")?;
            }
        }
    }
    Ok(())
}

fn validate_layout(
    validator: &SnapshotValidator<'_>,
    layout: CompileTryLayoutTarget,
    origin: MirSourceOrigin,
    context: &str,
) -> Result<(), MirBuildError> {
    let owner = validator.require_type(layout.type_id, origin, context)?;
    if layout.continue_variant == layout.break_variant {
        return Err(validator.error(
            origin,
            format!("{context} uses one variant for both continue and break"),
        ));
    }

    let continue_variant = validator
        .snapshot
        .variant_descriptor(layout.continue_variant)
        .ok_or_else(|| {
            validator.error(
                origin,
                format!(
                    "{context} references missing continue variant #{}",
                    layout.continue_variant.get()
                ),
            )
        })?;
    let break_variant = validator
        .snapshot
        .variant_descriptor(layout.break_variant)
        .ok_or_else(|| {
            validator.error(
                origin,
                format!(
                    "{context} references missing break variant #{}",
                    layout.break_variant.get()
                ),
            )
        })?;
    if continue_variant.owner != layout.type_id
        || break_variant.owner != layout.type_id
        || !owner.variants.contains(&layout.continue_variant)
        || !owner.variants.contains(&layout.break_variant)
    {
        return Err(validator.error(
            origin,
            format!("{context} has inconsistent type-to-variant ownership"),
        ));
    }

    let continue_payload = validator
        .snapshot
        .field_descriptor(layout.continue_payload)
        .ok_or_else(|| {
            validator.error(
                origin,
                format!(
                    "{context} references missing continue payload #{}",
                    layout.continue_payload.get()
                ),
            )
        })?;
    if continue_payload.owner != layout.type_id
        || continue_payload.variant != Some(layout.continue_variant)
        || !continue_variant.fields.contains(&layout.continue_payload)
    {
        return Err(validator.error(
            origin,
            format!("{context} has inconsistent continue-payload ownership"),
        ));
    }
    Ok(())
}
