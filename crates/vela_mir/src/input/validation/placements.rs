use std::collections::BTreeSet;

use crate::{
    CompileCallArguments, CompileCallTarget, CompileCalleeTarget, CompileConstructorField,
    CompileConstructorTarget, CompileConstructorValue, CompileDynamicConstructorField,
    CompileFieldTarget, CompileFunctionClass, CompileMemberTarget, CompileMethodClass,
    CompileParameterDefault, CompilePatternConstructorTarget, CompileSignature, MirBuildError,
    MirSourceOrigin,
};

use super::SnapshotValidator;
use super::contracts::validate_signature;
use super::host;

pub(super) fn validate(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    validate_calls(validator)?;
    validate_members(validator)?;
    validate_constructors(validator)?;
    validate_pattern_constructors(validator)?;
    validate_host_paths(validator)
}

fn validate_calls(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for ((function, expression), target) in &validator.snapshot.calls {
        let origin =
            validator.retained_origin(&validator.snapshot.origins.calls, &(*function, *expression));
        validator.require_root(*function, origin, "call placement")?;
        validate_call(validator, target, origin)?;
    }
    Ok(())
}

fn validate_call(
    validator: &SnapshotValidator<'_>,
    target: &CompileCallTarget,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    let signature = match &target.callee {
        CompileCalleeTarget::ScriptFunction { function, .. } => Some(
            &validator
                .require_script_function(*function, origin, "script call target")?
                .signature,
        ),
        CompileCalleeTarget::ScriptMethod { target, .. } => Some(
            &validator
                .require_script_method(*target, origin, "script method call target")?
                .signature,
        ),
        CompileCalleeTarget::NativeFunction { function, .. } => {
            let descriptor = validator.require_function(*function, origin, "native call target")?;
            if !matches!(
                descriptor.class,
                CompileFunctionClass::Native | CompileFunctionClass::Registry
            ) {
                return Err(validator.error(
                    origin,
                    format!(
                        "native call target #{} has the wrong function class",
                        function.get()
                    ),
                ));
            }
            Some(&descriptor.signature)
        }
        CompileCalleeTarget::StdlibFunction { function, .. }
        | CompileCalleeTarget::SetFromArray { function, .. } => {
            let descriptor =
                validator.require_function(*function, origin, "standard call target")?;
            if descriptor.class != CompileFunctionClass::Stdlib {
                return Err(validator.error(
                    origin,
                    format!(
                        "standard call target #{} has the wrong function class",
                        function.get()
                    ),
                ));
            }
            Some(&descriptor.signature)
        }
        CompileCalleeTarget::Reflection { function, .. } => {
            let descriptor =
                validator.require_function(*function, origin, "reflection call target")?;
            if !matches!(
                descriptor.class,
                CompileFunctionClass::Native | CompileFunctionClass::Registry
            ) {
                return Err(validator.error(
                    origin,
                    format!(
                        "reflection call target #{} has the wrong function class",
                        function.get()
                    ),
                ));
            }
            Some(&descriptor.signature)
        }
        CompileCalleeTarget::ValueMethod { owner, method, .. } => {
            let descriptor =
                validator.require_method(*owner, *method, origin, "value method call target")?;
            if !matches!(
                descriptor.class,
                CompileMethodClass::Value | CompileMethodClass::Registry
            ) {
                return Err(validator.error(
                    origin,
                    format!(
                        "value method call target #{} has the wrong method class",
                        method.get()
                    ),
                ));
            }
            Some(&descriptor.signature)
        }
        CompileCalleeTarget::HostMethod(target) => {
            host::validate_method(validator, target, origin, "host method call target")?;
            Some(&target.signature)
        }
        CompileCalleeTarget::HostRemove { path } | CompileCalleeTarget::HostPush { path } => {
            host::validate_path(validator, path, origin, "host intrinsic call target")?;
            None
        }
        CompileCalleeTarget::Local(_)
        | CompileCalleeTarget::Lambda(_)
        | CompileCalleeTarget::DynamicCallable
        | CompileCalleeTarget::DynamicMethod(_) => None,
    };
    if let Some(signature) = signature {
        validate_signature(validator, signature, origin, "call target signature")?;
    }
    validate_call_arguments(validator, target, signature, origin)
}

