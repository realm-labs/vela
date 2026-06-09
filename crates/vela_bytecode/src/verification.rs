use std::fmt;

use crate::{
    CacheSiteId, CacheSiteKind, CallArgument, CodeObject, ConstantId, HostTargetPlanId,
    Instruction, InstructionKind, InstructionOffset, Program, ProgramImage, Register,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationError {
    pub function: String,
    pub instruction: Option<usize>,
    pub kind: VerificationErrorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerificationErrorKind {
    RegisterOutOfBounds {
        register: Register,
        register_count: u16,
    },
    ConstantOutOfBounds {
        constant: ConstantId,
        constant_count: usize,
    },
    InstructionOutOfBounds {
        target: InstructionOffset,
        instruction_count: usize,
    },
    ArityFrameMismatch {
        capture_count: u16,
        parameter_count: usize,
        register_count: u16,
    },
    ParameterDefaultsMismatch {
        parameter_count: usize,
        default_count: usize,
    },
    FunctionIndexOutOfBounds {
        function: crate::FunctionIndex,
        function_count: usize,
    },
    ScriptMethodFunctionMissing {
        function: String,
    },
    GlobalSlotOutOfBounds {
        slot: usize,
        global_count: usize,
    },
    GlobalSlotNameMismatch {
        slot: usize,
        expected: String,
        actual: String,
    },
    CacheSiteOutOfBounds {
        site: CacheSiteId,
        cache_site_count: usize,
    },
    CacheSiteKindMismatch {
        site: CacheSiteId,
        expected: CacheSiteKind,
        actual: CacheSiteKind,
    },
    HostTargetOutOfBounds {
        target: HostTargetPlanId,
        target_count: usize,
    },
    HostTargetDynamicArgMismatch {
        expected: usize,
        actual: usize,
    },
    HostTargetDynamicArgGap {
        index: u8,
    },
}

impl fmt::Display for VerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.instruction {
            Some(instruction) => write!(
                f,
                "bytecode verification failed in `{}` at instruction {}: {:?}",
                self.function, instruction, self.kind
            ),
            None => write!(
                f,
                "bytecode verification failed in `{}`: {:?}",
                self.function, self.kind
            ),
        }
    }
}

impl std::error::Error for VerificationError {}

pub fn verify_program(program: &Program) -> Result<(), VerificationError> {
    for function in program.functions.values() {
        verify_code_object(function)?;
        verify_program_instruction_metadata(program, function)?;
    }
    for function in program.script_methods().function_names() {
        if !program.functions.contains_key(function) {
            return Err(error(
                function,
                None,
                VerificationErrorKind::ScriptMethodFunctionMissing {
                    function: function.to_owned(),
                },
            ));
        }
    }
    Ok(())
}

pub fn verify_program_image(image: &ProgramImage) -> Result<(), VerificationError> {
    let closure_scope = ClosureIndexScope::Image {
        function_count: image.function_count(),
    };
    for (_, function) in image.functions() {
        verify_code_object_with_scope(
            function,
            &function.name,
            closure_scope,
            CacheIndexScope::Image(image),
        )?;
        verify_program_image_instruction_metadata(image, function)?;
    }
    for function in image.script_methods().function_names() {
        if image.function_by_name(function).is_none() {
            return Err(error(
                function,
                None,
                VerificationErrorKind::ScriptMethodFunctionMissing {
                    function: function.to_owned(),
                },
            ));
        }
    }
    Ok(())
}

fn verify_program_instruction_metadata(
    program: &Program,
    code: &CodeObject,
) -> Result<(), VerificationError> {
    let global_count = program.global_names().len();
    for (index, instruction) in code.instructions.iter().enumerate() {
        if let InstructionKind::LoadGlobal {
            global,
            slot: Some(slot),
            ..
        } = &instruction.kind
        {
            if slot.get() >= global_count {
                return Err(error(
                    &code.name,
                    Some(index),
                    VerificationErrorKind::GlobalSlotOutOfBounds {
                        slot: slot.get(),
                        global_count,
                    },
                ));
            }
            if let Some(expected) = program.global_name(*slot)
                && expected != global
            {
                return Err(error(
                    &code.name,
                    Some(index),
                    VerificationErrorKind::GlobalSlotNameMismatch {
                        slot: slot.get(),
                        expected: expected.to_owned(),
                        actual: global.clone(),
                    },
                ));
            }
        }
    }
    for nested in &code.nested_functions {
        verify_program_instruction_metadata(program, nested)?;
    }
    Ok(())
}

