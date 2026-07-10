use std::collections::{BTreeMap, BTreeSet};

use vela_def::GlobalId;

use crate::{
    CompileFunctionClass, CompileFunctionIdentity, CompileGuardKey, CompileMethodClass,
    CompileTypeClass, MethodExecutableTarget, MirBuildError, MirGuardLocation, MirSourceOrigin,
    MirTypeContract,
};

use super::SnapshotValidator;
use super::contracts::{validate_contract, validate_signature};

pub(super) fn validate(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    validate_types(validator)?;
    validate_variants(validator)?;
    validate_fields(validator)?;
    validate_functions(validator)?;
    validate_methods(validator)?;
    validate_globals(validator)?;
    validate_identity_indexes(validator)?;
    validate_roots(validator)?;
    validate_guards(validator)
}

fn validate_types(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for (type_id, descriptor) in validator.snapshot.target_table().types() {
        let origin =
            validator.retained_origin(&validator.snapshot.origins.type_descriptors, &type_id);
        if descriptor.id != type_id {
            return Err(validator.error(
                origin,
                format!(
                    "type table key #{} does not match descriptor #{}",
                    type_id.get(),
                    descriptor.id.get()
                ),
            ));
        }
        if validator
            .snapshot
            .types_by_name
            .get(&descriptor.canonical_name)
            != Some(&type_id)
        {
            return Err(validator.error(
                origin,
                format!(
                    "type #{} has no exact canonical-name index for {:?}",
                    type_id.get(),
                    descriptor.canonical_name
                ),
            ));
        }
        require_unique(
            validator,
            &descriptor.fields,
            origin,
            &format!("type #{} field list", type_id.get()),
        )?;
        require_unique(
            validator,
            &descriptor.variants,
            origin,
            &format!("type #{} variant list", type_id.get()),
        )?;
        match descriptor.class {
            CompileTypeClass::ScriptRecord
                if descriptor.shape.is_none() || !descriptor.variants.is_empty() =>
            {
                return Err(validator.error(
                    origin,
                    format!(
                        "script record type #{} has inconsistent layout",
                        type_id.get()
                    ),
                ));
            }
            CompileTypeClass::ScriptEnum
                if descriptor.shape.is_some() || !descriptor.fields.is_empty() =>
            {
                return Err(validator.error(
                    origin,
                    format!(
                        "script enum type #{} has inconsistent layout",
                        type_id.get()
                    ),
                ));
            }
            CompileTypeClass::OpaqueExternal
                if descriptor.shape.is_some()
                    || !descriptor.fields.is_empty()
                    || !descriptor.variants.is_empty() =>
            {
                return Err(validator.error(
                    origin,
                    format!(
                        "opaque external type #{} owns structural layout",
                        type_id.get()
                    ),
                ));
            }
            CompileTypeClass::Host { .. } if descriptor.shape.is_some() => {
                return Err(validator.error(
                    origin,
                    format!("host type #{} owns a script record shape", type_id.get()),
                ));
            }
            CompileTypeClass::ScriptRecord
            | CompileTypeClass::ScriptEnum
            | CompileTypeClass::OpaqueExternal
            | CompileTypeClass::Registry
            | CompileTypeClass::Standard
            | CompileTypeClass::Host { .. } => {}
        }
        let mut field_orders = BTreeSet::new();
        for field in &descriptor.fields {
            let field_descriptor =
                validator.snapshot.field_descriptor(*field).ok_or_else(|| {
                    validator.error(
                        origin,
                        format!(
                            "type #{} references missing field #{}",
                            type_id.get(),
                            field.get()
                        ),
                    )
                })?;
            if field_descriptor.owner != type_id || field_descriptor.variant.is_some() {
                return Err(validator.error(
                    origin,
                    format!(
                        "type #{} directly lists non-owned or variant field #{}",
                        type_id.get(),
                        field.get()
                    ),
                ));
            }
            if !field_orders.insert(field_descriptor.declaration_order) {
                return Err(validator.error(
                    origin,
                    format!(
                        "type #{} has duplicate field declaration order",
                        type_id.get()
                    ),
                ));
            }
        }
        let mut variant_orders = BTreeSet::new();
        for variant in &descriptor.variants {
            let variant_descriptor =
                validator
                    .snapshot
                    .variant_descriptor(*variant)
                    .ok_or_else(|| {
                        validator.error(
                            origin,
                            format!(
                                "type #{} references missing variant #{}",
                                type_id.get(),
                                variant.get()
                            ),
                        )
                    })?;
            if variant_descriptor.owner != type_id {
                return Err(validator.error(
                    origin,
                    format!(
                        "type #{} lists variant #{} owned by type #{}",
                        type_id.get(),
                        variant.get(),
                        variant_descriptor.owner.get()
                    ),
                ));
            }
            if !variant_orders.insert(variant_descriptor.declaration_order) {
                return Err(validator.error(
                    origin,
                    format!(
                        "type #{} has duplicate variant declaration order",
                        type_id.get()
                    ),
                ));
            }
        }
    }
    for (name, type_id) in &validator.snapshot.types_by_name {
        let descriptor = validator
            .snapshot
            .type_descriptor(*type_id)
            .ok_or_else(|| {
                let origin = validator
                    .retained_origin(&validator.snapshot.origins.type_descriptors, type_id);
                validator.error(
                    origin,
                    format!("canonical-name index {name:?} references missing type #{type_id:?}"),
                )
            })?;
        if descriptor.canonical_name != *name {
            let origin =
                validator.retained_origin(&validator.snapshot.origins.type_descriptors, type_id);
            return Err(validator.error(
                origin,
                format!("canonical-name index {name:?} disagrees with its type descriptor"),
            ));
        }
    }
    Ok(())
}

