use std::collections::BTreeSet;
use std::fmt;

use vela_registry::DebugNameId;

use crate::linked::{
    MethodDispatchHandle, NativeHandle, ScriptFunctionHandle, TypeHandle, VariantHandle,
};
use crate::{
    CacheSiteId, CacheSiteInstruction, CacheSiteKind, CacheSiteStorage, CallArgument, ConstantId,
    DynamicCallArgument, FormatStringPart, HostTargetPlanId, InstructionOffset, ProgramImage,
    Register, TypeGuardPlanId, UnlinkedCodeObject, UnlinkedInstruction, UnlinkedInstructionKind,
    UnlinkedProgram,
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
    InvalidExecutionUnits {
        units: u32,
    },
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
    InvalidAwaitOperation,
    InvalidSelectedPlan {
        detail: &'static str,
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
    StateSlotOutOfBounds {
        slot: usize,
        state_count: usize,
    },
    StateSlotNameMismatch {
        slot: usize,
        expected: String,
        actual: String,
    },
    StateStorageMismatch {
        slot: usize,
        expected: crate::StateStorage,
        actual: crate::StateStorage,
    },
    InvalidStateDescriptor {
        slot: usize,
        detail: String,
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
    verify_state_descriptors(program.states())?;
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
    verify_state_initializer_targets(program.states(), |function| {
        program.function_by_id(function).is_some()
    })?;
    Ok(())
}

pub fn verify_program_image(image: &ProgramImage) -> Result<(), VerificationError> {
    verify_state_descriptors(image.states())?;
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
    verify_state_initializer_targets(image.states(), |function| {
        image.function_by_id(function).is_some()
    })?;
    Ok(())
}

fn verify_state_descriptors(states: &[crate::StateDescriptor]) -> Result<(), VerificationError> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for (slot, state) in states.iter().enumerate() {
        let detail = if state.qualified_name.is_empty() {
            Some("qualified name is empty".to_owned())
        } else if !ids.insert(state.id) {
            Some(format!("duplicate state ID {}", state.id.get()))
        } else if !names.insert(state.qualified_name.as_str()) {
            Some(format!(
                "duplicate qualified name `{}`",
                state.qualified_name
            ))
        } else if state.storage == crate::StateStorage::Extern && state.initializer.is_some() {
            Some("extern state carries an initializer".to_owned())
        } else if state.storage == crate::StateStorage::Extern
            && !matches!(state.type_contract, vela_mir::MirTypeContract::Host(_))
        {
            Some("extern state requires a host type contract".to_owned())
        } else {
            None
        };
        if let Some(detail) = detail {
            return Err(error(
                "<state descriptor>",
                None,
                VerificationErrorKind::InvalidStateDescriptor { slot, detail },
            ));
        }
    }
    Ok(())
}

fn verify_state_initializer_targets(
    states: &[crate::StateDescriptor],
    has_initializer: impl Fn(vela_def::FunctionId) -> bool,
) -> Result<(), VerificationError> {
    for (slot, state) in states.iter().enumerate() {
        let detail = if state.storage == crate::StateStorage::Vm && state.initializer.is_none() {
            Some("VM state is missing its required initializer".to_owned())
        } else if state
            .initializer
            .is_some_and(|function| !has_initializer(function))
        {
            Some("initializer function is missing from the program".to_owned())
        } else {
            None
        };
        if let Some(detail) = detail {
            return Err(error(
                "<state descriptor>",
                None,
                VerificationErrorKind::InvalidStateDescriptor { slot, detail },
            ));
        }
    }
    Ok(())
}

fn verify_program_instruction_metadata(
    program: &UnlinkedProgram,
    code: &UnlinkedCodeObject,
) -> Result<(), VerificationError> {
    let state_count = program.states().len();
    for (index, instruction) in code.instructions.iter().enumerate() {
        let target = match &instruction.kind {
            UnlinkedInstructionKind::LoadState {
                state,
                slot: Some(slot),
                ..
            }
            | UnlinkedInstructionKind::StoreState {
                state,
                slot: Some(slot),
                ..
            } => Some((state, *slot, crate::StateStorage::Vm)),
            UnlinkedInstructionKind::LoadExternState {
                state,
                slot: Some(slot),
                ..
            } => Some((state, *slot, crate::StateStorage::Extern)),
            _ => None,
        };
        if let Some((state, slot, expected_storage)) = target {
            if slot.get() >= state_count {
                return Err(error(
                    &code.name,
                    Some(index),
                    VerificationErrorKind::StateSlotOutOfBounds {
                        slot: slot.get(),
                        state_count,
                    },
                ));
            }
            let descriptor = program.state(slot).expect("state slot bounds were checked");
            if descriptor.qualified_name != *state {
                return Err(error(
                    &code.name,
                    Some(index),
                    VerificationErrorKind::StateSlotNameMismatch {
                        slot: slot.get(),
                        expected: descriptor.qualified_name.clone(),
                        actual: state.clone(),
                    },
                ));
            }
            if descriptor.storage != expected_storage {
                return Err(error(
                    &code.name,
                    Some(index),
                    VerificationErrorKind::StateStorageMismatch {
                        slot: slot.get(),
                        expected: expected_storage,
                        actual: descriptor.storage,
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
    let state_count = image.states().len();
    for (index, instruction) in code.instructions.iter().enumerate() {
        let target = match &instruction.kind {
            UnlinkedInstructionKind::LoadState {
                state,
                slot: Some(slot),
                ..
            }
            | UnlinkedInstructionKind::StoreState {
                state,
                slot: Some(slot),
                ..
            } => Some((state, *slot, crate::StateStorage::Vm)),
            UnlinkedInstructionKind::LoadExternState {
                state,
                slot: Some(slot),
                ..
            } => Some((state, *slot, crate::StateStorage::Extern)),
            _ => None,
        };
        if let Some((state, slot, expected_storage)) = target {
            if slot.get() >= state_count {
                return Err(error(
                    &code.name,
                    Some(index),
                    VerificationErrorKind::StateSlotOutOfBounds {
                        slot: slot.get(),
                        state_count,
                    },
                ));
            }
            let descriptor = image.state(slot).expect("state slot bounds were checked");
            if descriptor.qualified_name != *state {
                return Err(error(
                    &code.name,
                    Some(index),
                    VerificationErrorKind::StateSlotNameMismatch {
                        slot: slot.get(),
                        expected: descriptor.qualified_name.clone(),
                        actual: state.clone(),
                    },
                ));
            }
            if descriptor.storage != expected_storage {
                return Err(error(
                    &code.name,
                    Some(index),
                    VerificationErrorKind::StateStorageMismatch {
                        slot: slot.get(),
                        expected: expected_storage,
                        actual: descriptor.storage,
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
    crate::selected_plan::verify_selected_physical_units(code).map_err(|detail| {
        error(
            function,
            None,
            VerificationErrorKind::InvalidSelectedPlan { detail },
        )
    })?;
    crate::scalar_plan::verify_scalar_block_plans(
        &code.scalar_blocks,
        code.register_count,
        code.instructions.len(),
    )
    .map_err(|detail| {
        error(
            function,
            None,
            VerificationErrorKind::InvalidSelectedPlan { detail },
        )
    })?;
    crate::scalar_plan::verify_unlinked_scalar_block_references(code).map_err(|detail| {
        error(
            function,
            None,
            VerificationErrorKind::InvalidSelectedPlan { detail },
        )
    })?;
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
    verify_instruction_cache_site(function, instruction_index, code, instruction, cache_scope)?;
    match &instruction.kind {
        UnlinkedInstructionKind::ChargeExecutionUnits { units } => {
            verify_execution_units(function, instruction_index, *units)
        }
        UnlinkedInstructionKind::LoadConst { dst, constant } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_constant(function, instruction_index, code, *constant)
        }
        UnlinkedInstructionKind::Move { dst, src }
        | UnlinkedInstructionKind::Not { dst, src }
        | UnlinkedInstructionKind::Truthy { dst, src }
        | UnlinkedInstructionKind::Negate { dst, src }
        | UnlinkedInstructionKind::ReleaseBorrowLease { dst, src }
        | UnlinkedInstructionKind::TryReleaseBorrowLease { dst, src }
        | UnlinkedInstructionKind::TryPropagate { dst, src, .. } => {
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
        | UnlinkedInstructionKind::IdentityEqual { dst, lhs, rhs }
        | UnlinkedInstructionKind::IdentityNotEqual { dst, lhs, rhs }
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
        UnlinkedInstructionKind::RunScalarBlock { .. } => Ok(()),
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
        UnlinkedInstructionKind::AwaitCall { operation, resume } => {
            verify_jump(function, instruction_index, code, *resume)?;
            if !is_unlinked_await_operation(operation) {
                return Err(error(
                    function,
                    instruction_index,
                    VerificationErrorKind::InvalidAwaitOperation,
                ));
            }
            verify_instruction(
                function,
                code,
                index,
                &UnlinkedInstruction::new((**operation).clone()),
                closure_scope,
                cache_scope,
            )
        }
        UnlinkedInstructionKind::CallNative { dst, args, .. } => {
            if let Some(dst) = dst {
                verify_register(function, instruction_index, code, *dst)?;
            }
            verify_registers(function, instruction_index, code, args)
        }
        UnlinkedInstructionKind::CallFunction { dst, args, .. } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_call_arguments(function, instruction_index, code, args)
        }
        UnlinkedInstructionKind::Task(task) => {
            verify_register(function, instruction_index, code, task.dst)?;
            verify_call_arguments(function, instruction_index, code, &task.args)
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
        UnlinkedInstructionKind::MakeTuple { dst, elements } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_registers(function, instruction_index, code, elements)
        }
        UnlinkedInstructionKind::MakeSetFromArray { dst, src } => {
            verify_register(function, instruction_index, code, *dst)?;
            verify_register(function, instruction_index, code, *src)
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
        | UnlinkedInstructionKind::TupleArityEqual {
            dst, value: record, ..
        }
        | UnlinkedInstructionKind::GetTupleField {
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
        UnlinkedInstructionKind::GuardTupleArity { value, .. } => {
            verify_register(function, instruction_index, code, *value)
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
        UnlinkedInstructionKind::IterInit { dst, iterable, .. } => {
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
        UnlinkedInstructionKind::LoadState { dst, .. }
        | UnlinkedInstructionKind::LoadExternState { dst, .. } => {
            verify_register(function, instruction_index, code, *dst)
        }
        UnlinkedInstructionKind::StoreState { src, .. } => {
            verify_register(function, instruction_index, code, *src)
        }
        UnlinkedInstructionKind::HostRead {
            dst,
            root,
            target,
            dynamic_args,
            ..
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
            )
        }
        UnlinkedInstructionKind::HostWrite {
            root,
            target,
            dynamic_args,
            src,
            ..
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
            )
        }
        UnlinkedInstructionKind::HostMutate {
            root,
            target,
            dynamic_args,
            rhs,
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
            )
        }
        UnlinkedInstructionKind::HostRemove {
            root,
            target,
            dynamic_args,
            ..
        } => {
            verify_register(function, instruction_index, code, *root)?;
            verify_registers(function, instruction_index, code, dynamic_args)?;
            verify_host_target(
                function,
                instruction_index,
                code,
                *target,
                dynamic_args.len(),
            )
        }
        UnlinkedInstructionKind::HostCall {
            dst,
            root,
            target,
            dynamic_args,
            args,
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
            )
        }
        UnlinkedInstructionKind::Return { src } => {
            verify_register(function, instruction_index, code, *src)
        }
    }
}

fn is_unlinked_await_operation(operation: &UnlinkedInstructionKind) -> bool {
    matches!(
        operation,
        UnlinkedInstructionKind::CallNative { .. }
            | UnlinkedInstructionKind::CallFunction { .. }
            | UnlinkedInstructionKind::CallClosure { .. }
            | UnlinkedInstructionKind::CallDynamicMethod { .. }
            | UnlinkedInstructionKind::CallMethodId { .. }
            | UnlinkedInstructionKind::HostRead { .. }
            | UnlinkedInstructionKind::HostWrite { .. }
            | UnlinkedInstructionKind::HostMutate { .. }
            | UnlinkedInstructionKind::HostRemove { .. }
            | UnlinkedInstructionKind::HostCall { .. }
    )
}

fn verify_instruction_cache_site(
    function: &str,
    instruction_index: Option<usize>,
    code: &UnlinkedCodeObject,
    instruction: &UnlinkedInstruction,
    cache_scope: CacheIndexScope<'_>,
) -> Result<(), VerificationError> {
    let Some(policy) = instruction.kind.cache_site_policy() else {
        return Ok(());
    };
    match policy.storage {
        CacheSiteStorage::Sidecar => Ok(()),
        CacheSiteStorage::OptionalOperand => verify_optional_cache_site(
            function,
            instruction_index,
            code,
            instruction.kind.cache_site(),
            policy.kind,
            cache_scope,
        ),
        CacheSiteStorage::RequiredOperand => verify_cache_site(
            function,
            instruction_index,
            code,
            instruction
                .kind
                .cache_site()
                .expect("required cache policy exposes its operand"),
            policy.kind,
            cache_scope,
        ),
    }
}

fn verify_execution_units(
    function: &str,
    instruction: Option<usize>,
    units: u32,
) -> Result<(), VerificationError> {
    if units == 0 {
        return Err(VerificationError {
            function: function.to_owned(),
            instruction,
            kind: VerificationErrorKind::InvalidExecutionUnits { units },
        });
    }
    Ok(())
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
        crate::Constant::Unit => "()",
        crate::Constant::Bool(_) => "bool",
        crate::Constant::Char(_) => "char",
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
        let actual = instruction
            .kind
            .cache_site_policy()
            .map(|policy| policy.kind);
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
