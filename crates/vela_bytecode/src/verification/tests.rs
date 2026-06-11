use vela_def::FieldId;
use vela_host::target::HostTargetPlan;
use vela_registry::DebugNameId;

use crate::{
    Constant, FieldSlot, FrameSlotInfo, FrameSlotKind, GuardContext, GuardKind, GuardLocation,
    Instruction, InstructionKind, LinkedCodeObject, LinkedMethodDispatch, LinkedMethodDispatchKind,
    LinkedNativeFunction, LinkedProgram, LinkedType, LinkedVariant, MethodDispatchHandle,
    NativeHandle, ScriptFunctionHandle, TypeGuard, TypeGuardPlan, TypeHandle, UnlinkedInstruction,
    UnlinkedTypeGuard, UnlinkedTypeGuardPlan, VariantHandle,
};

use super::*;

#[test]
fn accepts_valid_code_object() {
    let mut code = UnlinkedCodeObject::new("main", 2).with_params(vec!["value".to_owned()]);
    let constant = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(42)));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(verify_code_object(&code), Ok(()));
}

#[test]
fn linked_program_verify_accepts_valid_handles_and_debug_names() {
    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let native_name = program.intern_debug_name("award");
    let method_name = program.intern_debug_name("score");
    let type_name = program.intern_debug_name("Player");
    let variant_name = program.intern_debug_name("Player::Ranked");
    let field_name = program.intern_debug_name("score");

    let native = program.push_native_function(LinkedNativeFunction::new(
        vela_def::FunctionId::new(1),
        native_name,
    ));
    let ty = program.push_type(LinkedType::new(vela_def::TypeId::new(2), type_name));
    let variant = program.push_variant(LinkedVariant::new(
        vela_def::VariantId::new(3),
        ty,
        variant_name,
    ));
    let dispatch = program.push_method_dispatch(LinkedMethodDispatch::new(
        method_name,
        LinkedMethodDispatchKind::Value {
            method_id: vela_def::MethodId::new(4),
        },
    ));

    let mut code = LinkedCodeObject::new(main_name, 3);
    let constant = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(42)));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant,
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(Register(1)),
        native,
        debug_name: native_name,
        args: vec![Register(0)],
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(2),
        receiver: Register(1),
        dispatch,
        debug_name: method_name,
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::MakeEnum {
        dst: Register(2),
        enum_ty: ty,
        variant,
        fields: vec![(FieldSlot::new(0), field_name, Register(1))],
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));

    let main = program.push_function(code);
    program.set_entry_point(main_name, main);

    assert_eq!(program.verify(), Ok(()));
}

#[test]
fn linked_program_verify_rejects_invalid_debug_name_references() {
    let mut program = LinkedProgram::new();
    let code = LinkedCodeObject::new(DebugNameId::new(99), 1);
    program.push_function(code);

    assert_eq!(
        verify_linked_program(&program),
        Err(error(
            "<linked function 0>",
            None,
            VerificationErrorKind::DebugNameOutOfBounds {
                debug_name: DebugNameId::new(99),
                debug_name_count: 0,
            }
        ))
    );
}

#[test]
fn linked_program_verify_rejects_invalid_native_handles() {
    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let native_name = program.intern_debug_name("award");
    let mut code = LinkedCodeObject::new(main_name, 1);
    code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: None,
        native: NativeHandle::new(0),
        debug_name: native_name,
        args: Vec::new(),
    }));
    program.push_function(code);

    assert_eq!(
        verify_linked_program(&program),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::NativeHandleOutOfBounds {
                handle: NativeHandle::new(0),
                native_count: 0,
            }
        ))
    );
}

#[test]
fn linked_program_verify_rejects_invalid_script_function_handles() {
    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let helper_name = program.intern_debug_name("helper");
    let mut code = LinkedCodeObject::new(main_name, 1);
    code.push_instruction(Instruction::new(InstructionKind::CallFunction {
        dst: Register(0),
        function: ScriptFunctionHandle::new(1),
        debug_name: helper_name,
        args: Vec::new(),
    }));
    program.push_function(code);

    assert_eq!(
        verify_linked_program(&program),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::ScriptFunctionHandleOutOfBounds {
                handle: ScriptFunctionHandle::new(1),
                function_count: 1,
            }
        ))
    );
}

