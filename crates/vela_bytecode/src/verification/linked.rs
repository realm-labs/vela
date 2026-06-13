use vela_registry::DebugNameId;

use crate::linked::{
    Instruction, InstructionKind, LinkedCodeObject, LinkedMethodDispatchKind, LinkedProgram,
    MethodDispatchHandle, NativeHandle, ScriptFunctionHandle, TypeGuard, TypeGuardPlan, TypeHandle,
    VariantHandle,
};
use crate::{
    CacheSiteId, CacheSiteKind, CallArgument, Constant, ConstantId, FormatStringPart,
    HostTargetPlanId, InstructionOffset, Register,
};

use super::{VerificationError, VerificationErrorKind, constant_kind, error};

pub fn verify_linked_program(program: &LinkedProgram) -> Result<(), VerificationError> {
    let context = LinkedVerificationContext::new(program);
    for (_, native) in program.native_functions() {
        verify_linked_debug_name("<linked native>", None, &context, native.debug_name)?;
    }
    for (_, dispatch) in program.method_dispatches() {
        verify_linked_debug_name(
            "<linked method dispatch>",
            None,
            &context,
            dispatch.debug_name,
        )?;
        if let LinkedMethodDispatchKind::Script { function, .. } = dispatch.kind {
            verify_linked_function_handle("<linked method dispatch>", None, &context, function)?;
        }
    }
    for (_, ty) in program.types() {
        verify_linked_debug_name("<linked type>", None, &context, ty.debug_name)?;
    }
    for (_, variant) in program.variants() {
        verify_linked_debug_name("<linked variant>", None, &context, variant.debug_name)?;
        verify_linked_type_handle("<linked variant>", None, &context, variant.owner)?;
    }
    for (debug_name, function) in program.entry_points() {
        verify_linked_debug_name("<linked entry point>", None, &context, debug_name)?;
        verify_linked_function_handle("<linked entry point>", None, &context, function)?;
    }

    for (handle, function) in program.functions() {
        let name = linked_function_name(program, handle, function);
        verify_linked_code_object_with_context(function, &name, &context)?;
    }
    Ok(())
}

pub fn verify_linked_code_object(code: &LinkedCodeObject) -> Result<(), VerificationError> {
    let context = LinkedVerificationContext::for_code_only();
    verify_linked_code_object_with_context(code, "<linked code>", &context)
}

struct LinkedVerificationContext<'program> {
    program: Option<&'program LinkedProgram>,
    debug_name_count: usize,
    native_count: usize,
    function_count: usize,
    dispatch_count: usize,
    type_count: usize,
    variant_count: usize,
}

impl<'program> LinkedVerificationContext<'program> {
    fn new(program: &'program LinkedProgram) -> Self {
        Self {
            program: Some(program),
            debug_name_count: program.debug_names().len(),
            native_count: program.native_function_count(),
            function_count: program.function_count(),
            dispatch_count: program.method_dispatch_count(),
            type_count: program.type_count(),
            variant_count: program.variant_count(),
        }
    }

    fn for_code_only() -> Self {
        Self {
            program: None,
            debug_name_count: 0,
            native_count: usize::MAX,
            function_count: usize::MAX,
            dispatch_count: usize::MAX,
            type_count: usize::MAX,
            variant_count: usize::MAX,
        }
    }

    fn verify_debug_names(&self) -> bool {
        self.program.is_some()
    }
}

fn linked_function_name(
    program: &LinkedProgram,
    handle: ScriptFunctionHandle,
    code: &LinkedCodeObject,
) -> String {
    if debug_name_in_bounds(program.debug_names().len(), code.debug_name) {
        program.debug_name(code.debug_name).to_owned()
    } else {
        format!("<linked function {}>", handle.index())
    }
}

