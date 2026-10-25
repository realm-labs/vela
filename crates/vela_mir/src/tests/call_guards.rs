use vela_common::{PrimitiveTag, SourceId, Span};
use vela_def::FunctionId;
use vela_hir::ids::HirBodyId;

use crate::*;

#[test]
fn script_function_calls_own_parameter_guard_policy() {
    let body = HirBodyId::new(600);
    let origin = MirSourceOrigin::body(body, Span::new(SourceId::new(60), 4, 18));
    let mut function = MirFunction::new(
        body,
        MirFunctionOwner::Function(FunctionId::new(601)),
        "guard_policy",
        None,
        origin,
    );
    let entry = function.entry_block();
    let signature = CompileSignature {
        parameters: vec![CompileParameter {
            name: "value".to_owned(),
            contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
            default: CompileParameterDefault::HirBody(HirBodyId::new(602)),
            origin: Some(origin),
        }],
        positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
        return_contract: None,
        effect: MirEffect::PURE,
    };

    let missing_result = function.add_temp(MirValueType::Dynamic, origin);
    let missing_safepoint = function.add_safepoint(MirSafepoint::new(origin));
    let missing_call = |parameter_guards| {
        MirStatement::new(
            origin,
            Some(MirPlace::temp(missing_result)),
            MirStatementKind::Call(MirCall::ScriptFunction {
                function: FunctionId::new(603),
                debug_name: "typed_default".to_owned(),
                signature: signature.clone(),
                arguments: vec![MirScriptArgument::missing(0)],
                parameter_guards,
            }),
            MirEffect::script_call(),
            Some(missing_safepoint),
        )
    };
    assert_eq!(
        function.append_statement(
            entry,
            missing_call(MirScriptParameterGuardMode::ProvenAtCallSite),
        ),
        Err(MirBuildError::InvalidCallArgumentPlacement { origin })
    );
    function
        .append_statement(
            entry,
            missing_call(MirScriptParameterGuardMode::CheckCalleeParameterContracts),
        )
        .expect("a missing typed default must retain callee parameter guards");

    let proven_result = function.add_temp(MirValueType::Dynamic, origin);
    let proven_safepoint = function.add_safepoint(MirSafepoint::new(origin));
    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(proven_result)),
                MirStatementKind::Call(MirCall::ScriptFunction {
                    function: FunctionId::new(603),
                    debug_name: "typed_default".to_owned(),
                    signature,
                    arguments: vec![MirScriptArgument::placed(
                        0,
                        MirOperand::Immediate(MirImmediate::Scalar(vela_common::ScalarValue::I64(
                            9,
                        ))),
                    )],
                    parameter_guards: MirScriptParameterGuardMode::ProvenAtCallSite,
                }),
                MirEffect::script_call(),
                Some(proven_safepoint),
            ),
        )
        .expect("a proven typed argument may skip callee parameter guards");

    let mut program = MirProgram::new(MirTargetTable::default());
    program
        .add_function(function)
        .expect("guard-policy fixture has a unique function identity");
    let dump = program.dump();
    assert!(dump.contains("parameter_guards=CheckCalleeParameterContracts"));
    assert!(dump.contains("parameter_guards=ProvenAtCallSite"));
}