fn validate_variants(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for (variant, descriptor) in validator.snapshot.target_table().variants() {
        let origin =
            validator.retained_origin(&validator.snapshot.origins.variant_descriptors, &variant);
        if descriptor.id != variant {
            return Err(validator.error(origin, "variant table key does not match descriptor ID"));
        }
        let owner = validator.require_type(descriptor.owner, origin, "variant descriptor")?;
        if !owner.variants.contains(&variant) {
            return Err(validator.error(
                origin,
                format!(
                    "variant #{} is absent from owner type #{}",
                    variant.get(),
                    descriptor.owner.get()
                ),
            ));
        }
        require_unique(
            validator,
            &descriptor.fields,
            origin,
            &format!("variant #{} field list", variant.get()),
        )?;
        let mut orders = BTreeSet::new();
        for field in &descriptor.fields {
            let field_descriptor =
                validator.snapshot.field_descriptor(*field).ok_or_else(|| {
                    validator.error(
                        origin,
                        format!(
                            "variant #{} references missing field #{}",
                            variant.get(),
                            field.get()
                        ),
                    )
                })?;
            if field_descriptor.owner != descriptor.owner
                || field_descriptor.variant != Some(variant)
            {
                return Err(validator.error(
                    origin,
                    format!(
                        "variant #{} lists field #{} with a different owner",
                        variant.get(),
                        field.get()
                    ),
                ));
            }
            if !orders.insert(field_descriptor.declaration_order) {
                return Err(validator.error(
                    origin,
                    format!(
                        "variant #{} has duplicate field declaration order",
                        variant.get()
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn validate_fields(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for (field, descriptor) in validator.snapshot.target_table().fields() {
        let origin =
            validator.retained_origin(&validator.snapshot.origins.field_descriptors, &field);
        if descriptor.id != field {
            return Err(validator.error(origin, "field table key does not match descriptor ID"));
        }
        let owner = validator.require_type(descriptor.owner, origin, "field descriptor")?;
        match descriptor.variant {
            Some(variant) => {
                let variant_descriptor = validator
                    .snapshot
                    .variant_descriptor(variant)
                    .ok_or_else(|| {
                        validator.error(
                            origin,
                            format!(
                                "field #{} references missing variant #{}",
                                field.get(),
                                variant.get()
                            ),
                        )
                    })?;
                if variant_descriptor.owner != descriptor.owner
                    || !variant_descriptor.fields.contains(&field)
                {
                    return Err(validator.error(
                        origin,
                        format!(
                            "field #{} is absent from its variant descriptor",
                            field.get()
                        ),
                    ));
                }
            }
            None if !owner.fields.contains(&field) => {
                return Err(validator.error(
                    origin,
                    format!(
                        "field #{} is absent from its owner type descriptor",
                        field.get()
                    ),
                ));
            }
            None => {}
        }
        if let Some(contract) = &descriptor.contract {
            validate_contract(
                validator,
                contract,
                origin,
                &format!("field #{} contract", field.get()),
            )?;
        }
        match (owner.class, descriptor.host_runtime) {
            (CompileTypeClass::Host { .. }, _) | (_, None) => {}
            (_, Some(runtime)) => {
                return Err(validator.error(
                    origin,
                    format!(
                        "non-host field #{} carries runtime host field #{}",
                        field.get(),
                        runtime.get()
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn validate_functions(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for (function, descriptor) in validator.snapshot.target_table().functions() {
        let origin =
            validator.retained_origin(&validator.snapshot.origins.function_descriptors, &function);
        if descriptor.id != function {
            return Err(validator.error(origin, "function table key does not match descriptor ID"));
        }
        validate_signature(
            validator,
            &descriptor.signature,
            origin,
            &format!("function #{}", function.get()),
        )?;
        if descriptor.class == CompileFunctionClass::Script {
            let declaration_owned = validator
                .snapshot
                .functions_by_declaration
                .values()
                .any(|candidate| *candidate == function);
            let method_owned = validator
                .snapshot
                .target_table()
                .methods()
                .any(|(_, _, method)| {
                    matches!(
                        method.class,
                        CompileMethodClass::Script { executable, .. }
                            if executable.function == function
                    )
                });
            if !declaration_owned && !method_owned {
                return Err(validator.error(
                    origin,
                    format!(
                        "script function #{} has no declaration or method identity",
                        function.get()
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn validate_methods(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for (owner, method, descriptor) in validator.snapshot.target_table().methods() {
        let origin = validator.retained_origin(
            &validator.snapshot.origins.method_descriptors,
            &(owner, method),
        );
        if descriptor.owner != owner || descriptor.id != method {
            return Err(validator.error(origin, "method table key does not match descriptor ID"));
        }
        let owner_descriptor = validator.require_type(owner, origin, "method descriptor")?;
        validate_signature(
            validator,
            &descriptor.signature,
            origin,
            &format!("method #{} for type #{}", method.get(), owner.get()),
        )?;
        match &descriptor.class {
            CompileMethodClass::Script {
                executable,
                code_symbol,
                ..
            } => validate_script_method_descriptor(
                validator,
                descriptor,
                *executable,
                code_symbol,
                origin,
            )?,
            CompileMethodClass::Host { .. }
                if !matches!(owner_descriptor.class, CompileTypeClass::Host { .. }) =>
            {
                return Err(validator.error(
                    origin,
                    format!("host method #{} has a non-host owner", method.get()),
                ));
            }
            CompileMethodClass::Host { .. }
            | CompileMethodClass::Value
            | CompileMethodClass::Registry => {}
        }
    }
    Ok(())
}

fn validate_script_method_descriptor(
    validator: &SnapshotValidator<'_>,
    descriptor: &crate::CompileMethodDescriptor,
    executable: MethodExecutableTarget,
    code_symbol: &str,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    if executable.owner != descriptor.owner || executable.method != descriptor.id {
        return Err(validator.error(
            origin,
            format!(
                "script method #{} executable identity disagrees with its descriptor",
                descriptor.id.get()
            ),
        ));
    }
    let function = validator.require_script_function(
        executable.function,
        origin,
        "script method descriptor",
    )?;
    if function.canonical_symbol != code_symbol {
        return Err(validator.error(
            origin,
            format!(
                "script method #{} code symbol disagrees with function #{}",
                descriptor.id.get(),
                executable.function.get()
            ),
        ));
    }
    let Some(user_parameters) = function.signature.parameters.get(1..) else {
        return Err(validator.error(
            origin,
            format!(
                "script method function #{} has no receiver parameter",
                executable.function.get()
            ),
        ));
    };
    if user_parameters != descriptor.signature.parameters
        || function.signature.positional != descriptor.signature.positional
        || function.signature.return_contract != descriptor.signature.return_contract
        || function.signature.effect != descriptor.signature.effect
    {
        return Err(validator.error(
            origin,
            format!(
                "script method #{} signature disagrees with function #{}",
                descriptor.id.get(),
                executable.function.get()
            ),
        ));
    }
    let registered = validator
        .snapshot
        .methods_by_node
        .get(&executable.node)
        .is_some_and(|targets| targets.contains(&executable));
    if !registered {
        return Err(validator.error(
            origin,
            format!(
                "script method #{} executable is absent from the method-node index",
                descriptor.id.get()
            ),
        ));
    }
    Ok(())
}

fn validate_globals(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    let mut binding_counts = BTreeMap::<GlobalId, usize>::new();
    for (declaration, global) in &validator.snapshot.globals {
        let origin =
            validator.retained_origin(&validator.snapshot.origins.global_bindings, declaration);
        if validator.snapshot.global_by_id(*global).is_none() {
            return Err(validator.error(
                origin,
                format!(
                    "global declaration {declaration:?} references missing global #{}",
                    global.get()
                ),
            ));
        }
        *binding_counts.entry(*global).or_default() += 1;
    }
    for (global, descriptor) in validator.snapshot.target_table().globals() {
        let origin =
            validator.retained_origin(&validator.snapshot.origins.global_descriptors, &global);
        if descriptor.id != global || binding_counts.get(&global) != Some(&1) {
            return Err(validator.error(
                origin,
                format!(
                    "global #{} does not have one exact declaration binding",
                    global.get()
                ),
            ));
        }
        validate_contract(
            validator,
            &descriptor.contract,
            origin,
            &format!("global #{} contract", global.get()),
        )?;
    }
    Ok(())
}

fn validate_identity_indexes(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for (declaration, function) in &validator.snapshot.functions_by_declaration {
        let origin = validator.retained_origin(
            &validator.snapshot.origins.function_declarations,
            declaration,
        );
        validator.require_script_function(*function, origin, "function declaration index")?;
    }
    for (declaration, type_id) in &validator.snapshot.types_by_declaration {
        let origin =
            validator.retained_origin(&validator.snapshot.origins.type_declarations, declaration);
        let descriptor = validator.require_type(*type_id, origin, "type declaration index")?;
        if !matches!(
            descriptor.class,
            CompileTypeClass::ScriptRecord | CompileTypeClass::ScriptEnum
        ) {
            return Err(validator.error(origin, "type declaration index targets a non-script type"));
        }
    }
    for (node, targets) in &validator.snapshot.methods_by_node {
        let mut owners = BTreeSet::new();
        for target in targets {
            let origin =
                validator.retained_origin(&validator.snapshot.origins.method_targets, target);
            if target.node != *node || !owners.insert(target.owner) {
                return Err(
                    validator.error(origin, "method-node index contains an inconsistent target")
                );
            }
            validator.require_script_method(*target, origin, "method-node index")?;
        }
    }
    Ok(())
}

fn validate_roots(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for (function, target) in &validator.snapshot.functions {
        let origin = validator.retained_origin(&validator.snapshot.origins.roots, function);
        if target.identity.function() != *function {
            return Err(validator.error(origin, "compilation-root key disagrees with its identity"));
        }
        validator.require_script_function(*function, origin, "compilation root")?;
        match target.identity {
            CompileFunctionIdentity::Function(function) => {
                if !validator
                    .snapshot
                    .functions_by_declaration
                    .values()
                    .any(|candidate| *candidate == function)
                {
                    return Err(
                        validator.error(origin, "script function root has no declaration identity")
                    );
                }
            }
            CompileFunctionIdentity::Method(method) => {
                validator.require_script_method(method, origin, "script method root")?;
            }
        }
        let matches = validator
            .snapshot
            .functions_by_body
            .get(&target.body)
            .into_iter()
            .flatten()
            .filter(|candidate| **candidate == *function)
            .count();
        if matches != 1 {
            return Err(validator.error(origin, "compilation root has no exact body reverse index"));
        }
    }
    for (body, functions) in &validator.snapshot.functions_by_body {
        let mut unique = BTreeSet::new();
        for function in functions {
            let target = validator.snapshot.function(*function).ok_or_else(|| {
                let origin = validator.retained_origin(&validator.snapshot.origins.roots, function);
                validator.error(origin, "body reverse index references a missing root")
            })?;
            let origin = validator.retained_origin(&validator.snapshot.origins.roots, function);
            if target.body != *body || !unique.insert(*function) {
                return Err(validator.error(origin, "body reverse index disagrees with its root"));
            }
        }
    }
    Ok(())
}

fn validate_guards(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for (key, guard) in &validator.snapshot.guards {
        let origin = validator.retained_origin(&validator.snapshot.origins.guards, key);
        if matches!(guard.contract, MirTypeContract::Any) {
            return Err(validator.error(origin, format!("guard {key:?} redundantly checks Any")));
        }
        validate_contract(validator, &guard.contract, origin, "guard contract")?;
        validate_guard_context(validator, *key, guard, origin)?;
        let expected = match *key {
            CompileGuardKey::Expression { function, .. } => {
                validator.require_script_function(function, origin, "expression guard")?;
                None
            }
            CompileGuardKey::Parameter {
                function,
                parameter,
            } => {
                let function =
                    validator.require_script_function(function, origin, "parameter guard")?;
                function
                    .signature
                    .parameters
                    .get(parameter as usize)
                    .and_then(|parameter| parameter.contract.as_ref())
            }
            CompileGuardKey::Return(function) => validator
                .require_script_function(function, origin, "return guard")?
                .signature
                .return_contract
                .as_ref(),
            CompileGuardKey::Global(declaration) => validator
                .snapshot
                .global(declaration)
                .map(|global| &global.contract),
            CompileGuardKey::Field(field) => validator
                .snapshot
                .field_descriptor(field)
                .and_then(|field| field.contract.as_ref()),
        };
        if let Some(expected) = expected
            && expected != &guard.contract
        {
            return Err(validator.error(
                origin,
                format!("guard {key:?} disagrees with its owning contract"),
            ));
        }
        if !matches!(key, CompileGuardKey::Expression { .. }) && expected.is_none() {
            return Err(validator.error(origin, format!("guard {key:?} has no owning contract")));
        }
    }
    validate_required_guards(validator)
}

fn validate_guard_context(
    validator: &SnapshotValidator<'_>,
    key: CompileGuardKey,
    guard: &crate::CompileGuardTarget,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    if guard.context.debug_name.trim().is_empty() {
        return Err(validator.error(origin, format!("guard {key:?} has an empty debug name")));
    }
    let valid = match key {
        CompileGuardKey::Expression { .. } => matches!(
            guard.context.location,
            MirGuardLocation::Parameter { .. } | MirGuardLocation::Local | MirGuardLocation::Field
        ),
        CompileGuardKey::Parameter { parameter, .. } => {
            guard.context.location == MirGuardLocation::Parameter { index: parameter }
        }
        CompileGuardKey::Return(_) => guard.context.location == MirGuardLocation::Return,
        CompileGuardKey::Global(_) => guard.context.location == MirGuardLocation::Global,
        CompileGuardKey::Field(_) => guard.context.location == MirGuardLocation::Field,
    };
    if valid {
        Ok(())
    } else {
        Err(validator.error(
            origin,
            format!(
                "guard {key:?} has inconsistent boundary location {:?}",
                guard.context.location
            ),
        ))
    }
}

fn validate_required_guards(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    for (function, descriptor) in validator.snapshot.target_table().functions() {
        if descriptor.class != CompileFunctionClass::Script {
            continue;
        }
        let origin =
            validator.retained_origin(&validator.snapshot.origins.function_descriptors, &function);
        for (index, parameter) in descriptor.signature.parameters.iter().enumerate() {
            let Some(contract) = meaningful(parameter.contract.as_ref()) else {
                continue;
            };
            require_guard(
                validator,
                CompileGuardKey::Parameter {
                    function,
                    parameter: u32::try_from(index).map_err(|_| {
                        validator.error(origin, "function parameter index exceeds u32")
                    })?,
                },
                contract,
                origin,
            )?;
        }
        if let Some(contract) = meaningful(descriptor.signature.return_contract.as_ref()) {
            require_guard(
                validator,
                CompileGuardKey::Return(function),
                contract,
                origin,
            )?;
        }
    }
    for (field, descriptor) in validator.snapshot.target_table().fields() {
        let owner = validator
            .snapshot
            .type_descriptor(descriptor.owner)
            .expect("validated field owner");
        if !matches!(
            owner.class,
            CompileTypeClass::ScriptRecord | CompileTypeClass::ScriptEnum
        ) {
            continue;
        }
        let Some(contract) = meaningful(descriptor.contract.as_ref()) else {
            continue;
        };
        let origin =
            validator.retained_origin(&validator.snapshot.origins.field_descriptors, &field);
        require_guard(validator, CompileGuardKey::Field(field), contract, origin)?;
    }
    for (declaration, global) in &validator.snapshot.globals {
        let descriptor = validator
            .snapshot
            .global_by_id(*global)
            .expect("validated global binding");
        let Some(contract) = meaningful(Some(&descriptor.contract)) else {
            continue;
        };
        let origin =
            validator.retained_origin(&validator.snapshot.origins.global_bindings, declaration);
        require_guard(
            validator,
            CompileGuardKey::Global(*declaration),
            contract,
            origin,
        )?;
    }
    Ok(())
}

fn meaningful(contract: Option<&MirTypeContract>) -> Option<&MirTypeContract> {
    contract.filter(|contract| !matches!(contract, MirTypeContract::Any))
}

fn require_guard(
    validator: &SnapshotValidator<'_>,
    key: CompileGuardKey,
    contract: &MirTypeContract,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    match validator.snapshot.guard(key) {
        Some(guard) if &guard.contract == contract => Ok(()),
        Some(_) => Err(validator.error(
            origin,
            format!("required guard {key:?} has the wrong contract"),
        )),
        None => Err(validator.error(origin, format!("missing required guard {key:?}"))),
    }
}

fn require_unique<T: Copy + Ord + std::fmt::Debug>(
    validator: &SnapshotValidator<'_>,
    values: &[T],
    origin: MirSourceOrigin,
    context: &str,
) -> Result<(), MirBuildError> {
    let mut unique = BTreeSet::new();
    for value in values {
        if !unique.insert(*value) {
            return Err(validator.error(origin, format!("{context} contains duplicate {value:?}")));
        }
    }
    Ok(())
}
