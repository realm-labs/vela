use super::*;

use vela_common::{HostTypeId, ShapeId};
use vela_def::{FieldId, MethodId};

use crate::{
    CompileFieldAccess, CompileFieldDescriptor, CompileFunctionAccess, CompileMethodAccess,
    CompileMethodClass, CompileMethodDescriptor, CompileTypeClass, CompileTypeDescriptor,
    HostFieldTarget, MirAggregate, MirCall, MirGuard, MirGuardAssumption, MirGuardContext,
    MirGuardLocation, MirHostPathSegment, MirTypeContract,
};

fn finish(function: &mut MirFunction) {
    function
        .set_terminator(
            function.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(None),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("return terminator");
}

fn allocate_statement(
    function: &mut MirFunction,
    destination: MirLocalId,
    aggregate: MirAggregate,
) {
    let safepoint = function.add_safepoint(MirSafepoint::new(origin()));
    function
        .append_statement(
            function.entry_block(),
            MirStatement::new(
                origin(),
                Some(MirPlace::local(destination)),
                MirStatementKind::Allocate(aggregate),
                MirEffect::allocation(),
                Some(safepoint),
            ),
        )
        .expect("aggregate statement");
}

#[test]
fn mir_verifier_accepts_registry_native_value_method_and_reflection_targets() {
    let registry_function = FunctionId::new(7_200);
    let owner = TypeId::new(7_201);
    let method = MethodId::new(7_202);
    let registry_signature = CompileSignature {
        parameters: Vec::new(),
        positional: CompilePositionalPolicy::RuntimeChecked,
        return_contract: None,
        effect: MirEffect::PURE,
    };
    let mut table = target_table();
    assert!(table.insert_function(CompileFunctionDescriptor {
        id: registry_function,
        class: CompileFunctionClass::Registry,
        canonical_symbol: "registry::dispatch".to_owned(),
        debug_name: "dispatch".to_owned(),
        signature: registry_signature.clone(),
        access: CompileFunctionAccess::new(true, true, true),
    }));
    assert!(table.insert_type(CompileTypeDescriptor {
        id: owner,
        canonical_name: "registry::Value".to_owned(),
        class: CompileTypeClass::Registry,
        shape: None,
        fields: Vec::new(),
        variants: Vec::new(),
    }));
    assert!(table.insert_method(CompileMethodDescriptor {
        id: method,
        owner,
        member_name: "dispatch".to_owned(),
        debug_name: "Value::dispatch".to_owned(),
        class: CompileMethodClass::Registry,
        signature: registry_signature.clone(),
        access: CompileMethodAccess::new(true, true, Vec::new()),
    }));

    let mut function = function();
    for statement in [
        MirStatementKind::Call(MirCall::NativeFunction {
            function: registry_function,
            debug_name: "dispatch".to_owned(),
            signature: registry_signature.clone(),
            arguments: Vec::new(),
        }),
        MirStatementKind::Call(MirCall::ValueMethod {
            owner,
            method,
            debug_name: "Value::dispatch".to_owned(),
            receiver: scalar(1),
            signature: registry_signature.clone(),
            arguments: Vec::new(),
        }),
        MirStatementKind::Reflect(MirReflectionOperation::Read {
            function: registry_function,
            target: scalar(1),
            member: scalar(2),
        }),
    ] {
        let destination = function.add_synthetic_local(MirValueType::Dynamic, origin());
        let safepoint = function.add_safepoint(MirSafepoint::new(origin()));
        let effect = match &statement {
            MirStatementKind::Reflect(_) => MirEffect::reflection_read(),
            MirStatementKind::Call(_) => MirEffect::external_call(),
            _ => unreachable!(),
        };
        function
            .append_statement(
                function.entry_block(),
                MirStatement::new(
                    origin(),
                    Some(MirPlace::local(destination)),
                    statement,
                    effect,
                    Some(safepoint),
                ),
            )
            .expect("registry operation");
    }
    finish(&mut function);
    let mut program = crate::MirProgram::new(table);
    program.add_function(function).expect("MIR function");
    verify_mir(&program).expect("registry-backed operation forms verify");
}

#[test]
fn mir_verifier_rejects_incomplete_duplicate_and_mistyped_aggregates() {
    let record = TypeId::new(7_300);
    let first = FieldId::new(7_301);
    let second = FieldId::new(7_302);
    let shape = ShapeId::new(7_303);
    let mut table = target_table();
    assert!(table.insert_type(CompileTypeDescriptor {
        id: record,
        canonical_name: "verifier::Record".to_owned(),
        class: CompileTypeClass::ScriptRecord,
        shape: Some(shape),
        fields: vec![first, second],
        variants: Vec::new(),
    }));
    for (order, field) in [first, second].into_iter().enumerate() {
        assert!(table.insert_field(CompileFieldDescriptor {
            id: field,
            owner: record,
            variant: None,
            name: format!("field_{order}"),
            contract: None,
            declaration_order: order as u32,
            access: CompileFieldAccess::script(),
            host_runtime: None,
        }));
    }
    let mut record_function = function();
    let destination = record_function.add_synthetic_local(
        MirValueType::ScriptType {
            type_id: record,
            shape,
        },
        origin(),
    );
    allocate_statement(
        &mut record_function,
        destination,
        MirAggregate::Record {
            type_id: record,
            shape,
            fields: vec![(first, scalar(1))],
        },
    );
    finish(&mut record_function);
    let mut record_program = crate::MirProgram::new(table);
    record_program
        .add_function(record_function)
        .expect("MIR function");
    assert!(matches!(
        verify_error(&record_program).into_kind(),
        MirVerifyErrorKind::InconsistentTarget { .. }
    ));

    let mut dynamic = function();
    let destination = dynamic.add_synthetic_local(MirValueType::Dynamic, origin());
    allocate_statement(
        &mut dynamic,
        destination,
        MirAggregate::DynamicRecord {
            type_name: "Loose".to_owned(),
            fields: vec![
                ("same".to_owned(), scalar(1)),
                ("same".to_owned(), scalar(2)),
            ],
        },
    );
    finish(&mut dynamic);
    assert!(matches!(
        verify_error(&program(dynamic)).into_kind(),
        MirVerifyErrorKind::InconsistentTarget { .. }
    ));

    let mut tuple = function();
    let destination =
        tuple.add_synthetic_local(MirValueType::Primitive(PrimitiveTag::I64), origin());
    allocate_statement(
        &mut tuple,
        destination,
        MirAggregate::Tuple(vec![scalar(1)]),
    );
    finish(&mut tuple);
    assert!(matches!(
        verify_error(&program(tuple)).into_kind(),
        MirVerifyErrorKind::InvalidOperandType { .. }
    ));
}

#[test]
fn mir_verifier_rejects_guard_context_operand_and_recoverability_mismatches() {
    let mut trap = function();
    let guard = trap.add_guard(MirGuard {
        assumption: MirGuardAssumption::Type(MirTypeContract::Primitive(PrimitiveTag::Bool)),
        context: None,
        origin: origin(),
    });
    trap.append_statement(
        trap.entry_block(),
        MirStatement::new(
            origin(),
            None,
            MirStatementKind::GuardTrap {
                value: MirOperand::Immediate(MirImmediate::Bool(true)),
                guard,
            },
            MirEffect::may_trap(),
            None,
        ),
    )
    .expect("guard trap");
    finish(&mut trap);
    assert!(matches!(
        verify_error(&program(trap)).into_kind(),
        MirVerifyErrorKind::InvalidTerminatorContract(_)
    ));

    let mut branch = function();
    let guard = branch.add_guard(MirGuard {
        assumption: MirGuardAssumption::TupleArity { arity: 1 },
        context: Some(MirGuardContext::new(MirGuardLocation::Local, "tuple")),
        origin: origin(),
    });
    let passed = branch.add_block();
    let slow = branch.add_block();
    branch
        .set_terminator(
            branch.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::GuardBranch {
                    value: MirOperand::Immediate(MirImmediate::Unit),
                    guard,
                    passed,
                    slow,
                },
                MirEffect::PURE,
                None,
            ),
        )
        .expect("guard branch");
    for block in [passed, slow] {
        branch
            .set_terminator(
                block,
                MirTerminator::new(
                    origin(),
                    MirTerminatorKind::Return(None),
                    MirEffect::PURE,
                    None,
                ),
            )
            .expect("return");
    }
    assert!(matches!(
        verify_error(&program(branch)).into_kind(),
        MirVerifyErrorKind::InvalidTerminatorContract(_)
    ));

    let mut arity = function();
    let tuple = arity.add_synthetic_local(MirValueType::Tuple(1), origin());
    let guard = arity.add_guard(MirGuard {
        assumption: MirGuardAssumption::TupleArity { arity: 2 },
        context: None,
        origin: origin(),
    });
    arity
        .append_statement(
            arity.entry_block(),
            MirStatement::new(
                origin(),
                None,
                MirStatementKind::GuardTrap {
                    value: MirOperand::Local(tuple),
                    guard,
                },
                MirEffect::may_trap(),
                None,
            ),
        )
        .expect("tuple guard trap");
    finish(&mut arity);
    assert!(matches!(
        verify_error(&program(arity)).into_kind(),
        MirVerifyErrorKind::InvalidTerminatorContract(_)
    ));

    let mut tuple_context = function();
    let tuple = tuple_context.add_synthetic_local(MirValueType::Tuple(2), origin());
    allocate_statement(
        &mut tuple_context,
        tuple,
        MirAggregate::Tuple(vec![scalar(1), scalar(2)]),
    );
    let guard = tuple_context.add_guard(MirGuard {
        assumption: MirGuardAssumption::TupleArity { arity: 2 },
        context: Some(MirGuardContext::new(MirGuardLocation::Local, "tuple")),
        origin: origin(),
    });
    tuple_context
        .append_statement(
            tuple_context.entry_block(),
            MirStatement::new(
                origin(),
                None,
                MirStatementKind::GuardTrap {
                    value: MirOperand::Local(tuple),
                    guard,
                },
                MirEffect::may_trap(),
                None,
            ),
        )
        .expect("tuple guard trap");
    finish(&mut tuple_context);
    assert!(matches!(
        verify_error(&program(tuple_context)).into_kind(),
        MirVerifyErrorKind::InvalidTerminatorContract(_)
    ));
}