fn verify_linked_code_object_with_context(
    code: &LinkedCodeObject,
    function: &str,
    context: &LinkedVerificationContext<'_>,
) -> Result<(), VerificationError> {
    verify_linked_debug_name(function, None, context, code.debug_name)?;

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

    for param in &code.params {
        verify_linked_debug_name(function, None, context, *param)?;
    }
    for guard in &code.param_guards {
        verify_linked_parameter_guard(function, code, guard.parameter)?;
        verify_linked_type_guard_id(function, None, code, guard.guard)?;
    }
    if let Some(guard) = code.return_guard {
        verify_linked_type_guard_id(function, None, code, guard)?;
    }
    for slot in &code.frame.slots {
        verify_linked_debug_name(function, None, context, slot.name)?;
        verify_register_count(function, None, code.register_count, slot.register)?;
    }
    for guard in &code.type_guards {
        verify_linked_type_guard(function, context, guard)?;
    }
    for (index, instruction) in code.instructions.iter().enumerate() {
        verify_linked_instruction(function, code, index, instruction, context)?;
    }
    verify_linked_cache_site_layout(function, code)?;
    Ok(())
}

fn verify_linked_instruction(
    function: &str,
    code: &LinkedCodeObject,
    index: usize,
    instruction: &Instruction,
    context: &LinkedVerificationContext<'_>,
) -> Result<(), VerificationError> {
    let instruction_index = Some(index);
    match &instruction.kind {
        InstructionKind::LoadConst { dst, constant } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_constant(function, instruction_index, code, *constant)
        }
        InstructionKind::Move { dst, src }
        | InstructionKind::Not { dst, src }
        | InstructionKind::Truthy { dst, src }
        | InstructionKind::Negate { dst, src }
        | InstructionKind::TryPropagate { dst, src } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *src)
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
        | InstructionKind::GreaterEqual { dst, lhs, rhs }
        | InstructionKind::I64Add { dst, lhs, rhs }
        | InstructionKind::I64Sub { dst, lhs, rhs }
        | InstructionKind::I64Mul { dst, lhs, rhs }
        | InstructionKind::I64Rem { dst, lhs, rhs } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *lhs)?;
            verify_linked_register(function, instruction_index, code, *rhs)
        }
        InstructionKind::I64AddImm { dst, lhs, .. }
        | InstructionKind::I64SubImm { dst, lhs, .. }
        | InstructionKind::I64MulImm { dst, lhs, .. }
        | InstructionKind::I64CmpImm { dst, lhs, .. } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *lhs)
        }
        InstructionKind::I64RemImm { dst, lhs, imm } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *lhs)?;
            verify_linked_i64_rem_imm(function, instruction_index, *imm)
        }
        InstructionKind::I64CmpImmJumpIfFalse { lhs, target, .. } => {
            verify_linked_register(function, instruction_index, code, *lhs)?;
            verify_linked_jump(function, instruction_index, code, *target)
        }
        InstructionKind::BinaryIntLiteral { dst, value, .. }
        | InstructionKind::BinaryFloatLiteral { dst, value, .. } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *value)
        }
        InstructionKind::GuardType { src, guard } => {
            verify_linked_register(function, instruction_index, code, *src)?;
            verify_linked_type_guard_id(function, instruction_index, code, *guard)
        }
        InstructionKind::JumpIfFalse { condition, target } => {
            verify_linked_register(function, instruction_index, code, *condition)?;
            verify_linked_jump(function, instruction_index, code, *target)
        }
        InstructionKind::JumpIfNotMissing { value, target } => {
            verify_linked_register(function, instruction_index, code, *value)?;
            verify_linked_jump(function, instruction_index, code, *target)
        }
        InstructionKind::Jump { target } => {
            verify_linked_jump(function, instruction_index, code, *target)
        }
        InstructionKind::CallNative {
            dst,
            native,
            debug_name,
            cache_site,
            args,
        } => {
            if let Some(dst) = dst {
                verify_linked_register(function, instruction_index, code, *dst)?;
            }
            verify_linked_native_handle(function, instruction_index, context, *native)?;
            verify_linked_debug_name(function, instruction_index, context, *debug_name)?;
            verify_linked_registers(function, instruction_index, code, args)?;
            verify_linked_optional_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::NativeCall,
            )
        }
        InstructionKind::CallFunction {
            dst,
            function: callee,
            debug_name,
            args,
            mode: _,
        } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_function_handle(function, instruction_index, context, *callee)?;
            verify_linked_debug_name(function, instruction_index, context, *debug_name)?;
            verify_linked_call_arguments(function, instruction_index, code, args)
        }
        InstructionKind::MakeClosure {
            dst,
            function: closure,
            captures,
        } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_function_handle(function, instruction_index, context, *closure)?;
            verify_linked_registers(function, instruction_index, code, captures)
        }
        InstructionKind::CallClosure { dst, callee, args } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *callee)?;
            verify_linked_registers(function, instruction_index, code, args)
        }
        InstructionKind::CallMethod {
            dst,
            receiver,
            dispatch,
            debug_name,
            cache_site,
            args,
        } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *receiver)?;
            verify_linked_method_handle(function, instruction_index, context, *dispatch)?;
            verify_linked_debug_name(function, instruction_index, context, *debug_name)?;
            verify_linked_call_arguments(function, instruction_index, code, args)?;
            verify_linked_optional_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::MethodCall,
            )
        }
        InstructionKind::CallDynamicMethod {
            dst,
            receiver,
            method_name,
            cache_site,
            args,
        } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *receiver)?;
            verify_linked_debug_name(function, instruction_index, context, *method_name)?;
            verify_linked_dynamic_call_arguments(function, instruction_index, code, context, args)?;
            verify_linked_optional_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::MethodCall,
            )
        }
        InstructionKind::MakeArray { dst, elements } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_registers(function, instruction_index, code, elements)
        }
        InstructionKind::FormatString { dst, parts } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_format_string_parts(function, instruction_index, code, parts)
        }
        InstructionKind::MakeMap { dst, entries } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            for (constant, register) in entries {
                verify_linked_constant(function, instruction_index, code, *constant)?;
                verify_linked_register(function, instruction_index, code, *register)?;
            }
            Ok(())
        }
        InstructionKind::MakeRange {
            dst, start, end, ..
        } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *start)?;
            verify_linked_register(function, instruction_index, code, *end)
        }
        InstructionKind::MakeRecord { dst, ty, fields } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_type_handle(function, instruction_index, context, *ty)?;
            verify_linked_object_fields(function, instruction_index, code, context, fields)
        }
        InstructionKind::MakeEnum {
            dst,
            enum_ty,
            variant,
            fields,
        } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_type_handle(function, instruction_index, context, *enum_ty)?;
            verify_linked_variant_handle(function, instruction_index, context, *variant)?;
            verify_linked_object_fields(function, instruction_index, code, context, fields)
        }
        InstructionKind::GetRecordSlot {
            dst,
            record,
            debug_name,
            cache_site,
            ..
        } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *record)?;
            verify_linked_debug_name(function, instruction_index, context, *debug_name)?;
            verify_linked_optional_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::RecordFieldRead,
            )
        }
        InstructionKind::SetRecordSlot {
            record,
            debug_name,
            cache_site,
            src,
            ..
        } => {
            verify_linked_register(function, instruction_index, code, *record)?;
            verify_linked_debug_name(function, instruction_index, context, *debug_name)?;
            verify_linked_register(function, instruction_index, code, *src)?;
            verify_linked_optional_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::RecordFieldWrite,
            )
        }
        InstructionKind::GetEnumSlot {
            dst,
            value,
            debug_name,
            ..
        } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *value)?;
            verify_linked_debug_name(function, instruction_index, context, *debug_name)
        }
        InstructionKind::GetIndex { dst, base, index } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *base)?;
            verify_linked_register(function, instruction_index, code, *index)
        }
        InstructionKind::GetStringKeyIndex { dst, base, key } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *base)?;
            verify_linked_string_constant(function, instruction_index, code, *key)
        }
        InstructionKind::SetIndex { base, index, src } => {
            verify_linked_register(function, instruction_index, code, *base)?;
            verify_linked_register(function, instruction_index, code, *index)?;
            verify_linked_register(function, instruction_index, code, *src)
        }
        InstructionKind::SetStringKeyIndex { base, key, src } => {
            verify_linked_register(function, instruction_index, code, *base)?;
            verify_linked_string_constant(function, instruction_index, code, *key)?;
            verify_linked_register(function, instruction_index, code, *src)
        }
        InstructionKind::IterInit { dst, iterable } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *iterable)
        }
        InstructionKind::IterNext {
            iterator,
            dst,
            jump_if_done,
        } => {
            verify_linked_register(function, instruction_index, code, *iterator)?;
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_jump(function, instruction_index, code, *jump_if_done)
        }
        InstructionKind::RangeNext {
            cursor,
            end,
            done,
            dst,
            jump_if_done,
            ..
        }
        | InstructionKind::I64RangeNext {
            cursor,
            end,
            done,
            dst,
            jump_if_done,
            ..
        } => {
            verify_linked_register(function, instruction_index, code, *cursor)?;
            verify_linked_register(function, instruction_index, code, *end)?;
            verify_linked_register(function, instruction_index, code, *done)?;
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_jump(function, instruction_index, code, *jump_if_done)
        }
        InstructionKind::EnumTagEqual {
            dst,
            value,
            enum_ty,
            variant,
        } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *value)?;
            verify_linked_type_handle(function, instruction_index, context, *enum_ty)?;
            verify_linked_variant_handle(function, instruction_index, context, *variant)
        }
        InstructionKind::LoadGlobal {
            dst,
            debug_name,
            cache_site,
            ..
        } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_debug_name(function, instruction_index, context, *debug_name)?;
            verify_linked_optional_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::GlobalRead,
            )
        }
        InstructionKind::HostRead {
            dst,
            root,
            target,
            dynamic_args,
            cache_site,
        } => {
            verify_linked_register(function, instruction_index, code, *dst)?;
            verify_linked_register(function, instruction_index, code, *root)?;
            verify_linked_registers(function, instruction_index, code, dynamic_args)?;
            verify_linked_host_target(
                function,
                instruction_index,
                code,
                *target,
                dynamic_args.len(),
            )?;
            verify_linked_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::HostPathRead,
            )
        }
        InstructionKind::HostWrite {
            root,
            target,
            dynamic_args,
            src,
            cache_site,
        } => {
            verify_linked_register(function, instruction_index, code, *root)?;
            verify_linked_register(function, instruction_index, code, *src)?;
            verify_linked_registers(function, instruction_index, code, dynamic_args)?;
            verify_linked_host_target(
                function,
                instruction_index,
                code,
                *target,
                dynamic_args.len(),
            )?;
            verify_linked_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::HostPathWrite,
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
            verify_linked_register(function, instruction_index, code, *root)?;
            verify_linked_register(function, instruction_index, code, *rhs)?;
            verify_linked_registers(function, instruction_index, code, dynamic_args)?;
            verify_linked_host_target(
                function,
                instruction_index,
                code,
                *target,
                dynamic_args.len(),
            )?;
            verify_linked_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::HostPathMutate,
            )
        }
        InstructionKind::HostRemove {
            root,
            target,
            dynamic_args,
            cache_site,
        } => {
            verify_linked_register(function, instruction_index, code, *root)?;
            verify_linked_registers(function, instruction_index, code, dynamic_args)?;
            verify_linked_host_target(
                function,
                instruction_index,
                code,
                *target,
                dynamic_args.len(),
            )?;
            verify_linked_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::HostPathRemove,
            )
        }
        InstructionKind::HostCall {
            dst,
            root,
            target,
            dynamic_args,
            method,
            debug_name,
            args,
            cache_site,
        } => {
            if let Some(dst) = dst {
                verify_linked_register(function, instruction_index, code, *dst)?;
            }
            verify_linked_register(function, instruction_index, code, *root)?;
            verify_linked_registers(function, instruction_index, code, dynamic_args)?;
            verify_linked_registers(function, instruction_index, code, args)?;
            verify_linked_host_target(
                function,
                instruction_index,
                code,
                *target,
                dynamic_args.len(),
            )?;
            verify_linked_method_handle(function, instruction_index, context, *method)?;
            verify_linked_debug_name(function, instruction_index, context, *debug_name)?;
            verify_linked_cache_site(
                function,
                instruction_index,
                code,
                *cache_site,
                CacheSiteKind::HostPathCall,
            )
        }
        InstructionKind::Return { src } => {
            verify_linked_register(function, instruction_index, code, *src)
        }
    }
}