fn verify_program_image_instruction_metadata(
    image: &ProgramImage,
    code: &CodeObject,
) -> Result<(), VerificationError> {
    let global_count = image.global_names().len();
    for (index, instruction) in code.instructions.iter().enumerate() {
        if let InstructionKind::LoadGlobal {
            global,
            slot: Some(slot),
            ..
        } = &instruction.kind
        {
            if slot.get() >= global_count {
                return Err(error(
                    &code.name,
                    Some(index),
                    VerificationErrorKind::GlobalSlotOutOfBounds {
                        slot: slot.get(),
                        global_count,
                    },
                ));
            }
            if let Some(expected) = image.global_name(*slot)
                && expected != global
            {
                return Err(error(
                    &code.name,
                    Some(index),
                    VerificationErrorKind::GlobalSlotNameMismatch {
                        slot: slot.get(),
                        expected: expected.to_owned(),
                        actual: global.clone(),
                    },
                ));
            }
        }
    }
    Ok(())
}

pub fn verify_code_object(code: &CodeObject) -> Result<(), VerificationError> {
    verify_code_object_with_name(code, &code.name)
}

fn verify_code_object_with_name(
    code: &CodeObject,
    function: &str,
) -> Result<(), VerificationError> {
    verify_code_object_with_scope(
        code,
        function,
        ClosureIndexScope::Nested,
        CacheIndexScope::Local,
    )
}

#[derive(Clone, Copy)]
enum ClosureIndexScope {
    Nested,
    Image { function_count: usize },
}

#[derive(Clone, Copy)]
enum CacheIndexScope<'a> {
    Local,
    Image(&'a ProgramImage),
}

fn verify_code_object_with_scope(
    code: &CodeObject,
    function: &str,
    closure_scope: ClosureIndexScope,
    cache_scope: CacheIndexScope<'_>,
) -> Result<(), VerificationError> {
    let parameter_count = code.params.len();
    let frame_count = usize::from(code.capture_count) + parameter_count;
    if frame_count > usize::from(code.register_count) {
        return Err(error(
            function,
            None,
            VerificationErrorKind::ArityFrameMismatch {
                capture_count: code.capture_count,
                parameter_count,
                register_count: code.register_count,
            },
        ));
    }
    if code.param_defaults.len() != parameter_count {
        return Err(error(
            function,
            None,
            VerificationErrorKind::ParameterDefaultsMismatch {
                parameter_count,
                default_count: code.param_defaults.len(),
            },
        ));
    }

    for slot in &code.frame.slots {
        verify_register(function, None, code, slot.register)?;
    }
    for (index, instruction) in code.instructions.iter().enumerate() {
        verify_instruction(
            function,
            code,
            index,
            instruction,
            closure_scope,
            cache_scope,
        )?;
    }
    for nested in &code.nested_functions {
        verify_code_object_with_scope(nested, &nested.name, closure_scope, cache_scope)?;
    }
    Ok(())
}