#[test]
fn mir_verifier_rejects_host_write_permission_bypass() {
    let semantic = TypeId::new(7_400);
    let runtime = HostTypeId::new(7_401);
    let field = FieldId::new(7_402);
    let access = CompileFieldAccess::new(true, false, true, false, Vec::new());
    let host_type = HostTypeTarget { semantic, runtime };
    let mut table = target_table();
    assert!(table.insert_type(CompileTypeDescriptor {
        id: semantic,
        canonical_name: "host::Readonly".to_owned(),
        class: CompileTypeClass::Host { runtime },
        shape: None,
        fields: vec![field],
        variants: Vec::new(),
    }));
    assert!(table.insert_field(CompileFieldDescriptor {
        id: field,
        owner: semantic,
        variant: None,
        name: "value".to_owned(),
        contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
        declaration_order: 0,
        access: access.clone(),
        host_runtime: Some(field),
    }));

    let mut function = function();
    let root = function.add_synthetic_local(MirValueType::Host(host_type), origin());
    function
        .append_statement(
            function.entry_block(),
            MirStatement::new(
                origin(),
                None,
                MirStatementKind::Host(MirHostOperation::Write {
                    root: MirOperand::Local(root),
                    path: MirHostPath {
                        root_type: host_type,
                        segments: vec![MirHostPathSegment::Field(HostFieldTarget {
                            owner: host_type,
                            semantic: field,
                            runtime: field,
                            access,
                        })],
                    },
                    value: scalar(1),
                }),
                MirEffect::host_write(),
                None,
            ),
        )
        .expect("host write");
    finish(&mut function);
    let mut program = crate::MirProgram::new(table);
    program.add_function(function).expect("MIR function");
    assert!(matches!(
        verify_error(&program).into_kind(),
        MirVerifyErrorKind::InvalidHostContract(_)
    ));
}

