use std::fmt;

use vela_registry::DebugNameId;

use crate::linked::{
    MethodDispatchHandle, NativeHandle, ScriptFunctionHandle, TypeHandle, VariantHandle,
};
use crate::{
    CacheSiteId, CacheSiteKind, CallArgument, ConstantId, DynamicCallArgument, FormatStringPart,
    HostTargetPlanId, InstructionOffset, ProgramImage, Register, TypeGuardPlanId,
    UnlinkedCodeObject, UnlinkedInstruction, UnlinkedInstructionKind, UnlinkedProgram,
};

mod linked;

pub use linked::{verify_linked_code_object, verify_linked_program};

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
    ConstantKindMismatch {
        constant: ConstantId,
        expected: &'static str,
        actual: &'static str,
    },
    InstructionOutOfBounds {
        target: InstructionOffset,
        instruction_count: usize,
    },
    InvalidTypedImmediate {
        instruction: &'static str,
        reason: &'static str,
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
    ParameterGuardOutOfBounds {
        parameter: u16,
        parameter_count: usize,
    },
    TypeGuardPlanOutOfBounds {
        guard: TypeGuardPlanId,
        guard_count: usize,
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
    CacheSiteIdMismatch {
        expected: CacheSiteId,
        actual: CacheSiteId,
    },
    CacheSiteInstructionKindMismatch {
        site: CacheSiteId,
        expected: CacheSiteKind,
        actual: Option<CacheSiteKind>,
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
    DebugNameOutOfBounds {
        debug_name: DebugNameId,
        debug_name_count: usize,
    },
    NativeHandleOutOfBounds {
        handle: NativeHandle,
        native_count: usize,
    },
    ScriptFunctionHandleOutOfBounds {
        handle: ScriptFunctionHandle,
        function_count: usize,
    },
    MethodDispatchHandleOutOfBounds {
        handle: MethodDispatchHandle,
        dispatch_count: usize,
    },
    TypeHandleOutOfBounds {
        handle: TypeHandle,
        type_count: usize,
    },
    VariantHandleOutOfBounds {
        handle: VariantHandle,
        variant_count: usize,
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

pub fn verify_program(program: &UnlinkedProgram) -> Result<(), VerificationError> {
    for function in program.functions() {
        verify_code_object(function)?;
        verify_program_instruction_metadata(program, function)?;
    }
    for function in program.script_methods().function_names() {
        if program.function(function).is_none() {
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
    program: &UnlinkedProgram,
    code: &UnlinkedCodeObject,
) -> Result<(), VerificationError> {
    let global_count = program.global_names().len();
    for (index, instruction) in code.instructions.iter().enumerate() {
        if let UnlinkedInstructionKind::LoadGlobal {
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
    code: &UnlinkedCodeObject,
) -> Result<(), VerificationError> {
    let global_count = image.global_names().len();
    for (index, instruction) in code.instructions.iter().enumerate() {
        if let UnlinkedInstructionKind::LoadGlobal {
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

pub fn verify_code_object(code: &UnlinkedCodeObject) -> Result<(), VerificationError> {
    verify_code_object_with_name(code, &code.name)
}

fn verify_code_object_with_name(
    code: &UnlinkedCodeObject,
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
    code: &UnlinkedCodeObject,
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
    for guard in &code.param_guards {
        if usize::from(guard.parameter) >= parameter_count {
            return Err(error(
                function,
                None,
                VerificationErrorKind::ParameterGuardOutOfBounds {
                    parameter: guard.parameter,
                    parameter_count,
                },
            ));
        }
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
    verify_cache_site_layout(function, code, cache_scope)?;
    for nested in &code.nested_functions {
        verify_code_object_with_scope(nested, &nested.name, closure_scope, cache_scope)?;
    }
    Ok(())
}

fn verify_instruction(
    function: &str,
    code: &UnlinkedCodeObject,
    index: usize,
    instruction: &UnlinkedInstruction,
    closure_scope: ClosureIndexScope,
    cache_scope: CacheIndexScope<'_>,
) -> Result<(), VerificationError> {
    let instruction_index = Some(index);
    match &instruction.kind {
        UnlinkedInstructionKind::LoadConst { dst, constant } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_constant(function, instruction_index, code, *constant)
        }
        UnlinkedInstructionKind::Move { dst, src }
        | UnlinkedInstructionKind::Not { dst, src }
        | UnlinkedInstructionKind::Truthy { dst, src }
        | UnlinkedInstructionKind::Negate { dst, src }
        | UnlinkedInstructionKind::TryPropagate { dst, src } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *src)
        }
        UnlinkedInstructionKind::Add { dst, lhs, rhs }
        | UnlinkedInstructionKind::Sub { dst, lhs, rhs }
        | UnlinkedInstructionKind::Mul { dst, lhs, rhs }
        | UnlinkedInstructionKind::Div { dst, lhs, rhs }
        | UnlinkedInstructionKind::Rem { dst, lhs, rhs }
        | UnlinkedInstructionKind::Equal { dst, lhs, rhs }
        | UnlinkedInstructionKind::NotEqual { dst, lhs, rhs }
        | UnlinkedInstructionKind::Less { dst, lhs, rhs }
        | UnlinkedInstructionKind::LessEqual { dst, lhs, rhs }
        | UnlinkedInstructionKind::Greater { dst, lhs, rhs }
        | UnlinkedInstructionKind::GreaterEqual { dst, lhs, rhs }
        | UnlinkedInstructionKind::I64Add { dst, lhs, rhs }
        | UnlinkedInstructionKind::I64Sub { dst, lhs, rhs }
        | UnlinkedInstructionKind::I64Mul { dst, lhs, rhs }
        | UnlinkedInstructionKind::I64Rem { dst, lhs, rhs } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *lhs)?;
            verify_register(function, instruction_index, code, *rhs)
        }
        UnlinkedInstructionKind::I64AddImm { dst, lhs, .. }
        | UnlinkedInstructionKind::I64SubImm { dst, lhs, .. }
        | UnlinkedInstructionKind::I64MulImm { dst, lhs, .. }
        | UnlinkedInstructionKind::I64CmpImm { dst, lhs, .. } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *lhs)
        }
        UnlinkedInstructionKind::I64RemImm { dst, lhs, imm } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *lhs)?;
            verify_i64_rem_imm(function, instruction_index, *imm)
        }
        UnlinkedInstructionKind::I64CmpImmJumpIfFalse { lhs, target, .. } => {
            verify_register(function, instruction_index, code, *lhs)?;
            verify_jump(function, instruction_index, code, *target)
        }
        UnlinkedInstructionKind::BinaryIntLiteral { dst, value, .. }
        | UnlinkedInstructionKind::BinaryFloatLiteral { dst, value, .. } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *value)
        }
        UnlinkedInstructionKind::GuardType { src, .. } => {
            verify_register(function, instruction_index, code, *src)
        }
        UnlinkedInstructionKind::JumpIfFalse { condition, target } => {
            verify_register(function, instruction_index, code, *condition)?;
            verify_jump(function, instruction_index, code, *target)
        }
        UnlinkedInstructionKind::JumpIfNotMissing { value, target } => {
            verify_register(function, instruction_index, code, *value)?;
            verify_jump(function, instruction_index, code, *target)
        }
        UnlinkedInstructionKind::Jump { target } => {
            verify_jump(function, instruction_index, code, *target)
        }
        UnlinkedInstructionKind::CallNative {
            dst,
            cache_site,
            args,
            ..
        } => {
            if let Some(dst) = dst {
                verify_register(function, instruction_index, code, *dst)?;
            }
            verify_registers(function, instruction_index, code, args)?;
            verify_optional_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::NativeCall,
                cache_scope,
            )
        }
        UnlinkedInstructionKind::CallFunction { dst, args, .. } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_call_arguments(function, instruction_index, code, args)
        }
        UnlinkedInstructionKind::MakeClosure {
            dst,
            function: nested,
            captures,
        } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_registers(function, instruction_index, code, captures)?;
            verify_function_index(function, instruction_index, code, *nested, closure_scope)
        }
        UnlinkedInstructionKind::CallClosure { dst, callee, args } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *callee)?;
            verify_registers(function, instruction_index, code, args)
        }
        UnlinkedInstructionKind::CallDynamicMethod {
            dst,
            receiver,
            args,
            ..
        } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *receiver)?;
            verify_dynamic_call_arguments(function, instruction_index, code, args)
        }
        UnlinkedInstructionKind::CallMethodId {
            dst,
            receiver,
            args,
            ..
        } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *receiver)?;
            verify_call_arguments(function, instruction_index, code, args)
        }
        UnlinkedInstructionKind::MakeArray { dst, elements } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_registers(function, instruction_index, code, elements)
        }
        UnlinkedInstructionKind::FormatString { dst, parts } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_format_string_parts(function, instruction_index, code, parts)
        }
        UnlinkedInstructionKind::MakeMap { dst, entries } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_registers_from_pairs(function, instruction_index, code, entries)
        }
        UnlinkedInstructionKind::MakeRange {
            dst, start, end, ..
        } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *start)?;
            verify_register(function, instruction_index, code, *end)
        }
        UnlinkedInstructionKind::MakeRecord { dst, fields, .. }
        | UnlinkedInstructionKind::MakeEnum { dst, fields, .. } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_registers_from_pairs(function, instruction_index, code, fields)
        }
        UnlinkedInstructionKind::GetRecordField { dst, record, .. }
        | UnlinkedInstructionKind::GetRecordSlot { dst, record, .. }
        | UnlinkedInstructionKind::GetEnumField {
            dst, value: record, ..
        }
        | UnlinkedInstructionKind::GetEnumSlot {
            dst, value: record, ..
        }
        | UnlinkedInstructionKind::GetIndex {
            dst, base: record, ..
        }
        | UnlinkedInstructionKind::GetStringKeyIndex {
            dst, base: record, ..
        } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *record)?;
            if let UnlinkedInstructionKind::GetIndex { index, .. } = &instruction.kind {
                verify_register(function, instruction_index, code, *index)?;
            }
            if let UnlinkedInstructionKind::GetStringKeyIndex { key, .. } = &instruction.kind {
                verify_string_constant(function, instruction_index, code, *key)?;
            }
            Ok(())
        }
        UnlinkedInstructionKind::SetRecordField { record, src, .. }
        | UnlinkedInstructionKind::SetRecordSlot { record, src, .. } => {
            verify_register(function, instruction_index, code, *record)?;
            verify_register(function, instruction_index, code, *src)
        }
        UnlinkedInstructionKind::SetIndex { base, index, src } => {
            verify_register(function, instruction_index, code, *base)?;
            verify_register(function, instruction_index, code, *index)?;
            verify_register(function, instruction_index, code, *src)
        }
        UnlinkedInstructionKind::SetStringKeyIndex { base, src, .. } => {
            verify_register(function, instruction_index, code, *base)?;
            verify_register(function, instruction_index, code, *src)?;
            if let UnlinkedInstructionKind::SetStringKeyIndex { key, .. } = &instruction.kind {
                verify_string_constant(function, instruction_index, code, *key)?;
            }
            Ok(())
        }
        UnlinkedInstructionKind::IterInit { dst, iterable } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *iterable)
        }
        UnlinkedInstructionKind::IterNext {
            iterator,
            dst,
            jump_if_done,
        } => {
            verify_register(function, instruction_index, code, *iterator)?;
            verify_register(function, instruction_index, code, *dst)?;
            verify_jump(function, instruction_index, code, *jump_if_done)
        }
        UnlinkedInstructionKind::RangeNext {
            cursor,
            end,
            done,
            dst,
            jump_if_done,
            ..
        }
        | UnlinkedInstructionKind::I64RangeNext {
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
        UnlinkedInstructionKind::EnumTagEqual { dst, value, .. } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *value)
        }
        UnlinkedInstructionKind::LoadGlobal {
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
        UnlinkedInstructionKind::HostRead {
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
        UnlinkedInstructionKind::HostWrite {
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
        UnlinkedInstructionKind::HostMutate {
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
        UnlinkedInstructionKind::HostRemove {
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
        UnlinkedInstructionKind::HostCall {
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
        UnlinkedInstructionKind::Return { src } => {
            verify_register(function, instruction_index, code, *src)
        }
    }
}

fn verify_registers(
    function: &str,
    instruction: Option<usize>,
    code: &UnlinkedCodeObject,
    registers: &[Register],
) -> Result<(), VerificationError> {
    for register in registers {
        verify_register(function, instruction, code, *register)?;
    }
    Ok(())
}

fn verify_format_string_parts(
    function: &str,
    instruction: Option<usize>,
    code: &UnlinkedCodeObject,
    parts: &[FormatStringPart],
) -> Result<(), VerificationError> {
    for part in parts {
        match part {
            FormatStringPart::Text(constant) => {
                verify_string_constant(function, instruction, code, *constant)?;
            }
            FormatStringPart::Value(register) => {
                verify_register(function, instruction, code, *register)?;
            }
        }
    }
    Ok(())
}

fn verify_i64_rem_imm(
    function: &str,
    instruction: Option<usize>,
    imm: i64,
) -> Result<(), VerificationError> {
    if imm != 0 {
        Ok(())
    } else {
        Err(error(
            function,
            instruction,
            VerificationErrorKind::InvalidTypedImmediate {
                instruction: "I64RemImm",
                reason: "immediate must be nonzero",
            },
        ))
    }
}

fn verify_registers_from_pairs(
    function: &str,
    instruction: Option<usize>,
    code: &UnlinkedCodeObject,
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
    code: &UnlinkedCodeObject,
    args: &[CallArgument],
) -> Result<(), VerificationError> {
    for arg in args {
        if let CallArgument::Register(register) = arg {
            verify_register(function, instruction, code, *register)?;
        }
    }
    Ok(())
}

fn verify_dynamic_call_arguments(
    function: &str,
    instruction: Option<usize>,
    code: &UnlinkedCodeObject,
    args: &[DynamicCallArgument],
) -> Result<(), VerificationError> {
    for arg in args {
        verify_register(function, instruction, code, arg.value)?;
    }
    Ok(())
}

fn verify_register(
    function: &str,
    instruction: Option<usize>,
    code: &UnlinkedCodeObject,
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
    code: &UnlinkedCodeObject,
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

fn verify_string_constant(
    function: &str,
    instruction: Option<usize>,
    code: &UnlinkedCodeObject,
    constant: ConstantId,
) -> Result<(), VerificationError> {
    verify_constant(function, instruction, code, constant)?;
    match &code.constants[constant.0] {
        crate::Constant::String(_) => Ok(()),
        actual => Err(error(
            function,
            instruction,
            VerificationErrorKind::ConstantKindMismatch {
                constant,
                expected: "string",
                actual: constant_kind(actual),
            },
        )),
    }
}

fn constant_kind(constant: &crate::Constant) -> &'static str {
    match constant {
        crate::Constant::Null => "null",
        crate::Constant::Bool(_) => "bool",
        crate::Constant::Scalar(_) => "scalar",
        crate::Constant::String(_) => "string",
        crate::Constant::Bytes(_) => "bytes",
        crate::Constant::Array(_) => "array",
        crate::Constant::Map(_) => "map",
    }
}

fn verify_host_target(
    function: &str,
    instruction: Option<usize>,
    code: &UnlinkedCodeObject,
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
    code: &UnlinkedCodeObject,
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
    code: &UnlinkedCodeObject,
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
    code: &UnlinkedCodeObject,
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
    code: &UnlinkedCodeObject,
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

fn verify_cache_site_layout(
    function: &str,
    code: &UnlinkedCodeObject,
    cache_scope: CacheIndexScope<'_>,
) -> Result<(), VerificationError> {
    for (index, site) in code.cache_sites.sites().iter().enumerate() {
        if matches!(cache_scope, CacheIndexScope::Local) {
            let expected =
                CacheSiteId::new(u32::try_from(index).expect("cache site count exceeds u32::MAX"));
            if site.id != expected {
                return Err(error(
                    function,
                    None,
                    VerificationErrorKind::CacheSiteIdMismatch {
                        expected,
                        actual: site.id,
                    },
                ));
            }
        }

        let Some(instruction) = code.instructions.get(site.instruction_offset.0) else {
            return Err(error(
                function,
                None,
                VerificationErrorKind::InstructionOutOfBounds {
                    target: site.instruction_offset,
                    instruction_count: code.instructions.len(),
                },
            ));
        };
        let actual = instruction_cache_site_kind(&instruction.kind);
        if actual != Some(site.kind) {
            return Err(error(
                function,
                Some(site.instruction_offset.0),
                VerificationErrorKind::CacheSiteInstructionKindMismatch {
                    site: site.id,
                    expected: site.kind,
                    actual,
                },
            ));
        }
    }
    Ok(())
}

fn instruction_cache_site_kind(kind: &UnlinkedInstructionKind) -> Option<CacheSiteKind> {
    match kind {
        UnlinkedInstructionKind::LoadGlobal { .. } => Some(CacheSiteKind::GlobalRead),
        UnlinkedInstructionKind::CallNative { .. } => Some(CacheSiteKind::NativeCall),
        UnlinkedInstructionKind::CallDynamicMethod { .. }
        | UnlinkedInstructionKind::CallMethodId { .. } => Some(CacheSiteKind::MethodCall),
        UnlinkedInstructionKind::GetRecordSlot { .. } => Some(CacheSiteKind::RecordFieldRead),
        UnlinkedInstructionKind::SetRecordSlot { .. } => Some(CacheSiteKind::RecordFieldWrite),
        UnlinkedInstructionKind::HostRead { .. } => Some(CacheSiteKind::HostPathRead),
        UnlinkedInstructionKind::HostWrite { .. } => Some(CacheSiteKind::HostPathWrite),
        UnlinkedInstructionKind::HostMutate { .. } => Some(CacheSiteKind::HostPathMutate),
        UnlinkedInstructionKind::HostRemove { .. } => Some(CacheSiteKind::HostPathRemove),
        UnlinkedInstructionKind::HostCall { .. } => Some(CacheSiteKind::HostPathCall),
        _ => None,
    }
}

pub(super) fn error(
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
mod tests;

#[cfg(test)]
mod linked_tests;