#[test]
fn linked_program_verify_rejects_invalid_method_type_and_variant_handles() {
    let mut method_program = LinkedProgram::new();
    let main_name = method_program.intern_debug_name("main");
    let method_name = method_program.intern_debug_name("score");
    let mut method_code = LinkedCodeObject::new(main_name, 2);
    method_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(0),
        receiver: Register(1),
        dispatch: MethodDispatchHandle::new(0),
        debug_name: method_name,
        args: Vec::new(),
    }));
    method_program.push_function(method_code);

    assert_eq!(
        verify_linked_program(&method_program),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::MethodDispatchHandleOutOfBounds {
                handle: MethodDispatchHandle::new(0),
                dispatch_count: 0,
            }
        ))
    );

    let mut type_program = LinkedProgram::new();
    let main_name = type_program.intern_debug_name("main");
    let mut type_code = LinkedCodeObject::new(main_name, 1);
    type_code.push_instruction(Instruction::new(InstructionKind::MakeRecord {
        dst: Register(0),
        ty: TypeHandle::new(0),
        fields: Vec::new(),
    }));
    type_program.push_function(type_code);

    assert_eq!(
        verify_linked_program(&type_program),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::TypeHandleOutOfBounds {
                handle: TypeHandle::new(0),
                type_count: 0,
            }
        ))
    );

    let mut variant_program = LinkedProgram::new();
    let main_name = variant_program.intern_debug_name("main");
    let type_name = variant_program.intern_debug_name("Player");
    let ty = variant_program.push_type(LinkedType::new(vela_def::TypeId::new(2), type_name));
    let mut variant_code = LinkedCodeObject::new(main_name, 1);
    variant_code.push_instruction(Instruction::new(InstructionKind::MakeEnum {
        dst: Register(0),
        enum_ty: ty,
        variant: VariantHandle::new(0),
        fields: Vec::new(),
    }));
    variant_program.push_function(variant_code);

    assert_eq!(
        verify_linked_program(&variant_program),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::VariantHandleOutOfBounds {
                handle: VariantHandle::new(0),
                variant_count: 0,
            }
        ))
    );
}

#[test]
fn linked_program_verify_accepts_guard_plans_and_keeps_debug_context() {
    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let param_name = program.intern_debug_name("amount");
    let type_name = program.intern_debug_name("Invoice");
    let variant_name = program.intern_debug_name("Invoice::Paid");

    let ty = program.push_type(LinkedType::new(vela_def::TypeId::new(1), type_name));
    let variant = program.push_variant(LinkedVariant::new(
        vela_def::VariantId::new(2),
        ty,
        variant_name,
    ));

    let mut code = LinkedCodeObject::new(main_name, 1);
    let primitive = code.intern_type_guard(TypeGuard::new(
        TypeGuardPlan::Primitive(vela_common::PrimitiveTag::I64),
        GuardContext::new(
            GuardKind::Contract,
            GuardLocation::Parameter { index: 0 },
            param_name,
        ),
    ));
    let shape = code.intern_type_guard(TypeGuard::new(
        TypeGuardPlan::Shape {
            ty,
            shape_id: vela_common::ShapeId::new(7),
        },
        GuardContext::new(GuardKind::Specialization, GuardLocation::Local, type_name),
    ));
    code.intern_type_guard(TypeGuard::new(
        TypeGuardPlan::Variant(variant),
        GuardContext::new(GuardKind::Contract, GuardLocation::Return, variant_name),
    ));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(0),
    }));

    assert_eq!(
        code.type_guard(primitive)
            .map(|guard| guard.context.debug_name),
        Some(param_name)
    );
    assert_eq!(
        code.type_guard(shape).map(|guard| &guard.plan),
        Some(&TypeGuardPlan::Shape {
            ty,
            shape_id: vela_common::ShapeId::new(7),
        })
    );

    program.push_function(code);

    assert_eq!(program.verify(), Ok(()));
}

