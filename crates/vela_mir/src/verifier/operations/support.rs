use super::*;

pub(super) fn verify_abi(
    verifier: &FunctionVerifier<'_>,
    descriptor: &crate::CompileFunctionDescriptor,
) -> Result<(), MirVerifyError> {
    let function = verifier.function;
    if descriptor.canonical_symbol != function.code_symbol()
        || descriptor.signature.positional != CompilePositionalPolicy::ExactOrTrailingDefaults
        || descriptor.signature.parameters.len() != function.parameters().len()
        || descriptor.signature.return_contract.as_ref()
            != function.return_contract().map(|value| &value.contract)
    {
        return Err(bad_target(
            verifier,
            function.origin(),
            MirVerifyTarget::Function(descriptor.id),
            "function ABI disagrees with its descriptor",
        ));
    }
    for (parameter, expected) in function
        .parameters()
        .iter()
        .zip(&descriptor.signature.parameters)
    {
        let default = match expected.default {
            CompileParameterDefault::HirBody(body) => Some(body),
            CompileParameterDefault::Required | CompileParameterDefault::RuntimeProvided => None,
        };
        if parameter.name != expected.name
            || parameter.contract != expected.contract
            || parameter.default_body != default
        {
            return Err(bad_target(
                verifier,
                parameter.origin,
                MirVerifyTarget::Function(descriptor.id),
                "function parameter disagrees with its descriptor",
            ));
        }
    }
    Ok(())
}

pub(super) fn verify_origin(
    verifier: &FunctionVerifier<'_>,
    block: Option<crate::MirBlockId>,
    statement: Option<crate::MirStatementId>,
    origin: MirSourceOrigin,
) -> Result<(), MirVerifyError> {
    verify_origin_if(verifier, block, statement, origin, |body| {
        body == verifier.function.body()
            || verifier
                .function
                .parameters()
                .iter()
                .any(|parameter| parameter.default_body == Some(body))
    })
}

pub(super) fn verify_descendant_origin(
    verifier: &FunctionVerifier<'_>,
    origin: MirSourceOrigin,
) -> Result<(), MirVerifyError> {
    verify_origin_if(verifier, None, None, origin, |body| {
        function_owns_descendant_origin_body(verifier, body)
    })
}

fn verify_origin_if(
    verifier: &FunctionVerifier<'_>,
    block: Option<crate::MirBlockId>,
    statement: Option<crate::MirStatementId>,
    origin: MirSourceOrigin,
    allowed: impl FnOnce(vela_hir::ids::HirBodyId) -> bool,
) -> Result<(), MirVerifyError> {
    let structurally_valid = match origin.node {
        MirSourceNode::Declaration(_) => false,
        MirSourceNode::Body(body) => origin.body == Some(body),
        MirSourceNode::Expression(_) | MirSourceNode::Statement(_) | MirSourceNode::Pattern(_) => {
            origin.body.is_some()
        }
    };
    let allowed_body = origin.body.is_some_and(allowed);
    if structurally_valid && allowed_body && origin.span.start <= origin.span.end {
        Ok(())
    } else {
        Err(error(
            verifier,
            block,
            statement,
            origin,
            MirVerifyErrorKind::InvalidSourceOrigin(
                "origin is structurally invalid or outside the executable/default bodies"
                    .to_owned(),
            ),
        ))
    }
}

fn function_owns_descendant_origin_body(
    verifier: &FunctionVerifier<'_>,
    body: vela_hir::ids::HirBodyId,
) -> bool {
    verifier
        .program
        .functions()
        .any(|(candidate_id, candidate)| {
            let body_matches = candidate.body() == body
                || candidate
                    .parameters()
                    .iter()
                    .any(|parameter| parameter.default_body == Some(body));
            body_matches && is_self_or_descendant(verifier, candidate_id)
        })
}

fn is_self_or_descendant(
    verifier: &FunctionVerifier<'_>,
    mut candidate: crate::MirFunctionId,
) -> bool {
    let mut seen = BTreeSet::new();
    loop {
        if candidate == verifier.function_id {
            return true;
        }
        if !seen.insert(candidate) {
            return false;
        }
        let Some(function) = verifier.program.function(candidate) else {
            return false;
        };
        let crate::MirFunctionOwner::Lambda { parent, .. } = function.owner() else {
            return false;
        };
        candidate = *parent;
    }
}

