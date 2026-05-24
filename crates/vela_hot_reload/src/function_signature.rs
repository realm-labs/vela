use vela_bytecode::CodeObject;

use crate::{HotReloadError, HotReloadErrorKind, HotReloadPolicy, HotReloadResult};

pub(crate) fn ensure_compatible_function_signature(
    name: &str,
    old_code: &CodeObject,
    new_code: &CodeObject,
    policy: &HotReloadPolicy,
) -> HotReloadResult<()> {
    if new_code.params.len() < old_code.params.len() {
        return Err(HotReloadError::new(
            HotReloadErrorKind::DeletedFunctionParameters {
                function: name.to_owned(),
                old: old_code.params.clone(),
                new: new_code.params.clone(),
            },
        ));
    }

    let existing_params_changed = old_code
        .params
        .iter()
        .zip(&new_code.params)
        .any(|(old, new)| old != new);
    if existing_params_changed {
        return Err(HotReloadError::new(
            HotReloadErrorKind::ChangedFunctionParameters {
                function: name.to_owned(),
                old: old_code.params.clone(),
                new: new_code.params.clone(),
            },
        ));
    }

    let appended_without_defaults = new_code
        .params
        .iter()
        .enumerate()
        .skip(old_code.params.len())
        .filter(|(index, _)| {
            !new_code
                .param_defaults
                .get(*index)
                .copied()
                .unwrap_or(false)
        })
        .map(|(_, param)| param.clone())
        .collect::<Vec<_>>();
    if !appended_without_defaults.is_empty() {
        return Err(HotReloadError::new(
            HotReloadErrorKind::AddedFunctionParametersWithoutDefaults {
                function: name.to_owned(),
                added: appended_without_defaults,
            },
        ));
    }

    let appended = new_code
        .params
        .iter()
        .skip(old_code.params.len())
        .cloned()
        .collect::<Vec<_>>();
    if !appended.is_empty() && !policy.allow_defaulted_parameter_additions() {
        return Err(HotReloadError::new(
            HotReloadErrorKind::AddedFunctionParametersDenied {
                function: name.to_owned(),
                added: appended,
            },
        ));
    }

    Ok(())
}
