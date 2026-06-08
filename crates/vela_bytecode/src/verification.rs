use std::fmt;

use crate::{
    CacheSiteId, CacheSiteKind, CallArgument, CodeObject, ConstantId, HostPathSegment, Instruction,
    InstructionKind, InstructionOffset, Program, ProgramImage, Register,
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
        verify_code_object_with_scope(function, &function.name, closure_scope)?;
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
    verify_code_object_with_scope(code, function, ClosureIndexScope::Nested)
}

#[derive(Clone, Copy)]
enum ClosureIndexScope {
    Nested,
    Image { function_count: usize },
}

fn verify_code_object_with_scope(
    code: &CodeObject,
    function: &str,
    closure_scope: ClosureIndexScope,
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
        verify_instruction(function, code, index, instruction, closure_scope)?;
    }
    for nested in &code.nested_functions {
        verify_code_object_with_scope(nested, &nested.name, closure_scope)?;
    }
    Ok(())
}

fn verify_instruction(
    function: &str,
    code: &CodeObject,
    index: usize,
    instruction: &Instruction,
    closure_scope: ClosureIndexScope,
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
            )
        }
        InstructionKind::GetHostField { dst, root, .. } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *root)
        }
        InstructionKind::GetHostPath {
            dst,
            root,
            segments,
        } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *root)?;
            verify_host_path_segments(function, instruction_index, code, segments)
        }
        InstructionKind::SetHostField { root, src, .. }
        | InstructionKind::AddHostField { root, rhs: src, .. }
        | InstructionKind::SubHostField { root, rhs: src, .. }
        | InstructionKind::MulHostField { root, rhs: src, .. }
        | InstructionKind::DivHostField { root, rhs: src, .. }
        | InstructionKind::RemHostField { root, rhs: src, .. } => {
            verify_register(function, instruction_index, code, *root)?;
            verify_register(function, instruction_index, code, *src)
        }
        InstructionKind::SetHostPath {
            root,
            segments,
            src,
        }
        | InstructionKind::AddHostPath {
            root,
            segments,
            rhs: src,
        }
        | InstructionKind::SubHostPath {
            root,
            segments,
            rhs: src,
        }
        | InstructionKind::MulHostPath {
            root,
            segments,
            rhs: src,
        }
        | InstructionKind::DivHostPath {
            root,
            segments,
            rhs: src,
        }
        | InstructionKind::RemHostPath {
            root,
            segments,
            rhs: src,
        } => {
            verify_register(function, instruction_index, code, *root)?;
            verify_host_path_segments(function, instruction_index, code, segments)?;
            verify_register(function, instruction_index, code, *src)
        }
        InstructionKind::PushHostPath {
            root,
            segments,
            value,
        } => {
            verify_register(function, instruction_index, code, *root)?;
            verify_host_path_segments(function, instruction_index, code, segments)?;
            verify_register(function, instruction_index, code, *value)
        }
        InstructionKind::RemoveHostPath { root, segments } => {
            verify_register(function, instruction_index, code, *root)?;
            verify_host_path_segments(function, instruction_index, code, segments)
        }
        InstructionKind::CallHostMethod {
            dst,
            root,
            segments,
            args,
            ..
        } => {
            if let Some(dst) = dst {
                verify_register(function, instruction_index, code, *dst)?;
            }
            verify_register(function, instruction_index, code, *root)?;
            verify_host_path_segments(function, instruction_index, code, segments)?;
            verify_registers(function, instruction_index, code, args)
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

fn verify_host_path_segments(
    function: &str,
    instruction: Option<usize>,
    code: &CodeObject,
    segments: &[HostPathSegment],
) -> Result<(), VerificationError> {
    for segment in segments {
        if let HostPathSegment::Value(register) = segment {
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
) -> Result<(), VerificationError> {
    let Some(site) = site else {
        return Ok(());
    };
    let Some(desc) = code.cache_sites.get(site) else {
        return Err(error(
            function,
            instruction,
            VerificationErrorKind::CacheSiteOutOfBounds {
                site,
                cache_site_count: code.cache_sites.len(),
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
        code.push_instruction(Instruction::new(InstructionKind::GetHostPath {
            dst: Register(0),
            root: Register(1),
            segments: vec![
                HostPathSegment::Field(FieldId::new(2)),
                HostPathSegment::Value(Register(2)),
            ],
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