pub(super) fn verify_value_type(
    verifier: &FunctionVerifier<'_>,
    origin: MirSourceOrigin,
    value: MirValueType,
) -> Result<(), MirVerifyError> {
    match value {
        MirValueType::ScriptType { type_id, shape } => {
            let descriptor = require_type(verifier, type_id, origin)?;
            if descriptor.shape != Some(shape) {
                return Err(bad_target(
                    verifier,
                    origin,
                    MirVerifyTarget::Type(type_id),
                    "MIR value shape disagrees with its type descriptor",
                ));
            }
        }
        MirValueType::Enum(type_id) => {
            require_type(verifier, type_id, origin)?;
        }
        MirValueType::Host(target) => verify_host_type(verifier, origin, target)?,
        MirValueType::Dynamic
        | MirValueType::Unit
        | MirValueType::Primitive(_)
        | MirValueType::Range
        | MirValueType::Iterator
        | MirValueType::Tuple(_)
        | MirValueType::Callable => {}
    }
    Ok(())
}

pub(super) fn verify_contract(
    verifier: &FunctionVerifier<'_>,
    origin: MirSourceOrigin,
    contract: &MirTypeContract,
) -> Result<(), MirVerifyError> {
    match contract {
        MirTypeContract::Array(value)
        | MirTypeContract::Set(value)
        | MirTypeContract::Iterator(value)
        | MirTypeContract::Option(value) => {
            if let Some(value) = value {
                verify_contract(verifier, origin, value)?;
            }
        }
        MirTypeContract::Map { key, value } => {
            if let Some(value) = key {
                verify_contract(verifier, origin, value)?;
            }
            if let Some(value) = value {
                verify_contract(verifier, origin, value)?;
            }
        }
        MirTypeContract::Tuple(values) => {
            for value in values.iter().flatten() {
                verify_contract(verifier, origin, value)?;
            }
        }
        MirTypeContract::Result { ok, err } => {
            if let Some(value) = ok {
                verify_contract(verifier, origin, value)?;
            }
            if let Some(value) = err {
                verify_contract(verifier, origin, value)?;
            }
        }
        MirTypeContract::Definition(type_id) => {
            require_type(verifier, *type_id, origin)?;
        }
        MirTypeContract::Shape { type_id, shape } => {
            let descriptor = require_type(verifier, *type_id, origin)?;
            if descriptor.shape != Some(*shape) {
                return Err(bad_target(
                    verifier,
                    origin,
                    MirVerifyTarget::Type(*type_id),
                    "shape contract disagrees with its type descriptor",
                ));
            }
        }
        MirTypeContract::Variant { type_id, variant } => {
            require_variant(verifier, *type_id, *variant, origin)?;
        }
        MirTypeContract::Host(target) => verify_host_type(verifier, origin, *target)?,
        MirTypeContract::Any
        | MirTypeContract::Primitive(_)
        | MirTypeContract::Range
        | MirTypeContract::Callable { .. } => {}
    }
    Ok(())
}

pub(super) fn rvalue_type(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    value: &MirRvalue,
) -> Result<MirValueType, MirVerifyError> {
    match value {
        MirRvalue::Use(value) => verifier.operand_type(value, block, Some(statement), origin),
        MirRvalue::Constant { value, .. } => Ok(value.value_type()),
        MirRvalue::Truthy { .. } | MirRvalue::IsMissing { .. } | MirRvalue::PatternPredicate(_) => {
            Ok(MirValueType::Primitive(PrimitiveTag::Bool))
        }
    }
}

pub(super) fn place_type(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    place: MirPlace,
) -> Result<MirValueType, MirVerifyError> {
    match place {
        MirPlace::Local(id) => verifier
            .function
            .local(id)
            .map(|value| value.value_type)
            .ok_or_else(|| {
                error(
                    verifier,
                    Some(block),
                    Some(statement),
                    origin,
                    MirVerifyErrorKind::MissingLocal(id),
                )
            }),
        MirPlace::Temp(id) => verifier
            .function
            .temp(id)
            .map(|value| value.value_type)
            .ok_or_else(|| {
                error(
                    verifier,
                    Some(block),
                    Some(statement),
                    origin,
                    MirVerifyErrorKind::MissingTemp(id),
                )
            }),
    }
}