#[test]
fn linked_program_verify_rejects_invalid_guard_type_and_variant_handles() {
    let mut type_program = LinkedProgram::new();
    let main_name = type_program.intern_debug_name("main");
    let guard_name = type_program.intern_debug_name("amount");
    let mut type_code = LinkedCodeObject::new(main_name, 1);
    type_code.intern_type_guard(TypeGuard::new(
        TypeGuardPlan::Type(TypeHandle::new(0)),
        GuardContext::new(GuardKind::Contract, GuardLocation::Field, guard_name),
    ));
    type_program.push_function(type_code);

    assert_eq!(
        verify_linked_program(&type_program),
        Err(error(
            "main",
            None,
            VerificationErrorKind::TypeHandleOutOfBounds {
                handle: TypeHandle::new(0),
                type_count: 0,
            }
        ))
    );

    let mut variant_program = LinkedProgram::new();
    let main_name = variant_program.intern_debug_name("main");
    let guard_name = variant_program.intern_debug_name("status");
    let mut variant_code = LinkedCodeObject::new(main_name, 1);
    variant_code.intern_type_guard(TypeGuard::new(
        TypeGuardPlan::Variant(VariantHandle::new(0)),
        GuardContext::new(GuardKind::Contract, GuardLocation::Return, guard_name),
    ));
    variant_program.push_function(variant_code);

    assert_eq!(
        verify_linked_program(&variant_program),
        Err(error(
            "main",
            None,
            VerificationErrorKind::VariantHandleOutOfBounds {
                handle: VariantHandle::new(0),
                variant_count: 0,
            }
        ))
    );
}

#[test]
fn verify_rejects_guard_metadata_outside_function_layout() {
    let mut unlinked = UnlinkedCodeObject::new("main", 1).with_params(vec!["value".to_owned()]);
    unlinked.push_param_guard(
        1,
        UnlinkedTypeGuard::new(
            UnlinkedTypeGuardPlan::Primitive(vela_common::PrimitiveTag::I64),
            crate::UnlinkedGuardContext::new(
                GuardKind::Contract,
                GuardLocation::Parameter { index: 1 },
                "value",
            ),
        ),
    );

    assert_eq!(
        verify_code_object(&unlinked),
        Err(error(
            "main",
            None,
            VerificationErrorKind::ParameterGuardOutOfBounds {
                parameter: 1,
                parameter_count: 1,
            }
        ))
    );

    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut linked = LinkedCodeObject::new(main_name, 1).with_params(vec![main_name]);
    linked.push_param_guard(0, crate::TypeGuardPlanId::new(0));
    program.push_function(linked);

    assert_eq!(
        verify_linked_program(&program),
        Err(error(
            "main",
            None,
            VerificationErrorKind::TypeGuardPlanOutOfBounds {
                guard: crate::TypeGuardPlanId::new(0),
                guard_count: 0,
            }
        ))
    );
}

#[test]
fn linked_program_verify_checks_side_table_handles() {
    let mut dispatch_program = LinkedProgram::new();
    let method_name = dispatch_program.intern_debug_name("score");
    dispatch_program.push_method_dispatch(LinkedMethodDispatch::new(
        method_name,
        LinkedMethodDispatchKind::Script {
            method_id: vela_def::MethodId::new(1),
            function: ScriptFunctionHandle::new(0),
        },
    ));

    assert_eq!(
        verify_linked_program(&dispatch_program),
        Err(error(
            "<linked method dispatch>",
            None,
            VerificationErrorKind::ScriptFunctionHandleOutOfBounds {
                handle: ScriptFunctionHandle::new(0),
                function_count: 0,
            }
        ))
    );

    let mut variant_program = LinkedProgram::new();
    let variant_name = variant_program.intern_debug_name("Player::Ranked");
    variant_program.push_variant(LinkedVariant::new(
        vela_def::VariantId::new(1),
        TypeHandle::new(0),
        variant_name,
    ));

    assert_eq!(
        verify_linked_program(&variant_program),
        Err(error(
            "<linked variant>",
            None,
            VerificationErrorKind::TypeHandleOutOfBounds {
                handle: TypeHandle::new(0),
                type_count: 0,
            }
        ))
    );
}

