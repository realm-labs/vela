use super::{
    CompileFunctionClass, CompileMethodClass, FunctionVerifier, MirCall, MirSourceOrigin,
    MirValueType, MirVerifyError, MirVerifyTarget, bad_target, destination_contract,
    function_call_target, function_target, method_target, require_type,
    verify_call_argument_contracts,
};

pub(super) fn verify_call(
    verifier: &FunctionVerifier<'_>,
    block: crate::MirBlockId,
    statement: crate::MirStatementId,
    origin: MirSourceOrigin,
    call: &MirCall,
    destination: Option<MirValueType>,
) -> Result<(), MirVerifyError> {
    let return_contract = match call {
        MirCall::ScriptFunction {
            function,
            debug_name,
            signature,
            ..
        } => function_call_target(
            verifier,
            origin,
            *function,
            debug_name,
            signature,
            CompileFunctionClass::Script,
        )
        .map(|_| signature.return_contract.as_ref())?,
        MirCall::NativeFunction {
            function,
            debug_name,
            signature,
            ..
        } => function_call_target(
            verifier,
            origin,
            *function,
            debug_name,
            signature,
            CompileFunctionClass::Native,
        )
        .map(|_| signature.return_contract.as_ref())?,
        MirCall::StdlibFunction {
            function,
            debug_name,
            signature,
            ..
        } => function_call_target(
            verifier,
            origin,
            *function,
            debug_name,
            signature,
            CompileFunctionClass::Stdlib,
        )
        .map(|_| signature.return_contract.as_ref())?,
        MirCall::ScriptMethod {
            target,
            debug_name,
            signature,
            ..
        } => {
            let method = method_target(verifier, target.owner, target.method, origin)?;
            if (method.debug_name != *debug_name && method.member_name != *debug_name)
                || method.signature != *signature
                || !matches!(&method.class, CompileMethodClass::Script { executable, .. } if executable == target)
            {
                return Err(bad_target(
                    verifier,
                    origin,
                    MirVerifyTarget::Method {
                        owner: target.owner,
                        method: target.method,
                    },
                    "script method call disagrees with its descriptor",
                ));
            }
            function_target(verifier, target.function, origin)?;
            signature.return_contract.as_ref()
        }
        MirCall::ValueMethod {
            owner,
            method,
            debug_name,
            signature,
            ..
        } => {
            require_type(verifier, *owner, origin)?;
            let descriptor = method_target(verifier, *owner, *method, origin)?;
            if !matches!(
                descriptor.class,
                CompileMethodClass::Value | CompileMethodClass::Registry
            ) || (descriptor.debug_name != *debug_name && descriptor.member_name != *debug_name)
                || descriptor.signature != *signature
            {
                return Err(bad_target(
                    verifier,
                    origin,
                    MirVerifyTarget::Method {
                        owner: *owner,
                        method: *method,
                    },
                    "value-method call disagrees with its descriptor",
                ));
            }
            signature.return_contract.as_ref()
        }
        MirCall::CallableValue { .. }
        | MirCall::DynamicCallable { .. }
        | MirCall::DynamicMethod { .. } => None,
    };
    verify_call_argument_contracts(verifier, block, statement, origin, call)?;
    destination_contract(
        verifier,
        block,
        statement,
        origin,
        destination,
        return_contract,
    )
}