pub(super) fn destination_accepts(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    destination: Option<MirValueType>,
    result: MirValueType,
) -> Result<(), MirVerifyError> {
    let Some(destination) = destination else {
        return Ok(());
    };
    if compatible(destination, result) {
        Ok(())
    } else {
        Err(type_error(
            verifier,
            block,
            Some(statement),
            origin,
            "statement destination",
            destination,
        ))
    }
}

pub(super) fn destination_contract(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    destination: Option<MirValueType>,
    contract: Option<&MirTypeContract>,
) -> Result<(), MirVerifyError> {
    let Some(actual) = destination else {
        return Ok(());
    };
    // An absent runtime contract means analysis may retain any proven result
    // type; it does not mean the result must be erased back to `Dynamic`.
    let valid = contract.is_none_or(|contract| satisfies_contract(actual, contract));
    if valid {
        Ok(())
    } else {
        Err(type_error(
            verifier,
            block,
            Some(statement),
            origin,
            "effectful operation destination",
            actual,
        ))
    }
}

pub(super) fn operand_is(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: Option<crate::MirStatementId>,
    origin: MirSourceOrigin,
    operand: &MirOperand,
    expected: MirValueType,
) -> Result<(), MirVerifyError> {
    let actual = verifier.operand_type(operand, block, statement, origin)?;
    if actual == expected || actual == MirValueType::Dynamic {
        Ok(())
    } else {
        Err(type_error(
            verifier,
            block,
            statement,
            origin,
            "typed operation operand",
            actual,
        ))
    }
}

pub(super) fn switch_case(
    verifier: &FunctionVerifier<'_>,
    origin: MirSourceOrigin,
    actual: MirValueType,
    case: &MirSwitchValue,
) -> Result<(), MirVerifyError> {
    let compatible = match case {
        MirSwitchValue::Bool(_) => {
            matches!(
                actual,
                MirValueType::Dynamic | MirValueType::Primitive(PrimitiveTag::Bool)
            )
        }
        MirSwitchValue::Char(_) => {
            matches!(
                actual,
                MirValueType::Dynamic | MirValueType::Primitive(PrimitiveTag::Char)
            )
        }
        MirSwitchValue::Signed(_) => {
            matches!(actual, MirValueType::Dynamic)
                || matches!(actual, MirValueType::Primitive(tag) if tag.numeric_tag().is_some_and(|tag| tag.is_signed_integer()))
        }
        MirSwitchValue::Unsigned(_) => {
            matches!(actual, MirValueType::Dynamic)
                || matches!(actual, MirValueType::Primitive(tag) if tag.numeric_tag().is_some_and(|tag| tag.is_unsigned_integer()))
        }
    };
    if compatible {
        Ok(())
    } else {
        Err(error(
            verifier,
            None,
            None,
            origin,
            MirVerifyErrorKind::InvalidTerminatorContract(
                "switch case is incompatible with its proven discriminant type".to_owned(),
            ),
        ))
    }
}

pub(super) fn function_call_target(
    verifier: &FunctionVerifier<'_>,
    origin: MirSourceOrigin,
    function: vela_def::FunctionId,
    debug_name: &str,
    signature: &crate::CompileSignature,
    class: CompileFunctionClass,
) -> Result<(), MirVerifyError> {
    let descriptor = function_target(verifier, function, origin)?;
    let class_matches = descriptor.class == class
        || (class == CompileFunctionClass::Native
            && descriptor.class == CompileFunctionClass::Registry);
    if class_matches
        && (descriptor.debug_name == debug_name || descriptor.canonical_symbol == debug_name)
        && descriptor.signature == *signature
    {
        Ok(())
    } else {
        Err(bad_target(
            verifier,
            origin,
            MirVerifyTarget::Function(function),
            "call identity, class, name, or signature disagrees",
        ))
    }
}

pub(super) fn function_target<'a>(
    verifier: &'a FunctionVerifier<'_>,
    id: vela_def::FunctionId,
    origin: MirSourceOrigin,
) -> Result<&'a crate::CompileFunctionDescriptor, MirVerifyError> {
    verifier
        .program
        .targets()
        .function(id)
        .ok_or_else(|| missing_target(verifier, origin, MirVerifyTarget::Function(id)))
}