fn verify_instruction(
    function: &str,
    code: &CodeObject,
    index: usize,
    instruction: &Instruction,
    closure_scope: ClosureIndexScope,
    cache_scope: CacheIndexScope<'_>,
) -> Result<(), VerificationError> {
    let instruction_index = Some(index);
    match &instruction.kind {
        InstructionKind::LoadConst { dst, constant } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_constant(function, instruction_index, code, *constant)
        }
        InstructionKind::Move { dst, src }
        | InstructionKind::Not { dst, src }
        | InstructionKind::Truthy { dst, src }
        | InstructionKind::Negate { dst, src }
        | InstructionKind::TryPropagate { dst, src } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *src)
        }
        InstructionKind::Add { dst, lhs, rhs }
        | InstructionKind::Sub { dst, lhs, rhs }
        | InstructionKind::Mul { dst, lhs, rhs }
        | InstructionKind::Div { dst, lhs, rhs }
        | InstructionKind::Rem { dst, lhs, rhs }
        | InstructionKind::Equal { dst, lhs, rhs }
        | InstructionKind::NotEqual { dst, lhs, rhs }
        | InstructionKind::Less { dst, lhs, rhs }
        | InstructionKind::LessEqual { dst, lhs, rhs }
        | InstructionKind::Greater { dst, lhs, rhs }
        | InstructionKind::GreaterEqual { dst, lhs, rhs } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *lhs)?;
            verify_register(function, instruction_index, code, *rhs)
        }
        InstructionKind::JumpIfFalse { condition, target } => {
            verify_register(function, instruction_index, code, *condition)?;
            verify_jump(function, instruction_index, code, *target)
        }
        InstructionKind::JumpIfNotMissing { value, target } => {
            verify_register(function, instruction_index, code, *value)?;
            verify_jump(function, instruction_index, code, *target)
        }
        InstructionKind::Jump { target } => verify_jump(function, instruction_index, code, *target),
        InstructionKind::CallNative { dst, args, .. } => {
            if let Some(dst) = dst {
                verify_register(function, instruction_index, code, *dst)?;
            }
            verify_registers(function, instruction_index, code, args)
        }
        InstructionKind::CallFunction { dst, args, .. } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_call_arguments(function, instruction_index, code, args)
        }
        InstructionKind::MakeClosure {
            dst,
            function: nested,
            captures,
        } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_registers(function, instruction_index, code, captures)?;
            verify_function_index(function, instruction_index, code, *nested, closure_scope)
        }
        InstructionKind::CallClosure { dst, callee, args } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *callee)?;
            verify_registers(function, instruction_index, code, args)
        }
        InstructionKind::CallMethod {
            dst,
            receiver,
            args,
            ..
        } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *receiver)?;
            verify_call_arguments(function, instruction_index, code, args)
        }
        InstructionKind::CallMethodId {
            dst,
            receiver,
            args,
            ..
        } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *receiver)?;
            verify_call_arguments(function, instruction_index, code, args)
        }
        InstructionKind::MakeArray { dst, elements } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_registers(function, instruction_index, code, elements)
        }
        InstructionKind::MakeMap { dst, entries } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_registers_from_pairs(function, instruction_index, code, entries)
        }
        InstructionKind::MakeRange {
            dst, start, end, ..
        } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *start)?;
            verify_register(function, instruction_index, code, *end)
        }
        InstructionKind::MakeRecord { dst, fields, .. }
        | InstructionKind::MakeEnum { dst, fields, .. } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_registers_from_pairs(function, instruction_index, code, fields)
        }
        InstructionKind::GetRecordField { dst, record, .. }
        | InstructionKind::GetRecordSlot { dst, record, .. }
        | InstructionKind::GetEnumField {
            dst, value: record, ..
        }
        | InstructionKind::GetEnumSlot {
            dst, value: record, ..
        }
        | InstructionKind::GetIndex {
            dst, base: record, ..
        } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *record)?;
            if let InstructionKind::GetIndex { index, .. } = &instruction.kind {
                verify_register(function, instruction_index, code, *index)?;
            }
            Ok(())
        }
        InstructionKind::SetRecordField { record, src, .. }
        | InstructionKind::SetRecordSlot { record, src, .. } => {
            verify_register(function, instruction_index, code, *record)?;
            verify_register(function, instruction_index, code, *src)
        }
        InstructionKind::SetIndex { base, index, src } => {
            verify_register(function, instruction_index, code, *base)?;
            verify_register(function, instruction_index, code, *index)?;
            verify_register(function, instruction_index, code, *src)
        }
        InstructionKind::IterInit { dst, iterable } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *iterable)
        }
        InstructionKind::IterNext {
            iterator,
            dst,
            jump_if_done,
        } => {
            verify_register(function, instruction_index, code, *iterator)?;
            verify_register(function, instruction_index, code, *dst)?;
            verify_jump(function, instruction_index, code, *jump_if_done)
        }
        InstructionKind::RangeNext {
            cursor,
            end,
            done,
            dst,
            jump_if_done,
            ..
        } => {
            verify_register(function, instruction_index, code, *cursor)?;
            verify_register(function, instruction_index, code, *end)?;
            verify_register(function, instruction_index, code, *done)?;
            verify_register(function, instruction_index, code, *dst)?;
            verify_jump(function, instruction_index, code, *jump_if_done)
        }
        InstructionKind::EnumTagEqual { dst, value, .. } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *value)
        }
        InstructionKind::LoadGlobal {
            dst, cache_site, ..
        } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_optional_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::GlobalRead,
                cache_scope,
            )
        }
        InstructionKind::HostRead {
            dst,
            root,
            target,
            dynamic_args,
            cache_site,
        } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *root)?;
            verify_registers(function, instruction_index, code, dynamic_args)?;
            verify_host_target(
                function,
                instruction_index,
                code,
                *target,
                dynamic_args.len(),
            )?;
            verify_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::HostPathRead,
                cache_scope,
            )
        }
        InstructionKind::HostWrite {
            root,
            target,
            dynamic_args,
            src,
            cache_site,
        } => {
            verify_register(function, instruction_index, code, *root)?;
            verify_register(function, instruction_index, code, *src)?;
            verify_registers(function, instruction_index, code, dynamic_args)?;
            verify_host_target(
                function,
                instruction_index,
                code,
                *target,
                dynamic_args.len(),
            )?;
            verify_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::HostPathWrite,
                cache_scope,
            )
        }
        InstructionKind::HostMutate {
            root,
            target,
            dynamic_args,
            rhs,
            cache_site,
            ..
        } => {
            verify_register(function, instruction_index, code, *root)?;
            verify_register(function, instruction_index, code, *rhs)?;
            verify_registers(function, instruction_index, code, dynamic_args)?;
            verify_host_target(
                function,
                instruction_index,
                code,
                *target,
                dynamic_args.len(),
            )?;
            verify_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::HostPathMutate,
                cache_scope,
            )
        }
        InstructionKind::HostRemove {
            root,
            target,
            dynamic_args,
            cache_site,
        } => {
            verify_register(function, instruction_index, code, *root)?;
            verify_registers(function, instruction_index, code, dynamic_args)?;
            verify_host_target(
                function,
                instruction_index,
                code,
                *target,
                dynamic_args.len(),
            )?;
            verify_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::HostPathRemove,
                cache_scope,
            )
        }
        InstructionKind::HostCall {
            dst,
            root,
            target,
            dynamic_args,
            args,
            cache_site,
            ..
        } => {
            if let Some(dst) = dst {
                verify_register(function, instruction_index, code, *dst)?;
            }
            verify_register(function, instruction_index, code, *root)?;
            verify_registers(function, instruction_index, code, dynamic_args)?;
            verify_registers(function, instruction_index, code, args)?;
            verify_host_target(
                function,
                instruction_index,
                code,
                *target,
                dynamic_args.len(),
            )?;
            verify_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::HostPathCall,
                cache_scope,
            )
        }
        InstructionKind::Return { src } => verify_register(function, instruction_index, code, *src),
    }
}