#[test]
fn linked_code_verify_preserves_local_invariant_checks() {
    let mut register_code = LinkedCodeObject::new(DebugNameId::new(0), 1);
    register_code.push_instruction(Instruction::new(InstructionKind::Move {
        dst: Register(0),
        src: Register(1),
    }));
    assert_eq!(
        verify_linked_code_object(&register_code),
        Err(error(
            "<linked code>",
            Some(0),
            VerificationErrorKind::RegisterOutOfBounds {
                register: Register(1),
                register_count: 1,
            }
        ))
    );

    let mut constant_code = LinkedCodeObject::new(DebugNameId::new(0), 1);
    constant_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: ConstantId(3),
    }));
    assert_eq!(
        verify_linked_code_object(&constant_code),
        Err(error(
            "<linked code>",
            Some(0),
            VerificationErrorKind::ConstantOutOfBounds {
                constant: ConstantId(3),
                constant_count: 0,
            }
        ))
    );

    let mut host_code = LinkedCodeObject::new(DebugNameId::new(0), 3);
    let target = host_code.intern_host_target(
        HostTargetPlan::new(vela_common::HostTypeId::new(1))
            .field(FieldId::new(2))
            .dyn_key(0),
    );
    let cache_site = host_code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    host_code.push_instruction(Instruction::new(InstructionKind::HostRead {
        dst: Register(0),
        root: Register(1),
        target,
        dynamic_args: Vec::new(),
        cache_site,
    }));
    assert_eq!(
        verify_linked_code_object(&host_code),
        Err(error(
            "<linked code>",
            Some(0),
            VerificationErrorKind::HostTargetDynamicArgMismatch {
                expected: 1,
                actual: 0,
            }
        ))
    );
}

#[test]
fn program_verify_checks_all_functions() {
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);

    assert_eq!(
        program.verify(),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::RegisterOutOfBounds {
                register: Register(2),
                register_count: 1
            }
        ))
    );
}

#[test]
fn program_verify_rejects_missing_script_method_function() {
    let mut program = UnlinkedProgram::new();
    program.insert_script_method("Player", "bonus", vela_def::MethodId::new(7), "missing");

    assert_eq!(
        program.verify(),
        Err(error(
            "missing",
            None,
            VerificationErrorKind::ScriptMethodFunctionMissing {
                function: "missing".to_owned()
            }
        ))
    );
}

#[test]
fn rejects_parameter_frame_mismatch() {
    let code = UnlinkedCodeObject::new("main", 1)
        .with_capture_count(1)
        .with_params(vec!["value".to_owned()]);

    assert_eq!(
        code.verify(),
        Err(error(
            "main",
            None,
            VerificationErrorKind::ArityFrameMismatch {
                capture_count: 1,
                parameter_count: 1,
                register_count: 1
            }
        ))
    );
}

#[test]
fn rejects_parameter_default_mismatch() {
    let code = UnlinkedCodeObject::new("main", 1)
        .with_params(vec!["value".to_owned()])
        .with_param_defaults(Vec::new());

    assert_eq!(
        code.verify(),
        Err(error(
            "main",
            None,
            VerificationErrorKind::ParameterDefaultsMismatch {
                parameter_count: 1,
                default_count: 0
            }
        ))
    );
}

#[test]
fn rejects_out_of_bounds_registers() {
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Move {
        dst: Register(0),
        src: Register(1),
    }));

    assert_eq!(
        verify_code_object(&code),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::RegisterOutOfBounds {
                register: Register(1),
                register_count: 1
            }
        ))
    );
}