pub(super) fn method_target<'a>(
    verifier: &'a FunctionVerifier<'_>,
    owner: vela_def::TypeId,
    method: vela_def::MethodId,
    origin: MirSourceOrigin,
) -> Result<&'a crate::CompileMethodDescriptor, MirVerifyError> {
    verifier
        .program
        .targets()
        .method(owner, method)
        .ok_or_else(|| missing_target(verifier, origin, MirVerifyTarget::Method { owner, method }))
}

pub(super) fn require_type<'a>(
    verifier: &'a FunctionVerifier<'_>,
    id: vela_def::TypeId,
    origin: MirSourceOrigin,
) -> Result<&'a crate::CompileTypeDescriptor, MirVerifyError> {
    verifier
        .program
        .targets()
        .type_descriptor(id)
        .ok_or_else(|| missing_target(verifier, origin, MirVerifyTarget::Type(id)))
}

pub(super) fn require_variant(
    verifier: &FunctionVerifier<'_>,
    owner: vela_def::TypeId,
    id: vela_def::VariantId,
    origin: MirSourceOrigin,
) -> Result<(), MirVerifyError> {
    let variant = verifier
        .program
        .targets()
        .variant(id)
        .ok_or_else(|| missing_target(verifier, origin, MirVerifyTarget::Variant(id)))?;
    if variant.owner == owner {
        Ok(())
    } else {
        Err(bad_target(
            verifier,
            origin,
            MirVerifyTarget::Variant(id),
            "variant owner disagrees",
        ))
    }
}

pub(super) fn require_field<'a>(
    verifier: &'a FunctionVerifier<'_>,
    id: vela_def::FieldId,
    owner: vela_def::TypeId,
    variant: Option<vela_def::VariantId>,
    origin: MirSourceOrigin,
) -> Result<&'a crate::CompileFieldDescriptor, MirVerifyError> {
    let field = verifier
        .program
        .targets()
        .field(id)
        .ok_or_else(|| missing_target(verifier, origin, MirVerifyTarget::Field(id)))?;
    if field.owner == owner && field.variant == variant {
        Ok(field)
    } else {
        Err(bad_target(
            verifier,
            origin,
            MirVerifyTarget::Field(id),
            "field owner or variant disagrees",
        ))
    }
}

pub(super) fn require_local<'a>(
    verifier: &'a FunctionVerifier<'_>,
    block: crate::MirBlockId,
    origin: MirSourceOrigin,
    id: crate::MirLocalId,
) -> Result<&'a crate::MirLocal, MirVerifyError> {
    verifier.function.local(id).ok_or_else(|| {
        error(
            verifier,
            Some(block),
            None,
            origin,
            MirVerifyErrorKind::MissingLocal(id),
        )
    })
}

pub(super) fn arity_accepts(signature: &crate::CompileSignature, provided: usize) -> bool {
    match signature.positional {
        CompilePositionalPolicy::RuntimeChecked => true,
        CompilePositionalPolicy::Variadic { minimum } => provided >= minimum as usize,
        CompilePositionalPolicy::ExactOrTrailingDefaults => {
            provided <= signature.parameters.len()
                && !signature.parameters.iter().any(|parameter| {
                    matches!(parameter.default, CompileParameterDefault::HirBody(_))
                })
                && signature.parameters[provided..].iter().all(|parameter| {
                    matches!(parameter.default, CompileParameterDefault::RuntimeProvided)
                })
        }
    }
}

pub(super) fn operand_value_type(
    verifier: &FunctionVerifier<'_>,
    operand: &MirOperand,
) -> Result<MirValueType, MirVerifyError> {
    match operand {
        MirOperand::Immediate(value) => Ok(immediate_type(*value)),
        MirOperand::Local(id) => verifier
            .function
            .local(*id)
            .map(|value| value.value_type)
            .ok_or_else(|| {
                missing_target(
                    verifier,
                    verifier.function.origin(),
                    MirVerifyTarget::MirFunction(verifier.function_id),
                )
            }),
        MirOperand::Temp(id) => verifier
            .function
            .temp(*id)
            .map(|value| value.value_type)
            .ok_or_else(|| {
                missing_target(
                    verifier,
                    verifier.function.origin(),
                    MirVerifyTarget::MirFunction(verifier.function_id),
                )
            }),
    }
}

pub(super) fn immediate_type(value: MirImmediate) -> MirValueType {
    value.value_type()
}