fn verify_registers(
    function: &str,
    instruction: Option<usize>,
    code: &CodeObject,
    registers: &[Register],
) -> Result<(), VerificationError> {
    for register in registers {
        verify_register(function, instruction, code, *register)?;
    }
    Ok(())
}

fn verify_registers_from_pairs(
    function: &str,
    instruction: Option<usize>,
    code: &CodeObject,
    fields: &[(String, Register)],
) -> Result<(), VerificationError> {
    for (_, register) in fields {
        verify_register(function, instruction, code, *register)?;
    }
    Ok(())
}

fn verify_call_arguments(
    function: &str,
    instruction: Option<usize>,
    code: &CodeObject,
    args: &[CallArgument],
) -> Result<(), VerificationError> {
    for arg in args {
        if let CallArgument::Register(register) = arg {
            verify_register(function, instruction, code, *register)?;
        }
    }
    Ok(())
}

fn verify_register(
    function: &str,
    instruction: Option<usize>,
    code: &CodeObject,
    register: Register,
) -> Result<(), VerificationError> {
    if register.0 < code.register_count {
        Ok(())
    } else {
        Err(error(
            function,
            instruction,
            VerificationErrorKind::RegisterOutOfBounds {
                register,
                register_count: code.register_count,
            },
        ))
    }
}

fn verify_constant(
    function: &str,
    instruction: Option<usize>,
    code: &CodeObject,
    constant: ConstantId,
) -> Result<(), VerificationError> {
    if constant.0 < code.constants.len() {
        Ok(())
    } else {
        Err(error(
            function,
            instruction,
            VerificationErrorKind::ConstantOutOfBounds {
                constant,
                constant_count: code.constants.len(),
            },
        ))
    }
}