#[test]
fn rejects_out_of_bounds_deferred_literal_operand_registers() {
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::BinaryIntLiteral {
            dst: Register(0),
            op: crate::BinaryLiteralOp::Add,
            value: Register(1),
            literal: "1".to_owned(),
            side: crate::BinaryLiteralSide::Right,
        },
    ));

    assert_eq!(
        verify_code_object(&code),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::RegisterOutOfBounds {
                register: Register(1),
                register_count: 1
            }
        ))
    );
}

#[test]
fn rejects_out_of_bounds_constants() {
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: ConstantId(4),
        },
    ));

    assert_eq!(
        verify_code_object(&code),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::ConstantOutOfBounds {
                constant: ConstantId(4),
                constant_count: 0
            }
        ))
    );
}

#[test]
fn rejects_out_of_bounds_jumps() {
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Jump {
        target: InstructionOffset(2),
    }));

    assert_eq!(
        verify_code_object(&code),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::InstructionOutOfBounds {
                target: InstructionOffset(2),
                instruction_count: 1
            }
        ))
    );
}

#[test]
fn rejects_host_path_dynamic_registers_outside_frame() {
    let mut code = UnlinkedCodeObject::new("main", 2);
    let target =
        code.intern_host_target(HostTargetPlan::new(vela_common::HostTypeId::new(1)).dyn_key(0));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostRead {
            dst: Register(0),
            root: Register(1),
            target,
            dynamic_args: vec![Register(2)],
            cache_site,
        },
    ));

    assert_eq!(
        verify_code_object(&code),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::RegisterOutOfBounds {
                register: Register(2),
                register_count: 2
            }
        ))
    );
}

#[test]
fn rejects_nested_closure_invalid_registers() {
    let mut closure = UnlinkedCodeObject::new("main::<lambda>", 1);
    closure.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));

    let mut code = UnlinkedCodeObject::new("main", 1);
    let function = code.push_nested_function(closure);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeClosure {
            dst: Register(0),
            function,
            captures: Vec::new(),
        },
    ));

    assert_eq!(
        verify_code_object(&code),
        Err(error(
            "main::<lambda>",
            Some(0),
            VerificationErrorKind::RegisterOutOfBounds {
                register: Register(1),
                register_count: 1
            }
        ))
    );
}

#[test]
fn accepts_collapsed_host_read_with_verified_target_and_cache_site() {
    let mut code = UnlinkedCodeObject::new("main", 3);
    let target = code.intern_host_target(
        HostTargetPlan::new(vela_common::HostTypeId::new(1))
            .field(FieldId::new(2))
            .dyn_index(0),
    );
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostRead {
            dst: Register(0),
            root: Register(1),
            target,
            dynamic_args: vec![Register(2)],
            cache_site,
        },
    ));

    assert_eq!(verify_code_object(&code), Ok(()));
}

#[test]
fn rejects_collapsed_host_target_out_of_bounds() {
    let mut code = UnlinkedCodeObject::new("main", 2);
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostRead {
            dst: Register(0),
            root: Register(1),
            target: HostTargetPlanId::new(0),
            dynamic_args: Vec::new(),
            cache_site,
        },
    ));

    assert_eq!(
        verify_code_object(&code),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::HostTargetOutOfBounds {
                target: HostTargetPlanId::new(0),
                target_count: 0
            }
        ))
    );
}

#[test]
fn rejects_collapsed_host_dynamic_arg_count_mismatch() {
    let mut code = UnlinkedCodeObject::new("main", 3);
    let target =
        code.intern_host_target(HostTargetPlan::new(vela_common::HostTypeId::new(1)).dyn_key(0));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostRead {
            dst: Register(0),
            root: Register(1),
            target,
            dynamic_args: Vec::new(),
            cache_site,
        },
    ));

    assert_eq!(
        verify_code_object(&code),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::HostTargetDynamicArgMismatch {
                expected: 1,
                actual: 0
            }
        ))
    );
}