#[test]
fn mir_verifier_rechecks_call_argument_placement_after_construction() {
    let native = FunctionId::new(7_500);
    let parameter = CompileParameter {
        name: "value".to_owned(),
        contract: None,
        default: CompileParameterDefault::Required,
        origin: None,
    };
    let native_signature = CompileSignature {
        parameters: vec![parameter],
        positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
        return_contract: None,
        effect: MirEffect::PURE,
    };
    let mut table = target_table();
    assert!(table.insert_function(CompileFunctionDescriptor {
        id: native,
        class: CompileFunctionClass::Native,
        canonical_symbol: "native::one".to_owned(),
        debug_name: "one".to_owned(),
        signature: native_signature.clone(),
        access: CompileFunctionAccess::new(true, true, true),
    }));
    let mut function = function();
    let destination = function.add_synthetic_local(MirValueType::Dynamic, origin());
    let safepoint = function.add_safepoint(MirSafepoint::new(origin()));
    function.verifier_test_append_statement_unchecked(
        function.entry_block(),
        MirStatement::new(
            origin(),
            Some(MirPlace::local(destination)),
            MirStatementKind::Call(MirCall::NativeFunction {
                function: native,
                debug_name: "one".to_owned(),
                signature: native_signature,
                arguments: Vec::new(),
            }),
            MirEffect::external_call(),
            Some(safepoint),
        ),
    );
    finish(&mut function);
    let mut program = crate::MirProgram::new(table);
    program.add_function(function).expect("MIR function");
    assert!(matches!(
        verify_error(&program).into_kind(),
        MirVerifyErrorKind::InvalidCallContract(_)
    ));
}