fn verify_host_target(
    function: &str,
    instruction: Option<usize>,
    code: &CodeObject,
    target: HostTargetPlanId,
    dynamic_arg_count: usize,
) -> Result<(), VerificationError> {
    let Some(plan) = code.host_target(target) else {
        return Err(error(
            function,
            instruction,
            VerificationErrorKind::HostTargetOutOfBounds {
                target,
                target_count: code.host_targets.len(),
            },
        ));
    };
    let expected = plan.parts.dynamic_arg_count();
    if expected != dynamic_arg_count {
        return Err(error(
            function,
            instruction,
            VerificationErrorKind::HostTargetDynamicArgMismatch {
                expected,
                actual: dynamic_arg_count,
            },
        ));
    }
    for expected_index in 0..expected {
        let expected_index =
            u8::try_from(expected_index).expect("host target dynamic arg index exceeds u8::MAX");
        let has_placeholder = plan.parts.as_slice().iter().any(|part| match part {
            vela_host::target::HostPathPart::DynIndex { arg }
            | vela_host::target::HostPathPart::DynKey { arg } => *arg == expected_index,
            vela_host::target::HostPathPart::Field(_)
            | vela_host::target::HostPathPart::VariantField(_)
            | vela_host::target::HostPathPart::ConstIndex(_)
            | vela_host::target::HostPathPart::ConstKey(_) => false,
        });
        if !has_placeholder {
            return Err(error(
                function,
                instruction,
                VerificationErrorKind::HostTargetDynamicArgGap {
                    index: expected_index,
                },
            ));
        }
    }
    Ok(())
}

fn verify_function_index(
    function: &str,
    instruction: Option<usize>,
    code: &CodeObject,
    nested: crate::FunctionIndex,
    closure_scope: ClosureIndexScope,
) -> Result<(), VerificationError> {
    let function_count = match closure_scope {
        ClosureIndexScope::Nested => code.nested_functions.len(),
        ClosureIndexScope::Image { function_count } => function_count,
    };
    if nested.0 < function_count {
        Ok(())
    } else {
        Err(error(
            function,
            instruction,
            VerificationErrorKind::FunctionIndexOutOfBounds {
                function: nested,
                function_count,
            },
        ))
    }
}

fn verify_jump(
    function: &str,
    instruction: Option<usize>,
    code: &CodeObject,
    target: InstructionOffset,
) -> Result<(), VerificationError> {
    if target.0 <= code.instructions.len() {
        Ok(())
    } else {
        Err(error(
            function,
            instruction,
            VerificationErrorKind::InstructionOutOfBounds {
                target,
                instruction_count: code.instructions.len(),
            },
        ))
    }
}

fn verify_optional_cache_site(
    function: &str,
    instruction: Option<usize>,
    code: &CodeObject,
    site: Option<CacheSiteId>,
    expected: CacheSiteKind,
    cache_scope: CacheIndexScope<'_>,
) -> Result<(), VerificationError> {
    let Some(site) = site else {
        return Ok(());
    };
    let desc = match cache_scope {
        CacheIndexScope::Local => code.cache_sites.get(site),
        CacheIndexScope::Image(image) => image.cache_site(site),
    };
    let Some(desc) = desc else {
        let cache_site_count = match cache_scope {
            CacheIndexScope::Local => code.cache_sites.len(),
            CacheIndexScope::Image(image) => image.cache_site_count(),
        };
        return Err(error(
            function,
            instruction,
            VerificationErrorKind::CacheSiteOutOfBounds {
                site,
                cache_site_count,
            },
        ));
    };
    if desc.kind != expected {
        return Err(error(
            function,
            instruction,
            VerificationErrorKind::CacheSiteKindMismatch {
                site,
                expected,
                actual: desc.kind,
            },
        ));
    }
    Ok(())
}

fn verify_cache_site(
    function: &str,
    instruction: Option<usize>,
    code: &CodeObject,
    site: CacheSiteId,
    expected: CacheSiteKind,
    cache_scope: CacheIndexScope<'_>,
) -> Result<(), VerificationError> {
    verify_optional_cache_site(
        function,
        instruction,
        code,
        Some(site),
        expected,
        cache_scope,
    )
}

fn error(
    function: &str,
    instruction: Option<usize>,
    kind: VerificationErrorKind,
) -> VerificationError {
    VerificationError {
        function: function.to_owned(),
        instruction,
        kind,
    }
}

#[cfg(test)]
mod tests {
    use vela_common::FieldId;
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
        program.insert_script_method("Player", "bonus", vela_common::MethodId::new(7), "missing");

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
        let target = code
            .intern_host_target(HostTargetPlan::new(vela_common::HostTypeId::new(1)).dyn_key(0));
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
        let target = code
            .intern_host_target(HostTargetPlan::new(vela_common::HostTypeId::new(1)).dyn_key(0));
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
        let target = code
            .intern_host_target(HostTargetPlan::new(vela_common::HostTypeId::new(1)).dyn_key(1));
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
}