fn verify_linked_debug_name(
    function: &str,
    instruction: Option<usize>,
    context: &LinkedVerificationContext<'_>,
    debug_name: DebugNameId,
) -> Result<(), VerificationError> {
    if !context.verify_debug_names() || debug_name_in_bounds(context.debug_name_count, debug_name) {
        Ok(())
    } else {
        Err(error(
            function,
            instruction,
            VerificationErrorKind::DebugNameOutOfBounds {
                debug_name,
                debug_name_count: context.debug_name_count,
            },
        ))
    }
}

fn debug_name_in_bounds(debug_name_count: usize, debug_name: DebugNameId) -> bool {
    usize::try_from(debug_name.get()).is_ok_and(|index| index < debug_name_count)
}

fn verify_linked_native_handle(
    function: &str,
    instruction: Option<usize>,
    context: &LinkedVerificationContext<'_>,
    handle: NativeHandle,
) -> Result<(), VerificationError> {
    if handle.index() < context.native_count {
        Ok(())
    } else {
        Err(error(
            function,
            instruction,
            VerificationErrorKind::NativeHandleOutOfBounds {
                handle,
                native_count: context.native_count,
            },
        ))
    }
}

fn verify_linked_function_handle(
    function: &str,
    instruction: Option<usize>,
    context: &LinkedVerificationContext<'_>,
    handle: ScriptFunctionHandle,
) -> Result<(), VerificationError> {
    if handle.index() < context.function_count {
        Ok(())
    } else {
        Err(error(
            function,
            instruction,
            VerificationErrorKind::ScriptFunctionHandleOutOfBounds {
                handle,
                function_count: context.function_count,
            },
        ))
    }
}

