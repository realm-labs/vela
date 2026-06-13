use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use vela_common::HostMethodId;
use vela_def::{DefPath, FunctionId, MethodId, TypeId, VariantId};
use vela_registry::{Def, DefinitionRegistry};

use crate::linked::{
    DynamicCallArgumentLinked, GuardContext, Instruction, InstructionKind, LinkedCodeObject,
    LinkedFrameDebugInfo, LinkedFrameSlotInfo, LinkedMethodDispatch, LinkedMethodDispatchKind,
    LinkedNativeFunction, LinkedProgram, LinkedType, LinkedVariant, TypeGuard, TypeGuardPlan,
};
use crate::{
    CacheSiteId, CacheSiteKind, Constant, FieldSlot, FunctionIndex, HostTargetPlanId,
    InstructionOffset, MethodDispatchHandle, NativeHandle, ScriptFunctionHandle, TypeHandle,
    UnlinkedCodeObject, UnlinkedInstruction, UnlinkedInstructionKind, UnlinkedProgram,
    UnlinkedTypeGuard, UnlinkedTypeGuardPlan, VariantHandle, function_id_for_script_name,
};

#[derive(Clone, Debug, Default)]
pub struct Linker<'registry> {
    registry: Option<&'registry DefinitionRegistry>,
    native_implementations: BTreeSet<FunctionId>,
}

impl<'registry> Linker<'registry> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_registry(registry: &'registry DefinitionRegistry) -> Self {
        Self {
            registry: Some(registry),
            native_implementations: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn with_native_implementation(mut self, id: FunctionId) -> Self {
        self.native_implementations.insert(id);
        self
    }

    pub fn add_native_implementation(&mut self, id: FunctionId) {
        self.native_implementations.insert(id);
    }

    pub fn link_program(&self, program: &UnlinkedProgram) -> Result<LinkedProgram, LinkError> {
        LinkContext::new(self, program).link_program(program)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinkError {
    UnresolvedNative {
        name: String,
        id: FunctionId,
    },
    MissingNativeImplementation {
        name: String,
        id: FunctionId,
    },
    MissingScriptFunction {
        name: String,
        id: FunctionId,
    },
    InvalidNestedFunction {
        function: String,
        index: FunctionIndex,
    },
    MissingMethodDefinition {
        method: String,
        id: MethodId,
    },
    MissingGlobal {
        function: String,
        global: String,
    },
    InvalidHostTarget {
        function: String,
        target: HostTargetPlanId,
    },
    UnresolvedType {
        name: String,
    },
    UnresolvedVariant {
        enum_name: String,
        variant: String,
    },
    UnresolvedRecordField {
        function: String,
        field: String,
    },
    UnresolvedEnumField {
        function: String,
        field: String,
    },
}

impl fmt::Display for LinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedNative { name, id } => {
                write!(formatter, "unresolved native function {name} ({id:?})")
            }
            Self::MissingNativeImplementation { name, id } => {
                write!(
                    formatter,
                    "missing native implementation for {name} ({id:?})"
                )
            }
            Self::MissingScriptFunction { name, id } => {
                write!(formatter, "missing script function {name} ({id:?})")
            }
            Self::InvalidNestedFunction { function, index } => {
                write!(
                    formatter,
                    "function {function} references missing nested function {index:?}"
                )
            }
            Self::MissingMethodDefinition { method, id } => {
                write!(formatter, "missing method definition for {method} ({id:?})")
            }
            Self::MissingGlobal { function, global } => {
                write!(
                    formatter,
                    "function {function} references missing global {global}"
                )
            }
            Self::InvalidHostTarget { function, target } => {
                write!(
                    formatter,
                    "function {function} references missing host target {target:?}"
                )
            }
            Self::UnresolvedType { name } => {
                write!(formatter, "unresolved type {name}")
            }
            Self::UnresolvedVariant { enum_name, variant } => {
                write!(formatter, "unresolved variant {enum_name}::{variant}")
            }
            Self::UnresolvedRecordField { function, field } => {
                write!(
                    formatter,
                    "function {function} contains unresolved record field {field}"
                )
            }
            Self::UnresolvedEnumField { function, field } => {
                write!(
                    formatter,
                    "function {function} contains unresolved enum field {field}"
                )
            }
        }
    }
}

impl Error for LinkError {}