fn validate_call_arguments(
    validator: &SnapshotValidator<'_>,
    target: &CompileCallTarget,
    signature: Option<&CompileSignature>,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    match (&target.callee, &target.arguments) {
        (
            CompileCalleeTarget::ScriptFunction { .. } | CompileCalleeTarget::ScriptMethod { .. },
            CompileCallArguments::Script(arguments),
        ) => validate_script_arguments(
            validator,
            arguments,
            signature.expect("script call targets always resolve a signature"),
            origin,
        ),
        (
            CompileCalleeTarget::DynamicCallable | CompileCalleeTarget::DynamicMethod(_),
            CompileCallArguments::Dynamic(arguments),
        ) => {
            if let CompileCalleeTarget::DynamicMethod(method) = &target.callee {
                let positional = arguments
                    .iter()
                    .filter(|argument| argument.name.is_none())
                    .count();
                let named = arguments
                    .iter()
                    .filter_map(|argument| argument.name.clone())
                    .collect::<Vec<_>>();
                if usize::try_from(method.positional_arity) != Ok(positional)
                    || method.named_arguments != named
                {
                    return Err(validator.error(
                        origin,
                        "dynamic method argument metadata disagrees with its operands",
                    ));
                }
            }
            Ok(())
        }
        (
            CompileCalleeTarget::ScriptFunction { .. } | CompileCalleeTarget::ScriptMethod { .. },
            CompileCallArguments::Positional(_) | CompileCallArguments::Dynamic(_),
        ) => Err(validator.error(origin, "script call target lacks parameter-slot arguments")),
        (
            CompileCalleeTarget::DynamicCallable | CompileCalleeTarget::DynamicMethod(_),
            CompileCallArguments::Script(_) | CompileCallArguments::Positional(_),
        ) => Err(validator.error(origin, "dynamic call target lacks dynamic arguments")),
        (
            CompileCalleeTarget::Local(_)
            | CompileCalleeTarget::Lambda(_)
            | CompileCalleeTarget::NativeFunction { .. }
            | CompileCalleeTarget::StdlibFunction { .. }
            | CompileCalleeTarget::ValueMethod { .. }
            | CompileCalleeTarget::HostMethod(_)
            | CompileCalleeTarget::Reflection { .. }
            | CompileCalleeTarget::SetFromArray { .. }
            | CompileCalleeTarget::HostRemove { .. }
            | CompileCalleeTarget::HostPush { .. },
            CompileCallArguments::Positional(_),
        ) => Ok(()),
        (_, CompileCallArguments::Script(_) | CompileCallArguments::Dynamic(_)) => Err(validator
            .error(
                origin,
                "non-script call target has incompatible argument placement",
            )),
    }
}

fn validate_script_arguments(
    validator: &SnapshotValidator<'_>,
    arguments: &[crate::CompileScriptCallArgument],
    signature: &CompileSignature,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    if arguments.len() != signature.parameters.len() {
        return Err(validator.error(
            origin,
            "script call argument slots do not cover the signature",
        ));
    }
    for (index, (argument, parameter)) in arguments.iter().zip(&signature.parameters).enumerate() {
        if usize::try_from(argument.parameter) != Ok(index) {
            return Err(validator.error(origin, "script call argument slots are not contiguous"));
        }
        if argument.value.is_none()
            && !matches!(parameter.default, CompileParameterDefault::HirBody(_))
        {
            return Err(validator.error(
                origin,
                format!("script call omits required parameter {index}"),
            ));
        }
    }
    Ok(())
}

fn validate_members(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for ((function, expression), target) in &validator.snapshot.members {
        let origin = validator.retained_origin(
            &validator.snapshot.origins.members,
            &(*function, *expression),
        );
        validator.require_root(*function, origin, "member placement")?;
        match target {
            CompileMemberTarget::ScriptField(field) => {
                validate_field_target(validator, field, origin, "member placement")?;
            }
            CompileMemberTarget::HostField(field) => {
                host::validate_field(validator, field, origin, "host member placement")?;
            }
            CompileMemberTarget::ScriptMethod { target, .. } => {
                validator.require_script_method(*target, origin, "script member placement")?;
            }
            CompileMemberTarget::ValueMethod { owner, method, .. } => {
                let descriptor =
                    validator.require_method(*owner, *method, origin, "value member placement")?;
                if !matches!(
                    descriptor.class,
                    CompileMethodClass::Value | CompileMethodClass::Registry
                ) {
                    return Err(validator
                        .error(origin, "value member placement targets a non-value method"));
                }
            }
            CompileMemberTarget::TupleIndex(_) | CompileMemberTarget::Dynamic { .. } => {}
        }
    }
    Ok(())
}

