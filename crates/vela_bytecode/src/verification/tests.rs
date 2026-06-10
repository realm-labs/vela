use vela_def::FieldId;
use vela_host::target::HostTargetPlan;

use crate::{Constant, FrameSlotInfo, FrameSlotKind, Instruction};

use super::*;

#[test]
fn accepts_valid_code_object() {
    let mut code = CodeObject::new("main", 2).with_params(vec!["value".to_owned()]);
    let constant = code.push_constant(Constant::Int(42));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant,
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(verify_code_object(&code), Ok(()));
}

#[test]
fn program_verify_checks_all_functions() {
    let mut code = CodeObject::new("main", 1);
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    let mut program = Program::new();
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
    let mut program = Program::new();
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
    let code = CodeObject::new("main", 1)
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
    let code = CodeObject::new("main", 1)
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
    let mut code = CodeObject::new("main", 1);
    code.push_instruction(Instruction::new(InstructionKind::Move {
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
fn rejects_out_of_bounds_constants() {
    let mut code = CodeObject::new("main", 1);
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: ConstantId(4),
    }));

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
    let mut code = CodeObject::new("main", 1);
    code.push_instruction(Instruction::new(InstructionKind::Jump {
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
    let mut code = CodeObject::new("main", 2);
    let target =
        code.intern_host_target(HostTargetPlan::new(vela_common::HostTypeId::new(1)).dyn_key(0));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(Instruction::new(InstructionKind::HostRead {
        dst: Register(0),
        root: Register(1),
        target,
        dynamic_args: vec![Register(2)],
        cache_site,
    }));

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
    let mut closure = CodeObject::new("main::<lambda>", 1);
    closure.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));

    let mut code = CodeObject::new("main", 1);
    let function = code.push_nested_function(closure);
    code.push_instruction(Instruction::new(InstructionKind::MakeClosure {
        dst: Register(0),
        function,
        captures: Vec::new(),
    }));

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
    let mut code = CodeObject::new("main", 3);
    let target = code.intern_host_target(
        HostTargetPlan::new(vela_common::HostTypeId::new(1))
            .field(FieldId::new(2))
            .dyn_index(0),
    );
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(Instruction::new(InstructionKind::HostRead {
        dst: Register(0),
        root: Register(1),
        target,
        dynamic_args: vec![Register(2)],
        cache_site,
    }));

    assert_eq!(verify_code_object(&code), Ok(()));
}

#[test]
fn rejects_collapsed_host_target_out_of_bounds() {
    let mut code = CodeObject::new("main", 2);
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(Instruction::new(InstructionKind::HostRead {
        dst: Register(0),
        root: Register(1),
        target: HostTargetPlanId::new(0),
        dynamic_args: Vec::new(),
        cache_site,
    }));

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
    let mut code = CodeObject::new("main", 3);
    let target =
        code.intern_host_target(HostTargetPlan::new(vela_common::HostTypeId::new(1)).dyn_key(0));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(Instruction::new(InstructionKind::HostRead {
        dst: Register(0),
        root: Register(1),
        target,
        dynamic_args: Vec::new(),
        cache_site,
    }));

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
    let mut code = CodeObject::new("main", 4);
    let target =
        code.intern_host_target(HostTargetPlan::new(vela_common::HostTypeId::new(1)).dyn_key(1));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    code.push_instruction(Instruction::new(InstructionKind::HostRead {
        dst: Register(0),
        root: Register(1),
        target,
        dynamic_args: vec![Register(2), Register(3)],
        cache_site,
    }));

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
    let mut code = CodeObject::new("main", 2);
    let target = code.intern_host_target(HostTargetPlan::new(vela_common::HostTypeId::new(1)));
    let cache_site = code.push_cache_site(CacheSiteKind::HostPathWrite, InstructionOffset(0));
    code.push_instruction(Instruction::new(InstructionKind::HostRead {
        dst: Register(0),
        root: Register(1),
        target,
        dynamic_args: Vec::new(),
        cache_site,
    }));

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
    let mut code = CodeObject::new("main", 1);
    code.push_instruction(Instruction::new(InstructionKind::MakeClosure {
        dst: Register(0),
        function: crate::FunctionIndex(0),
        captures: Vec::new(),
    }));

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
    let mut program = Program::new();
    let mut code = CodeObject::new("main", 1);
    let closure = CodeObject::new("main::<lambda>", 1);
    let function = code.push_nested_function(closure);
    code.push_instruction(Instruction::new(InstructionKind::MakeClosure {
        dst: Register(0),
        function,
        captures: Vec::new(),
    }));
    program.insert_function(code);
    let image = ProgramImage::from_program(&program);

    assert_eq!(image.verify(), Ok(()));
}

#[test]
fn program_image_verify_rejects_out_of_bounds_closure_function_index() {
    let mut code = CodeObject::new("main", 1);
    code.push_instruction(Instruction::new(InstructionKind::MakeClosure {
        dst: Register(0),
        function: crate::FunctionIndex(7),
        captures: Vec::new(),
    }));
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
    let mut code = CodeObject::new("main", 1);
    code.push_instruction(Instruction::new(InstructionKind::LoadGlobal {
        dst: Register(0),
        global: "main::value".to_owned(),
        slot: None,
        cache_site: Some(CacheSiteId::new(7)),
    }));
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
    let mut code = CodeObject::new("main", 1);
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