#[test]
fn rejects_collapsed_host_dynamic_arg_gaps() {
    let mut code = UnlinkedCodeObject::new("main", 4);
    let target =
        code.intern_host_target(HostTargetPlan::new(vela_common::HostTypeId::new(1)).dyn_key(1));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostRead {
            dst: Register(0),
            root: Register(1),
            target,
            dynamic_args: vec![Register(2), Register(3)],
            cache_site,
        },
    ));

    assert_eq!(
        verify_code_object(&code),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::HostTargetDynamicArgGap { index: 0 }
        ))
    );
}

#[test]
fn rejects_collapsed_host_cache_site_kind_mismatch() {
    let mut code = UnlinkedCodeObject::new("main", 2);
    let target = code.intern_host_target(HostTargetPlan::new(vela_common::HostTypeId::new(1)));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathWrite, InstructionOffset(0));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostRead {
            dst: Register(0),
            root: Register(1),
            target,
            dynamic_args: Vec::new(),
            cache_site,
        },
    ));

    assert_eq!(
        verify_code_object(&code),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::CacheSiteKindMismatch {
                site: cache_site,
                expected: CacheSiteKind::HostPathRead,
                actual: CacheSiteKind::HostPathWrite
            }
        ))
    );
}

#[test]
fn rejects_out_of_bounds_closure_function_index() {
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeClosure {
            dst: Register(0),
            function: crate::FunctionIndex(0),
            captures: Vec::new(),
        },
    ));

    assert_eq!(
        verify_code_object(&code),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::FunctionIndexOutOfBounds {
                function: crate::FunctionIndex(0),
                function_count: 0
            }
        ))
    );
}

#[test]
fn program_image_verify_accepts_flattened_closure_function_index() {
    let mut program = UnlinkedProgram::new();
    let mut code = UnlinkedCodeObject::new("main", 1);
    let closure = UnlinkedCodeObject::new("main::<lambda>", 1);
    let function = code.push_nested_function(closure);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeClosure {
            dst: Register(0),
            function,
            captures: Vec::new(),
        },
    ));
    program.insert_function(code);
    let image = ProgramImage::from_program(&program);

    assert_eq!(image.verify(), Ok(()));
}

#[test]
fn program_image_verify_rejects_out_of_bounds_closure_function_index() {
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeClosure {
            dst: Register(0),
            function: crate::FunctionIndex(7),
            captures: Vec::new(),
        },
    ));
    let image = ProgramImage::from_parts(
        [code],
        Vec::<String>::new(),
        crate::script_methods::ScriptMethodTable::default(),
        None,
    );

    assert_eq!(
        image.verify(),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::FunctionIndexOutOfBounds {
                function: crate::FunctionIndex(7),
                function_count: 1
            }
        ))
    );
}

#[test]
fn program_image_verify_rejects_out_of_bounds_cache_site_index() {
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadGlobal {
            dst: Register(0),
            global: "main::value".to_owned(),
            slot: None,
            cache_site: Some(CacheSiteId::new(7)),
        },
    ));
    let image = ProgramImage::from_parts(
        [code],
        Vec::<String>::new(),
        crate::script_methods::ScriptMethodTable::default(),
        None,
    );

    assert_eq!(
        image.verify(),
        Err(error(
            "main",
            Some(0),
            VerificationErrorKind::CacheSiteOutOfBounds {
                site: CacheSiteId::new(7),
                cache_site_count: 0
            }
        ))
    );
}

#[test]
fn rejects_frame_metadata_registers_outside_frame() {
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.frame.push_slot(FrameSlotInfo::new(
        "bad",
        Register(3),
        FrameSlotKind::Local,
        None,
        None,
    ));

    assert_eq!(
        verify_code_object(&code),
        Err(error(
            "main",
            None,
            VerificationErrorKind::RegisterOutOfBounds {
                register: Register(3),
                register_count: 1
            }
        ))
    );
}