fn validate_field_target(
    validator: &SnapshotValidator<'_>,
    target: &CompileFieldTarget,
    origin: MirSourceOrigin,
    context: &str,
) -> Result<(), MirBuildError> {
    match target {
        CompileFieldTarget::RecordSlot {
            type_id,
            shape,
            field,
        } => {
            let owner = validator.require_type(*type_id, origin, context)?;
            let descriptor = validator.snapshot.field_descriptor(*field).ok_or_else(|| {
                validator.error(
                    origin,
                    format!("{context} references missing field #{}", field.get()),
                )
            })?;
            if owner.shape != Some(*shape)
                || descriptor.owner != *type_id
                || descriptor.variant.is_some()
                || !owner.fields.contains(field)
            {
                return Err(validator.error(
                    origin,
                    format!("{context} record field slot is inconsistent"),
                ));
            }
        }
        CompileFieldTarget::VariantSlot {
            type_id,
            variant,
            field,
        } => {
            let owner = validator.require_type(*type_id, origin, context)?;
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
            let field_descriptor =
                validator.snapshot.field_descriptor(*field).ok_or_else(|| {
                    validator.error(
                        origin,
                        format!("{context} references missing field #{}", field.get()),
                    )
                })?;
            if variant_descriptor.owner != *type_id
                || !owner.variants.contains(variant)
                || field_descriptor.owner != *type_id
                || field_descriptor.variant != Some(*variant)
                || !variant_descriptor.fields.contains(field)
            {
                return Err(validator.error(
                    origin,
                    format!("{context} variant field slot is inconsistent"),
                ));
            }
        }
        CompileFieldTarget::Dynamic { .. } => {}
    }
    Ok(())
}

fn validate_constructors(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for ((function, expression), target) in &validator.snapshot.constructors {
        let origin = validator.retained_origin(
            &validator.snapshot.origins.constructors,
            &(*function, *expression),
        );
        validator.require_root(*function, origin, "constructor placement")?;
        match target {
            CompileConstructorTarget::Record {
                type_id,
                shape,
                fields,
            } => {
                let owner = validator.require_type(*type_id, origin, "record constructor")?;
                if owner.shape != Some(*shape) {
                    return Err(
                        validator.error(origin, "record constructor shape disagrees with its type")
                    );
                }
                validate_constructor_fields(
                    validator,
                    fields,
                    &owner.fields,
                    *type_id,
                    None,
                    origin,
                )?;
            }
            CompileConstructorTarget::Variant {
                type_id,
                variant,
                fields,
            } => {
                let owner = validator.require_type(*type_id, origin, "variant constructor")?;
                let variant_descriptor = validator
                    .snapshot
                    .variant_descriptor(*variant)
                    .ok_or_else(|| {
                        validator.error(origin, "variant constructor references a missing variant")
                    })?;
                if variant_descriptor.owner != *type_id || !owner.variants.contains(variant) {
                    return Err(
                        validator.error(origin, "variant constructor owner is inconsistent")
                    );
                }
                validate_constructor_fields(
                    validator,
                    fields,
                    &variant_descriptor.fields,
                    *type_id,
                    Some(*variant),
                    origin,
                )?;
            }
            CompileConstructorTarget::DynamicRecord { type_name, fields } => {
                validate_dynamic_constructor(
                    validator,
                    type_name,
                    None,
                    fields,
                    origin,
                    "dynamic record constructor",
                )?;
            }
            CompileConstructorTarget::DynamicVariant {
                owner_name,
                variant_name,
                fields,
            } => {
                validate_dynamic_constructor(
                    validator,
                    owner_name,
                    Some(variant_name.as_str()),
                    fields,
                    origin,
                    "dynamic variant constructor",
                )?;
            }
        }
    }
    Ok(())
}

fn validate_dynamic_constructor(
    validator: &SnapshotValidator<'_>,
    owner_name: &str,
    variant_name: Option<&str>,
    fields: &[CompileDynamicConstructorField],
    origin: MirSourceOrigin,
    context: &str,
) -> Result<(), MirBuildError> {
    validate_dynamic_owner_and_variant(validator, owner_name, variant_name, origin, context)?;
    let mut unique = BTreeSet::new();
    for field in fields {
        if field.name.is_empty() {
            return Err(validator.error(origin, format!("{context} has an empty field name")));
        }
        if !unique.insert(field.name.as_str()) {
            return Err(validator.error(
                origin,
                format!("{context} has duplicate field name {:?}", field.name),
            ));
        }
    }
    Ok(())
}

fn validate_constructor_fields(
    validator: &SnapshotValidator<'_>,
    fields: &[CompileConstructorField],
    expected_fields: &[vela_def::FieldId],
    owner: vela_def::TypeId,
    variant: Option<vela_def::VariantId>,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    if fields.len() != expected_fields.len() {
        return Err(validator.error(origin, "constructor fields do not cover their descriptor"));
    }
    let mut seen_fields = BTreeSet::new();
    let mut seen_parameters = BTreeSet::new();
    for field in fields {
        if !seen_fields.insert(field.field)
            || !seen_parameters.insert(field.parameter)
            || usize::try_from(field.parameter)
                .ok()
                .is_none_or(|index| index >= fields.len())
        {
            return Err(validator.error(
                origin,
                "constructor field placement is not unique and contiguous",
            ));
        }
        let descriptor = validator
            .snapshot
            .field_descriptor(field.field)
            .ok_or_else(|| validator.error(origin, "constructor references a missing field"))?;
        if descriptor.owner != owner
            || descriptor.variant != variant
            || !expected_fields.contains(&field.field)
        {
            return Err(validator.error(origin, "constructor field has the wrong owner"));
        }
        if let CompileConstructorValue::EvaluatedDefault(body) = field.value
            && !validator
                .snapshot
                .evaluated_schema_defaults
                .contains_key(&body)
        {
            return Err(validator.error(
                origin,
                format!("constructor field references missing evaluated default {body:?}"),
            ));
        }
    }
    if seen_parameters.len() != fields.len() {
        return Err(validator.error(origin, "constructor parameter slots are incomplete"));
    }
    Ok(())
}