fn verify_linked_method_handle(
    function: &str,
    instruction: Option<usize>,
    context: &LinkedVerificationContext<'_>,
    handle: MethodDispatchHandle,
) -> Result<(), VerificationError> {
    if handle.index() < context.dispatch_count {
        Ok(())
    } else {
        Err(error(
            function,
            instruction,
            VerificationErrorKind::MethodDispatchHandleOutOfBounds {
                handle,
                dispatch_count: context.dispatch_count,
            },
        ))
    }
}

fn verify_linked_type_handle(
    function: &str,
    instruction: Option<usize>,
    context: &LinkedVerificationContext<'_>,
    handle: TypeHandle,
) -> Result<(), VerificationError> {
    if handle.index() < context.type_count {
        Ok(())
    } else {
        Err(error(
            function,
            instruction,
            VerificationErrorKind::TypeHandleOutOfBounds {
                handle,
                type_count: context.type_count,
            },
        ))
    }
}

fn verify_linked_variant_handle(
    function: &str,
    instruction: Option<usize>,
    context: &LinkedVerificationContext<'_>,
    handle: VariantHandle,
) -> Result<(), VerificationError> {
    if handle.index() < context.variant_count {
        Ok(())
    } else {
        Err(error(
            function,
            instruction,
            VerificationErrorKind::VariantHandleOutOfBounds {
                handle,
                variant_count: context.variant_count,
            },
        ))
    }
}