struct LinkContext<'linker, 'registry> {
    linker: &'linker Linker<'registry>,
    linked: LinkedProgram,
    script_functions_by_name: BTreeMap<String, ScriptFunctionHandle>,
    script_functions_by_id: BTreeMap<FunctionId, ScriptFunctionHandle>,
    script_methods_by_id: BTreeMap<MethodId, ScriptFunctionHandle>,
    native_handles: BTreeMap<FunctionId, NativeHandle>,
    method_handles: BTreeMap<MethodDispatchKey, MethodDispatchHandle>,
    type_handles: BTreeMap<TypeId, TypeHandle>,
    variant_handles: BTreeMap<VariantId, VariantHandle>,
    next_function_index: usize,
    extra_functions: Vec<LinkedCodeObject>,
}

struct LinkInstructionContext<'a> {
    program: &'a UnlinkedProgram,
    code: &'a UnlinkedCodeObject,
    nested_handles: &'a [ScriptFunctionHandle],
    host_target_map: &'a [HostTargetPlanId],
    linked_code: &'a mut LinkedCodeObject,
    instruction_offset: InstructionOffset,
}

impl<'linker, 'registry> LinkContext<'linker, 'registry> {
    fn new(linker: &'linker Linker<'registry>, program: &UnlinkedProgram) -> Self {
        let mut script_functions_by_name = BTreeMap::new();
        let mut script_functions_by_id = BTreeMap::new();
        for (index, name) in program.function_names().enumerate() {
            let handle = ScriptFunctionHandle::new(index);
            script_functions_by_name.insert(name.to_owned(), handle);
            script_functions_by_id.insert(function_id_for_script_name(name), handle);
        }

        let mut script_methods_by_id = BTreeMap::new();
        for (_, _, method) in program.script_methods().methods() {
            if let Some(function) = script_functions_by_name.get(&method.function) {
                script_methods_by_id.insert(method.id, *function);
            }
        }

        Self {
            linker,
            linked: LinkedProgram::new(),
            script_functions_by_name,
            script_functions_by_id,
            script_methods_by_id,
            native_handles: BTreeMap::new(),
            method_handles: BTreeMap::new(),
            type_handles: BTreeMap::new(),
            variant_handles: BTreeMap::new(),
            next_function_index: program.function_count(),
            extra_functions: Vec::new(),
        }
    }

    fn link_program(mut self, program: &UnlinkedProgram) -> Result<LinkedProgram, LinkError> {
        let mut top_level = Vec::with_capacity(program.function_count());
        for code in program.functions() {
            top_level.push(self.link_code(program, code)?);
        }

        for code in top_level {
            self.linked.push_function(code);
        }
        self.link_script_method_dispatches(program)?;
        for code in self.extra_functions {
            self.linked.push_function(code);
        }

        for name in program.function_names() {
            let debug_name = self.linked.intern_debug_name(name.to_owned());
            if let Some(function) = self.script_functions_by_name.get(name).copied() {
                self.linked.set_entry_point(debug_name, function);
            }
        }

        Ok(self.linked)
    }

    fn link_script_method_dispatches(
        &mut self,
        program: &UnlinkedProgram,
    ) -> Result<(), LinkError> {
        for (type_name, method_name, method) in program.script_methods().methods() {
            let Some(function) = self.script_functions_by_name.get(&method.function).copied()
            else {
                continue;
            };
            let dispatch = self.intern_method_dispatch(
                MethodDispatchKey::Script(method.id, function),
                method_name.to_owned(),
            )?;
            self.linked
                .insert_script_method_dispatch(type_name, method_name, dispatch);
        }
        Ok(())
    }

    fn link_code(
        &mut self,
        program: &UnlinkedProgram,
        code: &UnlinkedCodeObject,
    ) -> Result<LinkedCodeObject, LinkError> {
        let debug_name = self.linked.intern_debug_name(code.name.clone());
        let params = code
            .params
            .iter()
            .map(|param| self.linked.intern_debug_name(param.clone()))
            .collect::<Vec<_>>();
        let frame = self.link_frame(&code.frame);

        let mut linked = LinkedCodeObject::new(debug_name, code.register_count)
            .with_params(params)
            .with_param_defaults(code.param_defaults.clone())
            .with_capture_count(code.capture_count);
        linked.frame = frame;
        linked.cache_sites = code.cache_sites.clone();
        linked.constants = code.constants.clone();
        for guard in &code.param_guards {
            let linked_guard = self.link_type_guard(guard.guard.clone(), &mut linked)?;
            linked.push_param_guard(guard.parameter, linked_guard);
        }
        if let Some(guard) = code.return_guard.clone() {
            let linked_guard = self.link_type_guard(guard, &mut linked)?;
            linked.set_return_guard(linked_guard);
        }
        let host_target_map = code
            .host_targets
            .iter()
            .cloned()
            .map(|target| linked.intern_host_target(target))
            .collect::<Vec<_>>();

        let mut nested_handles = Vec::with_capacity(code.nested_functions.len());
        for nested in &code.nested_functions {
            let linked_nested = self.link_code(program, nested)?;
            let handle = ScriptFunctionHandle::new(self.next_function_index);
            self.next_function_index += 1;
            nested_handles.push(handle);
            self.extra_functions.push(linked_nested);
        }

        for (offset, instruction) in code.instructions.iter().enumerate() {
            let instruction = self.link_instruction(
                LinkInstructionContext {
                    program,
                    code,
                    nested_handles: &nested_handles,
                    host_target_map: &host_target_map,
                    linked_code: &mut linked,
                    instruction_offset: InstructionOffset(offset),
                },
                instruction,
            )?;
            linked.push_instruction(instruction);
        }

        Ok(linked)
    }