fn validate_pattern_constructors(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for ((function, pattern), target) in &validator.snapshot.pattern_constructors {
        let origin = validator.retained_origin(
            &validator.snapshot.origins.pattern_constructors,
            &(*function, *pattern),
        );
        validator.require_root(*function, origin, "pattern constructor placement")?;
        match target {
            CompilePatternConstructorTarget::Record {
                type_id,
                shape,
                fields,
            } => {
                let owner = validator.require_type(*type_id, origin, "record pattern")?;
                if owner.shape != Some(*shape) {
                    return Err(
                        validator.error(origin, "record pattern shape disagrees with its type")
                    );
                }
                validate_pattern_fields(validator, fields, &owner.fields, *type_id, None, origin)?;
            }
            CompilePatternConstructorTarget::Variant {
                type_id,
                variant,
                fields,
            } => {
                let owner = validator.require_type(*type_id, origin, "variant pattern")?;
                let variant_descriptor = validator
                    .snapshot
                    .variant_descriptor(*variant)
                    .ok_or_else(|| {
                        validator.error(origin, "variant pattern references a missing variant")
                    })?;
                if variant_descriptor.owner != *type_id || !owner.variants.contains(variant) {
                    return Err(validator.error(origin, "variant pattern owner is inconsistent"));
                }
                validate_pattern_fields(
                    validator,
                    fields,
                    &variant_descriptor.fields,
                    *type_id,
                    Some(*variant),
                    origin,
                )?;
            }
            CompilePatternConstructorTarget::DynamicRecord { type_name, fields } => {
                validate_dynamic_pattern(
                    validator,
                    type_name,
                    None,
                    fields,
                    origin,
                    "dynamic record pattern",
                )?;
            }
            CompilePatternConstructorTarget::DynamicVariant {
                owner_name,
                variant_name,
                fields,
            } => {
                validate_dynamic_pattern(
                    validator,
                    owner_name,
                    Some(variant_name.as_str()),
                    fields,
                    origin,
                    "dynamic variant pattern",
                )?;
            }
        }
    }
    Ok(())
}

fn validate_dynamic_pattern(
    validator: &SnapshotValidator<'_>,
    owner_name: &str,
    variant_name: Option<&str>,
    fields: &[String],
    origin: MirSourceOrigin,
    context: &str,
) -> Result<(), MirBuildError> {
    validate_dynamic_owner_and_variant(validator, owner_name, variant_name, origin, context)?;
    if fields.iter().any(String::is_empty) {
        return Err(validator.error(origin, format!("{context} has an empty field name")));
    }
    Ok(())
}

fn validate_dynamic_owner_and_variant(
    validator: &SnapshotValidator<'_>,
    owner_name: &str,
    variant_name: Option<&str>,
    origin: MirSourceOrigin,
    context: &str,
) -> Result<(), MirBuildError> {
    if owner_name.is_empty() {
        return Err(validator.error(origin, format!("{context} has an empty owner/type name")));
    }
    if variant_name.is_some_and(str::is_empty) {
        return Err(validator.error(origin, format!("{context} has an empty variant name")));
    }
    Ok(())
}

fn validate_pattern_fields(
    validator: &SnapshotValidator<'_>,
    fields: &[vela_def::FieldId],
    allowed: &[vela_def::FieldId],
    owner: vela_def::TypeId,
    variant: Option<vela_def::VariantId>,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    let mut unique = BTreeSet::new();
    for field in fields {
        let descriptor = validator
            .snapshot
            .field_descriptor(*field)
            .ok_or_else(|| validator.error(origin, "pattern references a missing field"))?;
        if !unique.insert(*field)
            || !allowed.contains(field)
            || descriptor.owner != owner
            || descriptor.variant != variant
        {
            return Err(validator.error(origin, "pattern field has an inconsistent owner"));
        }
    }
    Ok(())
}

fn validate_host_paths(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for ((function, expression), target) in &validator.snapshot.host_paths {
        let origin = validator.retained_origin(
            &validator.snapshot.origins.host_paths,
            &(*function, *expression),
        );
        validator.require_root(*function, origin, "host path placement")?;
        host::validate_path(validator, target, origin, "host path placement")?;
    }
    Ok(())
}