pub(super) fn constant_type(value: &crate::MirEvaluatedConstant) -> Option<MirValueType> {
    match value {
        crate::MirEvaluatedConstant::Unit => Some(MirValueType::Unit),
        crate::MirEvaluatedConstant::Bool(_) => Some(MirValueType::Primitive(PrimitiveTag::Bool)),
        crate::MirEvaluatedConstant::Char(_) => Some(MirValueType::Primitive(PrimitiveTag::Char)),
        crate::MirEvaluatedConstant::Scalar(value) => {
            Some(MirValueType::Primitive(value.primitive_tag()))
        }
        crate::MirEvaluatedConstant::String(_) => {
            Some(MirValueType::Primitive(PrimitiveTag::String))
        }
        crate::MirEvaluatedConstant::Bytes(_) => Some(MirValueType::Primitive(PrimitiveTag::Bytes)),
        crate::MirEvaluatedConstant::Array(_) | crate::MirEvaluatedConstant::Map(_) => None,
    }
}

pub(super) fn satisfies_contract(value: MirValueType, contract: &MirTypeContract) -> bool {
    if value == MirValueType::Dynamic || matches!(contract, MirTypeContract::Any) {
        return true;
    }
    match contract {
        MirTypeContract::Primitive(tag) => {
            value == MirValueType::Primitive(*tag)
                || (*tag == PrimitiveTag::Unit && value == MirValueType::Unit)
        }
        MirTypeContract::Range => value == MirValueType::Range,
        MirTypeContract::Iterator(_) => value == MirValueType::Iterator,
        MirTypeContract::Tuple(values) => value == MirValueType::Tuple(values.len() as u32),
        MirTypeContract::Callable { .. } => value == MirValueType::Callable,
        MirTypeContract::Definition(type_id) | MirTypeContract::Shape { type_id, .. } => {
            matches!(value, MirValueType::ScriptType { type_id: id, .. } | MirValueType::Enum(id) if id == *type_id)
        }
        MirTypeContract::Variant { type_id, .. } => value == MirValueType::Enum(*type_id),
        MirTypeContract::Host(target) => value == MirValueType::Host(*target),
        MirTypeContract::Option(_) | MirTypeContract::Result { .. } => {
            matches!(value, MirValueType::Enum(_))
        }
        MirTypeContract::Any
        | MirTypeContract::Array(_)
        | MirTypeContract::Map { .. }
        | MirTypeContract::Set(_) => false,
    }
}

pub(super) fn compatible(left: MirValueType, right: MirValueType) -> bool {
    left == MirValueType::Dynamic || right == MirValueType::Dynamic || left == right
}

pub(super) fn integer_or_dynamic(value: MirValueType) -> bool {
    value == MirValueType::Dynamic
        || matches!(value, MirValueType::Primitive(tag) if tag.numeric_tag().is_some_and(|tag| tag.is_integer()))
}

pub(super) fn type_error(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: Option<crate::MirStatementId>,
    origin: MirSourceOrigin,
    role: &'static str,
    actual: MirValueType,
) -> MirVerifyError {
    error(
        verifier,
        Some(block),
        statement,
        origin,
        MirVerifyErrorKind::InvalidOperandType {
            role,
            expected: "the operation's proven MIR type contract".to_owned(),
            actual,
        },
    )
}

pub(super) fn missing_target(
    verifier: &FunctionVerifier<'_>,
    origin: MirSourceOrigin,
    target: MirVerifyTarget,
) -> MirVerifyError {
    error(
        verifier,
        None,
        None,
        origin,
        MirVerifyErrorKind::MissingTarget(target),
    )
}

pub(super) fn bad_target(
    verifier: &FunctionVerifier<'_>,
    origin: MirSourceOrigin,
    target: MirVerifyTarget,
    detail: &str,
) -> MirVerifyError {
    error(
        verifier,
        None,
        None,
        origin,
        MirVerifyErrorKind::InconsistentTarget {
            target,
            detail: detail.to_owned(),
        },
    )
}

pub(super) fn error(
    verifier: &FunctionVerifier<'_>,
    block: Option<crate::MirBlockId>,
    statement: Option<crate::MirStatementId>,
    origin: MirSourceOrigin,
    kind: MirVerifyErrorKind,
) -> MirVerifyError {
    verifier.error(block, statement, origin, kind)
}