    fn link_frame(&mut self, frame: &crate::FrameDebugInfo) -> LinkedFrameDebugInfo {
        let mut linked = LinkedFrameDebugInfo::default();
        for slot in &frame.slots {
            linked.push_slot(LinkedFrameSlotInfo::new(
                self.linked.intern_debug_name(slot.name.clone()),
                slot.register,
                slot.span,
            ));
        }
        linked
    }

    fn link_instruction(
        &mut self,
        context: LinkInstructionContext<'_>,
        instruction: &UnlinkedInstruction,
    ) -> Result<Instruction, LinkError> {
        let program = context.program;
        let code = context.code;
        let nested_handles = context.nested_handles;
        let host_target_map = context.host_target_map;
        let linked_code = context.linked_code;
        let instruction_offset = context.instruction_offset;
        let kind = match &instruction.kind {
            UnlinkedInstructionKind::LoadConst { dst, constant } => InstructionKind::LoadConst {
                dst: *dst,
                constant: *constant,
            },
            UnlinkedInstructionKind::Move { dst, src } => InstructionKind::Move {
                dst: *dst,
                src: *src,
            },
            UnlinkedInstructionKind::Not { dst, src } => InstructionKind::Not {
                dst: *dst,
                src: *src,
            },
            UnlinkedInstructionKind::Truthy { dst, src } => InstructionKind::Truthy {
                dst: *dst,
                src: *src,
            },
            UnlinkedInstructionKind::Negate { dst, src } => InstructionKind::Negate {
                dst: *dst,
                src: *src,
            },
            UnlinkedInstructionKind::Add { dst, lhs, rhs } => InstructionKind::Add {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::Sub { dst, lhs, rhs } => InstructionKind::Sub {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::Mul { dst, lhs, rhs } => InstructionKind::Mul {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::Div { dst, lhs, rhs } => InstructionKind::Div {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::Rem { dst, lhs, rhs } => InstructionKind::Rem {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::Equal { dst, lhs, rhs } => InstructionKind::Equal {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::NotEqual { dst, lhs, rhs } => InstructionKind::NotEqual {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::Less { dst, lhs, rhs } => InstructionKind::Less {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::LessEqual { dst, lhs, rhs } => InstructionKind::LessEqual {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::Greater { dst, lhs, rhs } => InstructionKind::Greater {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::GreaterEqual { dst, lhs, rhs } => {
                InstructionKind::GreaterEqual {
                    dst: *dst,
                    lhs: *lhs,
                    rhs: *rhs,
                }
            }
            UnlinkedInstructionKind::I64Add { dst, lhs, rhs } => InstructionKind::I64Add {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::I64Sub { dst, lhs, rhs } => InstructionKind::I64Sub {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::I64Mul { dst, lhs, rhs } => InstructionKind::I64Mul {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::I64Rem { dst, lhs, rhs } => InstructionKind::I64Rem {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::I64AddImm { dst, lhs, imm } => InstructionKind::I64AddImm {
                dst: *dst,
                lhs: *lhs,
                imm: *imm,
            },
            UnlinkedInstructionKind::I64SubImm { dst, lhs, imm } => InstructionKind::I64SubImm {
                dst: *dst,
                lhs: *lhs,
                imm: *imm,
            },
            UnlinkedInstructionKind::I64MulImm { dst, lhs, imm } => InstructionKind::I64MulImm {
                dst: *dst,
                lhs: *lhs,
                imm: *imm,
            },
            UnlinkedInstructionKind::I64RemImm { dst, lhs, imm } => InstructionKind::I64RemImm {
                dst: *dst,
                lhs: *lhs,
                imm: *imm,
            },
            UnlinkedInstructionKind::I64CmpImm { dst, op, lhs, imm } => {
                InstructionKind::I64CmpImm {
                    dst: *dst,
                    op: *op,
                    lhs: *lhs,
                    imm: *imm,
                }
            }
            UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
                op,
                lhs,
                imm,
                target,
            } => InstructionKind::I64CmpImmJumpIfFalse {
                op: *op,
                lhs: *lhs,
                imm: *imm,
                target: *target,
            },
            UnlinkedInstructionKind::BinaryIntLiteral {
                dst,
                op,
                value,
                literal,
                side,
            } => InstructionKind::BinaryIntLiteral {
                dst: *dst,
                op: *op,
                value: *value,
                literal: literal.clone(),
                side: *side,
            },
            UnlinkedInstructionKind::BinaryFloatLiteral {
                dst,
                op,
                value,
                literal,
                side,
            } => InstructionKind::BinaryFloatLiteral {
                dst: *dst,
                op: *op,
                value: *value,
                literal: literal.clone(),
                side: *side,
            },
            UnlinkedInstructionKind::JumpIfFalse { condition, target } => {
                InstructionKind::JumpIfFalse {
                    condition: *condition,
                    target: *target,
                }
            }
            UnlinkedInstructionKind::JumpIfNotMissing { value, target } => {
                InstructionKind::JumpIfNotMissing {
                    value: *value,
                    target: *target,
                }
            }
            UnlinkedInstructionKind::Jump { target } => InstructionKind::Jump { target: *target },
            UnlinkedInstructionKind::CallNative {
                dst,
                name,
                native,
                cache_site,
                args,
            } => {
                let native = self.link_native(name, *native)?;
                let debug_name = self.linked.intern_debug_name(name.clone());
                InstructionKind::CallNative {
                    dst: *dst,
                    native,
                    debug_name,
                    cache_site: *cache_site,
                    args: args.clone(),
                }
            }
            UnlinkedInstructionKind::CallFunction {
                dst,
                target,
                name,
                mode,
                args,
            } => {
                let function = self.resolve_script_function(*target, name)?;
                let debug_name = self.linked.intern_debug_name(name.clone());
                InstructionKind::CallFunction {
                    dst: *dst,
                    function,
                    debug_name,
                    mode: *mode,
                    args: args.clone(),
                }
            }
            UnlinkedInstructionKind::MakeClosure {
                dst,
                function,
                captures,
            } => {
                let function = nested_handles.get(function.0).copied().ok_or_else(|| {
                    LinkError::InvalidNestedFunction {
                        function: code.name.clone(),
                        index: *function,
                    }
                })?;
                InstructionKind::MakeClosure {
                    dst: *dst,
                    function,
                    captures: captures.clone(),
                }
            }
            UnlinkedInstructionKind::CallClosure { dst, callee, args } => {
                InstructionKind::CallClosure {
                    dst: *dst,
                    callee: *callee,
                    args: args.clone(),
                }
            }
            UnlinkedInstructionKind::CallDynamicMethod {
                dst,
                receiver,
                method,
                args,
            } => {
                let method_name = self.linked.intern_debug_name(method.clone());
                let args = args
                    .iter()
                    .map(|arg| DynamicCallArgumentLinked {
                        name: arg
                            .name
                            .as_ref()
                            .map(|name| self.linked.intern_debug_name(name.clone())),
                        value: arg.value,
                    })
                    .collect();
                InstructionKind::CallDynamicMethod {
                    dst: *dst,
                    receiver: *receiver,
                    method_name,
                    cache_site: cache_site_at(code, instruction_offset, CacheSiteKind::MethodCall),
                    args,
                }
            }
            UnlinkedInstructionKind::CallMethodId {
                dst,
                receiver,
                method,
                method_id,
                args,
            } => {
                let dispatch = self.link_method_dispatch(method, *method_id)?;
                let debug_name = self.linked.intern_debug_name(method.clone());
                InstructionKind::CallMethod {
                    dst: *dst,
                    receiver: *receiver,
                    dispatch,
                    debug_name,
                    cache_site: cache_site_at(code, instruction_offset, CacheSiteKind::MethodCall),
                    args: args.clone(),
                }
            }
            UnlinkedInstructionKind::TryPropagate { dst, src } => InstructionKind::TryPropagate {
                dst: *dst,
                src: *src,
            },
            UnlinkedInstructionKind::MakeArray { dst, elements } => InstructionKind::MakeArray {
                dst: *dst,
                elements: elements.clone(),
            },
            UnlinkedInstructionKind::MakeMap { dst, entries } => {
                let entries = entries
                    .iter()
                    .map(|(key, value)| {
                        let key = linked_code.push_constant(Constant::String(key.clone()));
                        (key, *value)
                    })
                    .collect();
                InstructionKind::MakeMap { dst: *dst, entries }
            }
            UnlinkedInstructionKind::MakeRange {
                dst,
                start,
                end,
                inclusive,
            } => InstructionKind::MakeRange {
                dst: *dst,
                start: *start,
                end: *end,
                inclusive: *inclusive,
            },
            UnlinkedInstructionKind::MakeRecord {
                dst,
                type_name,
                fields,
            } => {
                let ty = self.link_type(type_name)?;
                let field_slots = sorted_field_slots(fields.iter().map(|(field, _)| field));
                let fields = fields
                    .iter()
                    .map(|(field, register)| {
                        (
                            FieldSlot::new(field_slots[field]),
                            self.linked.intern_debug_name(field.clone()),
                            *register,
                        )
                    })
                    .collect();
                InstructionKind::MakeRecord {
                    dst: *dst,
                    ty,
                    fields,
                }
            }
            UnlinkedInstructionKind::MakeEnum {
                dst,
                enum_name,
                variant,
                fields,
            } => {
                let enum_ty = self.link_type(enum_name)?;
                let variant = self.link_variant(enum_name, variant, enum_ty)?;
                let field_slots = sorted_field_slots(fields.iter().map(|(field, _)| field));
                let fields = fields
                    .iter()
                    .map(|(field, register)| {
                        (
                            FieldSlot::new(field_slots[field]),
                            self.linked.intern_debug_name(field.clone()),
                            *register,
                        )
                    })
                    .collect();
                InstructionKind::MakeEnum {
                    dst: *dst,
                    enum_ty,
                    variant,
                    fields,
                }
            }
            UnlinkedInstructionKind::GetRecordField { field, .. } => {
                return Err(LinkError::UnresolvedRecordField {
                    function: code.name.clone(),
                    field: field.clone(),
                });
            }
            UnlinkedInstructionKind::GetRecordSlot {
                dst,
                record,
                field,
                slot,
            } => InstructionKind::GetRecordSlot {
                dst: *dst,
                record: *record,
                field: FieldSlot::new(*slot),
                debug_name: self.linked.intern_debug_name(field.clone()),
                cache_site: cache_site_at(code, instruction_offset, CacheSiteKind::RecordFieldRead),
            },
            UnlinkedInstructionKind::SetRecordField { field, .. } => {
                return Err(LinkError::UnresolvedRecordField {
                    function: code.name.clone(),
                    field: field.clone(),
                });
            }
            UnlinkedInstructionKind::SetRecordSlot {
                record,
                field,
                slot,
                src,
            } => InstructionKind::SetRecordSlot {
                record: *record,
                field: FieldSlot::new(*slot),
                debug_name: self.linked.intern_debug_name(field.clone()),
                cache_site: cache_site_at(
                    code,
                    instruction_offset,
                    CacheSiteKind::RecordFieldWrite,
                ),
                src: *src,
            },
            UnlinkedInstructionKind::GetEnumField { field, .. } => {
                return Err(LinkError::UnresolvedEnumField {
                    function: code.name.clone(),
                    field: field.clone(),
                });
            }
            UnlinkedInstructionKind::GetEnumSlot {
                dst,
                value,
                field,
                slot,
            } => InstructionKind::GetEnumSlot {
                dst: *dst,
                value: *value,
                field: FieldSlot::new(*slot),
                debug_name: self.linked.intern_debug_name(field.clone()),
            },
            UnlinkedInstructionKind::GetIndex { dst, base, index } => InstructionKind::GetIndex {
                dst: *dst,
                base: *base,
                index: *index,
            },
            UnlinkedInstructionKind::GetStringKeyIndex { dst, base, key } => {
                InstructionKind::GetStringKeyIndex {
                    dst: *dst,
                    base: *base,
                    key: self.linked.intern_debug_name(key.clone()),
                }
            }
            UnlinkedInstructionKind::SetIndex { base, index, src } => InstructionKind::SetIndex {
                base: *base,
                index: *index,
                src: *src,
            },
            UnlinkedInstructionKind::SetStringKeyIndex { base, key, src } => {
                InstructionKind::SetStringKeyIndex {
                    base: *base,
                    key: self.linked.intern_debug_name(key.clone()),
                    src: *src,
                }
            }
            UnlinkedInstructionKind::IterInit { dst, iterable } => InstructionKind::IterInit {
                dst: *dst,
                iterable: *iterable,
            },
            UnlinkedInstructionKind::IterNext {
                iterator,
                dst,
                jump_if_done,
            } => InstructionKind::IterNext {
                iterator: *iterator,
                dst: *dst,
                jump_if_done: *jump_if_done,
            },
            UnlinkedInstructionKind::RangeNext {
                cursor,
                end,
                done,
                inclusive,
                dst,
                jump_if_done,
            } => InstructionKind::RangeNext {
                cursor: *cursor,
                end: *end,
                done: *done,
                inclusive: *inclusive,
                dst: *dst,
                jump_if_done: *jump_if_done,
            },
            UnlinkedInstructionKind::I64RangeNext {
                cursor,
                end,
                done,
                inclusive,
                dst,
                jump_if_done,
            } => InstructionKind::I64RangeNext {
                cursor: *cursor,
                end: *end,
                done: *done,
                inclusive: *inclusive,
                dst: *dst,
                jump_if_done: *jump_if_done,
            },
            UnlinkedInstructionKind::EnumTagEqual {
                dst,
                value,
                enum_name,
                variant,
            } => {
                let enum_ty = self.link_type(enum_name)?;
                let variant = self.link_variant(enum_name, variant, enum_ty)?;
                InstructionKind::EnumTagEqual {
                    dst: *dst,
                    value: *value,
                    enum_ty,
                    variant,
                }
            }
            UnlinkedInstructionKind::LoadGlobal {
                dst,
                global,
                slot,
                cache_site,
            } => {
                let slot = slot
                    .or_else(|| program.global_slot(global))
                    .ok_or_else(|| LinkError::MissingGlobal {
                        function: code.name.clone(),
                        global: global.clone(),
                    })?;
                let debug_name = self.linked.intern_debug_name(global.clone());
                InstructionKind::LoadGlobal {
                    dst: *dst,
                    slot,
                    debug_name,
                    cache_site: *cache_site,
                }
            }
            UnlinkedInstructionKind::HostRead {
                dst,
                root,
                target,
                dynamic_args,
                cache_site,
            } => {
                let target = self.link_host_target(code, host_target_map, *target)?;
                InstructionKind::HostRead {
                    dst: *dst,
                    root: *root,
                    target,
                    dynamic_args: dynamic_args.clone(),
                    cache_site: *cache_site,
                }
            }
            UnlinkedInstructionKind::HostWrite {
                root,
                target,
                dynamic_args,
                src,
                cache_site,
            } => {
                let target = self.link_host_target(code, host_target_map, *target)?;
                InstructionKind::HostWrite {
                    root: *root,
                    target,
                    dynamic_args: dynamic_args.clone(),
                    src: *src,
                    cache_site: *cache_site,
                }
            }
            UnlinkedInstructionKind::HostMutate {
                root,
                target,
                dynamic_args,
                op,
                rhs,
                cache_site,
            } => {
                let target = self.link_host_target(code, host_target_map, *target)?;
                InstructionKind::HostMutate {
                    root: *root,
                    target,
                    dynamic_args: dynamic_args.clone(),
                    op: *op,
                    rhs: *rhs,
                    cache_site: *cache_site,
                }
            }
            UnlinkedInstructionKind::HostRemove {
                root,
                target,
                dynamic_args,
                cache_site,
            } => {
                let target = self.link_host_target(code, host_target_map, *target)?;
                InstructionKind::HostRemove {
                    root: *root,
                    target,
                    dynamic_args: dynamic_args.clone(),
                    cache_site: *cache_site,
                }
            }
            UnlinkedInstructionKind::HostCall {
                dst,
                root,
                target,
                dynamic_args,
                method,
                args,
                cache_site,
            } => {
                let target = self.link_host_target(code, host_target_map, *target)?;
                let dispatch = self.link_host_method(*method);
                let debug_text = format!("host_method::{}", method.get());
                let debug_name = self.linked.intern_debug_name(debug_text);
                InstructionKind::HostCall {
                    dst: *dst,
                    root: *root,
                    target,
                    dynamic_args: dynamic_args.clone(),
                    method: dispatch,
                    debug_name,
                    args: args.clone(),
                    cache_site: *cache_site,
                }
            }
            UnlinkedInstructionKind::GuardType { src, guard } => {
                let guard = self.link_type_guard(guard.clone(), linked_code)?;
                InstructionKind::GuardType { src: *src, guard }
            }
            UnlinkedInstructionKind::Return { src } => InstructionKind::Return { src: *src },
        };

        Ok(Instruction {
            kind,
            span: instruction.span,
        })
    }

    fn resolve_script_function(
        &self,
        target: FunctionId,
        name: &str,
    ) -> Result<ScriptFunctionHandle, LinkError> {
        self.script_functions_by_id
            .get(&target)
            .copied()
            .ok_or_else(|| LinkError::MissingScriptFunction {
                name: name.to_owned(),
                id: target,
            })
    }

    fn link_native(&mut self, name: &str, id: FunctionId) -> Result<NativeHandle, LinkError> {
        if let Some(handle) = self.native_handles.get(&id).copied() {
            return Ok(handle);
        }

        if let Some(registry) = self.linker.registry
            && registry.get(id.def_id()).and_then(Def::function_id) != Some(id)
        {
            return Err(LinkError::UnresolvedNative {
                name: name.to_owned(),
                id,
            });
        }

        if !self.linker.native_implementations.contains(&id) {
            return Err(LinkError::MissingNativeImplementation {
                name: name.to_owned(),
                id,
            });
        }

        let debug_name = self.linked.intern_debug_name(name.to_owned());
        let handle = self
            .linked
            .push_native_function(LinkedNativeFunction::new(id, debug_name));
        self.native_handles.insert(id, handle);
        Ok(handle)
    }

    fn link_method_dispatch(
        &mut self,
        method: &str,
        method_id: MethodId,
    ) -> Result<MethodDispatchHandle, LinkError> {
        let key = if let Some(function) = self.script_methods_by_id.get(&method_id).copied() {
            MethodDispatchKey::Script(method_id, function)
        } else {
            if let Some(registry) = self.linker.registry
                && registry.get(method_id.def_id()).and_then(Def::method_id) != Some(method_id)
            {
                return Err(LinkError::MissingMethodDefinition {
                    method: method.to_owned(),
                    id: method_id,
                });
            }
            MethodDispatchKey::Value(method_id)
        };

        self.intern_method_dispatch(key, method.to_owned())
    }

    fn link_host_method(&mut self, method_id: HostMethodId) -> MethodDispatchHandle {
        self.intern_method_dispatch(
            MethodDispatchKey::Host(method_id),
            format!("host_method::{}", method_id.get()),
        )
        .expect("host method dispatch cannot fail")
    }

    fn intern_method_dispatch(
        &mut self,
        key: MethodDispatchKey,
        debug_text: String,
    ) -> Result<MethodDispatchHandle, LinkError> {
        if let Some(handle) = self.method_handles.get(&key).copied() {
            return Ok(handle);
        }

        let debug_name = self.linked.intern_debug_name(debug_text);
        let kind = match key {
            MethodDispatchKey::Script(method_id, function) => LinkedMethodDispatchKind::Script {
                method_id,
                function,
            },
            MethodDispatchKey::Value(method_id) => LinkedMethodDispatchKind::Value { method_id },
            MethodDispatchKey::Host(method_id) => LinkedMethodDispatchKind::Host { method_id },
        };
        let handle = self
            .linked
            .push_method_dispatch(LinkedMethodDispatch::new(debug_name, kind));
        self.method_handles.insert(key, handle);
        Ok(handle)
    }

    fn link_type(&mut self, name: &str) -> Result<TypeHandle, LinkError> {
        let id = self.resolve_type_id(name)?;
        if let Some(handle) = self.type_handles.get(&id).copied() {
            return Ok(handle);
        }

        let debug_name = self.linked.intern_debug_name(name.to_owned());
        let handle = self.linked.push_type(LinkedType::new(id, debug_name));
        self.type_handles.insert(id, handle);
        Ok(handle)
    }

    fn link_variant(
        &mut self,
        enum_name: &str,
        variant: &str,
        owner: TypeHandle,
    ) -> Result<VariantHandle, LinkError> {
        let id = self.resolve_variant_id(enum_name, variant)?;
        if let Some(handle) = self.variant_handles.get(&id).copied() {
            return Ok(handle);
        }

        let debug_name = self
            .linked
            .intern_debug_name(format!("{enum_name}::{variant}"));
        let handle = self
            .linked
            .push_variant(LinkedVariant::new(id, owner, debug_name));
        self.variant_handles.insert(id, handle);
        Ok(handle)
    }

    fn resolve_type_id(&self, name: &str) -> Result<TypeId, LinkError> {
        if let Some(registry) = self.linker.registry {
            for path in type_path_candidates(name) {
                if let Some(id) = registry.get_by_path(&path).and_then(Def::type_id) {
                    return Ok(id);
                }
            }
            return Ok(TypeId::from_def_id(script_type_path(name).id()));
        }

        Ok(TypeId::from_def_id(script_type_path(name).id()))
    }

    fn resolve_variant_id(&self, enum_name: &str, variant: &str) -> Result<VariantId, LinkError> {
        if let Some(registry) = self.linker.registry {
            for path in variant_path_candidates(enum_name, variant) {
                if let Some(id) = registry.get_by_path(&path).and_then(Def::variant_id) {
                    return Ok(id);
                }
            }
            return Ok(VariantId::from_def_id(
                script_variant_path(enum_name, variant).id(),
            ));
        }

        Ok(VariantId::from_def_id(
            script_variant_path(enum_name, variant).id(),
        ))
    }

    fn link_host_target(
        &self,
        code: &UnlinkedCodeObject,
        host_target_map: &[HostTargetPlanId],
        target: HostTargetPlanId,
    ) -> Result<HostTargetPlanId, LinkError> {
        host_target_map
            .get(target.index())
            .copied()
            .ok_or_else(|| LinkError::InvalidHostTarget {
                function: code.name.clone(),
                target,
            })
    }

    fn link_type_guard(
        &mut self,
        guard: UnlinkedTypeGuard,
        code: &mut LinkedCodeObject,
    ) -> Result<crate::TypeGuardPlanId, LinkError> {
        let plan = self.link_type_guard_plan(guard.plan)?;
        let context = GuardContext::new(
            guard.context.kind,
            guard.context.location,
            self.linked.intern_debug_name(guard.context.debug_name),
        );
        Ok(code.intern_type_guard(TypeGuard::new(plan, context)))
    }

    fn link_type_guard_plan(
        &mut self,
        plan: UnlinkedTypeGuardPlan,
    ) -> Result<TypeGuardPlan, LinkError> {
        match plan {
            UnlinkedTypeGuardPlan::Primitive(tag) => Ok(TypeGuardPlan::Primitive(tag)),
            UnlinkedTypeGuardPlan::Type(name) => self.link_type(&name).map(TypeGuardPlan::Type),
            UnlinkedTypeGuardPlan::Variant { enum_name, variant } => {
                let owner = self.link_type(&enum_name)?;
                self.link_variant(&enum_name, &variant, owner)
                    .map(TypeGuardPlan::Variant)
            }
            UnlinkedTypeGuardPlan::Shape {
                type_name,
                shape_id,
            } => self
                .link_type(&type_name)
                .map(|ty| TypeGuardPlan::Shape { ty, shape_id }),
            UnlinkedTypeGuardPlan::HostType(name) => {
                self.link_type(&name).map(TypeGuardPlan::HostType)
            }
        }
    }
}

fn cache_site_at(
    code: &UnlinkedCodeObject,
    instruction_offset: InstructionOffset,
    kind: CacheSiteKind,
) -> Option<CacheSiteId> {
    code.cache_sites
        .sites()
        .iter()
        .find(|site| site.instruction_offset == instruction_offset && site.kind == kind)
        .map(|site| site.id)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum MethodDispatchKey {
    Script(MethodId, ScriptFunctionHandle),
    Value(MethodId),
    Host(HostMethodId),
}

fn sorted_field_slots<'field>(
    fields: impl IntoIterator<Item = &'field String>,
) -> BTreeMap<String, usize> {
    let mut fields = fields.into_iter().cloned().collect::<Vec<_>>();
    fields.sort_unstable();
    fields.dedup();
    fields
        .into_iter()
        .enumerate()
        .map(|(slot, field)| (field, slot))
        .collect()
}

fn type_path_candidates(name: &str) -> Vec<DefPath> {
    let mut paths = Vec::new();
    if !name.contains("::") {
        paths.push(DefPath::ty("std", std::iter::empty::<&str>(), name));
        paths.push(DefPath::ty("host", std::iter::empty::<&str>(), name));
    }
    paths.push(script_type_path(name));
    paths
}

fn variant_path_candidates(enum_name: &str, variant: &str) -> Vec<DefPath> {
    let mut paths = Vec::new();
    if !enum_name.contains("::") {
        paths.push(DefPath::variant(
            "std",
            std::iter::empty::<&str>(),
            enum_name,
            variant,
        ));
        paths.push(DefPath::variant(
            "host",
            std::iter::empty::<&str>(),
            enum_name,
            variant,
        ));
    }
    paths.push(script_variant_path(enum_name, variant));
    paths
}

fn script_type_path(name: &str) -> DefPath {
    let mut segments = name.split("::").collect::<Vec<_>>();
    let ty = segments.pop().unwrap_or(name);
    DefPath::ty("script", segments, ty)
}

fn script_variant_path(enum_name: &str, variant: &str) -> DefPath {
    let mut segments = enum_name.split("::").collect::<Vec<_>>();
    let owner = segments.pop().unwrap_or(enum_name);
    DefPath::variant("script", segments, owner, variant)
}

#[cfg(test)]
mod tests;