fn verify_linked_type_guard(
    function: &str,
    context: &LinkedVerificationContext<'_>,
    guard: &TypeGuard,
) -> Result<(), VerificationError> {
    verify_linked_debug_name(function, None, context, guard.context.debug_name)?;
    match guard.plan {
        TypeGuardPlan::Primitive(_) => Ok(()),
        TypeGuardPlan::Type(handle) | TypeGuardPlan::HostType(handle) => {
            verify_linked_type_handle(function, None, context, handle)
        }
        TypeGuardPlan::Variant(handle) => {
            verify_linked_variant_handle(function, None, context, handle)
        }
        TypeGuardPlan::Shape { ty, .. } => verify_linked_type_handle(function, None, context, ty),
    }
}

fn verify_linked_parameter_guard(
    function: &str,
    code: &LinkedCodeObject,
    parameter: u16,
) -> Result<(), VerificationError> {
    if usize::from(parameter) < code.params.len() {
        Ok(())
    } else {
        Err(error(
            function,
            None,
            VerificationErrorKind::ParameterGuardOutOfBounds {
                parameter,
                parameter_count: code.params.len(),
            },
        ))
    }
}

fn verify_linked_type_guard_id(
    function: &str,
    instruction: Option<usize>,
    code: &LinkedCodeObject,
    guard: crate::TypeGuardPlanId,
) -> Result<(), VerificationError> {
    if guard.index() < code.type_guards.len() {
        Ok(())
    } else {
        Err(error(
            function,
            instruction,
            VerificationErrorKind::TypeGuardPlanOutOfBounds {
                guard,
                guard_count: code.type_guards.len(),
            },
        ))
    }
}

fn verify_linked_registers(
    function: &str,
    instruction: Option<usize>,
    code: &LinkedCodeObject,
    registers: &[Register],
) -> Result<(), VerificationError> {
    for register in registers {
        verify_linked_register(function, instruction, code, *register)?;
    }
    Ok(())
}

fn verify_linked_format_string_parts(
    function: &str,
    instruction: Option<usize>,
    code: &LinkedCodeObject,
    parts: &[FormatStringPart],
) -> Result<(), VerificationError> {
    for part in parts {
        match part {
            FormatStringPart::Text(constant) => {
                verify_linked_string_constant(function, instruction, code, *constant)?;
            }
            FormatStringPart::Value(register) => {
                verify_linked_register(function, instruction, code, *register)?;
            }
        }
    }
    Ok(())
}

fn verify_linked_i64_rem_imm(
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

fn verify_linked_object_fields(
    function: &str,
    instruction: Option<usize>,
    code: &LinkedCodeObject,
    context: &LinkedVerificationContext<'_>,
    fields: &[(crate::FieldSlot, DebugNameId, Register)],
) -> Result<(), VerificationError> {
    for (_, debug_name, register) in fields {
        verify_linked_debug_name(function, instruction, context, *debug_name)?;
        verify_linked_register(function, instruction, code, *register)?;
    }
    Ok(())
}

fn verify_linked_call_arguments(
    function: &str,
    instruction: Option<usize>,
    code: &LinkedCodeObject,
    args: &[CallArgument],
) -> Result<(), VerificationError> {
    for arg in args {
        if let CallArgument::Register(register) = arg {
            verify_linked_register(function, instruction, code, *register)?;
        }
    }
    Ok(())
}

fn verify_linked_dynamic_call_arguments(
    function: &str,
    instruction: Option<usize>,
    code: &LinkedCodeObject,
    context: &LinkedVerificationContext<'_>,
    args: &[crate::linked::DynamicCallArgumentLinked],
) -> Result<(), VerificationError> {
    for arg in args {
        if let Some(name) = arg.name {
            verify_linked_debug_name(function, instruction, context, name)?;
        }
        verify_linked_register(function, instruction, code, arg.value)?;
    }
    Ok(())
}

fn verify_linked_register(
    function: &str,
    instruction: Option<usize>,
    code: &LinkedCodeObject,
    register: Register,
) -> Result<(), VerificationError> {
    verify_register_count(function, instruction, code.register_count, register)
}

fn verify_register_count(
    function: &str,
    instruction: Option<usize>,
    register_count: u16,
    register: Register,
) -> Result<(), VerificationError> {
    if register.0 < register_count {
        Ok(())
    } else {
        Err(error(
            function,
            instruction,
            VerificationErrorKind::RegisterOutOfBounds {
                register,
                register_count,
            },
        ))
    }
}

fn verify_linked_constant(
    function: &str,
    instruction: Option<usize>,
    code: &LinkedCodeObject,
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

fn verify_linked_string_constant(
    function: &str,
    instruction: Option<usize>,
    code: &LinkedCodeObject,
    constant: ConstantId,
) -> Result<(), VerificationError> {
    verify_linked_constant(function, instruction, code, constant)?;
    match &code.constants[constant.0] {
        Constant::String(_) => Ok(()),
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

fn verify_linked_jump(
    function: &str,
    instruction: Option<usize>,
    code: &LinkedCodeObject,
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

fn verify_linked_optional_cache_site(
    function: &str,
    instruction: Option<usize>,
    code: &LinkedCodeObject,
    site: Option<CacheSiteId>,
    expected: CacheSiteKind,
) -> Result<(), VerificationError> {
    let Some(site) = site else {
        return Ok(());
    };
    verify_linked_cache_site(function, instruction, code, site, expected)
}

fn verify_linked_cache_site(
    function: &str,
    instruction: Option<usize>,
    code: &LinkedCodeObject,
    site: CacheSiteId,
    expected: CacheSiteKind,
) -> Result<(), VerificationError> {
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

fn verify_linked_cache_site_layout(
    function: &str,
    code: &LinkedCodeObject,
) -> Result<(), VerificationError> {
    for (index, site) in code.cache_sites.sites().iter().enumerate() {
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
        let actual = linked_instruction_cache_site_kind(&instruction.kind);
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

fn linked_instruction_cache_site_kind(kind: &InstructionKind) -> Option<CacheSiteKind> {
    match kind {
        InstructionKind::LoadGlobal { .. } => Some(CacheSiteKind::GlobalRead),
        InstructionKind::CallNative { .. } => Some(CacheSiteKind::NativeCall),
        InstructionKind::CallMethod { .. } | InstructionKind::CallDynamicMethod { .. } => {
            Some(CacheSiteKind::MethodCall)
        }
        InstructionKind::GetRecordSlot { .. } => Some(CacheSiteKind::RecordFieldRead),
        InstructionKind::SetRecordSlot { .. } => Some(CacheSiteKind::RecordFieldWrite),
        InstructionKind::HostRead { .. } => Some(CacheSiteKind::HostPathRead),
        InstructionKind::HostWrite { .. } => Some(CacheSiteKind::HostPathWrite),
        InstructionKind::HostMutate { .. } => Some(CacheSiteKind::HostPathMutate),
        InstructionKind::HostRemove { .. } => Some(CacheSiteKind::HostPathRemove),
        InstructionKind::HostCall { .. } => Some(CacheSiteKind::HostPathCall),
        _ => None,
    }
}

fn verify_linked_host_target(
    function: &str,
    instruction: Option<usize>,
    code: &LinkedCodeObject,
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
    verify_host_target_dynamic_args(function, instruction, plan, dynamic_arg_count)
}

fn verify_host_target_dynamic_args(
    function: &str,
    instruction: Option<usize>,
    plan: &vela_host::target::HostTargetPlan,
    dynamic_arg_count: usize,
) -> Result<(), VerificationError> {
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
