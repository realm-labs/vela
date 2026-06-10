use std::collections::BTreeMap;

use vela_common::{GlobalSlot, Span};
use vela_host::resolved::HostMutationOp;
use vela_host::target::HostTargetPlan;
use vela_registry::{DebugNameId, DebugNameTable};

use crate::{
    CacheSiteId, CacheSiteLayout, CallArgument, Constant, ConstantId, HostTargetPlanId,
    InstructionOffset, Register,
};

macro_rules! dense_handle {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(u32);

        impl $name {
            #[must_use]
            pub fn new(index: usize) -> Self {
                Self(
                    u32::try_from(index)
                        .expect(concat!(stringify!($name), " index exceeds u32::MAX")),
                )
            }

            #[must_use]
            pub const fn get(self) -> u32 {
                self.0
            }

            #[must_use]
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

dense_handle!(NativeHandle);
dense_handle!(ScriptFunctionHandle);
dense_handle!(MethodDispatchHandle);
dense_handle!(TypeHandle);
dense_handle!(VariantHandle);
dense_handle!(FieldSlot);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinkedProgram {
    debug_names: DebugNameTable,
    functions: Vec<LinkedCodeObject>,
    entry_points: BTreeMap<DebugNameId, ScriptFunctionHandle>,
}

impl LinkedProgram {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern_debug_name(&mut self, name: impl Into<String>) -> DebugNameId {
        self.debug_names.intern(name)
    }

    #[must_use]
    pub fn debug_name(&self, id: DebugNameId) -> &str {
        self.debug_names.debug_name(id)
    }

    #[must_use]
    pub fn debug_names(&self) -> &DebugNameTable {
        &self.debug_names
    }

    pub fn push_function(&mut self, function: LinkedCodeObject) -> ScriptFunctionHandle {
        let handle = ScriptFunctionHandle::new(self.functions.len());
        self.functions.push(function);
        handle
    }

    #[must_use]
    pub fn function(&self, handle: ScriptFunctionHandle) -> Option<&LinkedCodeObject> {
        self.functions.get(handle.index())
    }

    pub fn functions(&self) -> impl Iterator<Item = (ScriptFunctionHandle, &LinkedCodeObject)> {
        self.functions
            .iter()
            .enumerate()
            .map(|(index, function)| (ScriptFunctionHandle::new(index), function))
    }

    #[must_use]
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    pub fn set_entry_point(&mut self, debug_name: DebugNameId, function: ScriptFunctionHandle) {
        self.entry_points.insert(debug_name, function);
    }

    #[must_use]
    pub fn entry_point(&self, debug_name: DebugNameId) -> Option<ScriptFunctionHandle> {
        self.entry_points.get(&debug_name).copied()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinkedCodeObject {
    pub debug_name: DebugNameId,
    pub params: Vec<DebugNameId>,
    pub param_defaults: Vec<bool>,
    pub capture_count: u16,
    pub register_count: u16,
    pub frame: LinkedFrameDebugInfo,
    pub cache_sites: CacheSiteLayout,
    pub constants: Vec<Constant>,
    pub host_targets: Vec<HostTargetPlan>,
    pub instructions: Vec<Instruction>,
}

impl LinkedCodeObject {
    #[must_use]
    pub fn new(debug_name: DebugNameId, register_count: u16) -> Self {
        Self {
            debug_name,
            params: Vec::new(),
            param_defaults: Vec::new(),
            capture_count: 0,
            register_count,
            frame: LinkedFrameDebugInfo::default(),
            cache_sites: CacheSiteLayout::default(),
            constants: Vec::new(),
            host_targets: Vec::new(),
            instructions: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_params(mut self, params: Vec<DebugNameId>) -> Self {
        self.param_defaults = vec![false; params.len()];
        self.params = params;
        self
    }

    #[must_use]
    pub fn with_param_defaults(mut self, defaults: Vec<bool>) -> Self {
        self.param_defaults = defaults;
        self
    }

    #[must_use]
    pub fn with_capture_count(mut self, capture_count: u16) -> Self {
        self.capture_count = capture_count;
        self
    }

    pub fn push_constant(&mut self, constant: Constant) -> ConstantId {
        let id = ConstantId(self.constants.len());
        self.constants.push(constant);
        id
    }

    pub fn intern_host_target(&mut self, target: HostTargetPlan) -> HostTargetPlanId {
        if let Some(index) = self
            .host_targets
            .iter()
            .position(|existing| existing == &target)
        {
            return HostTargetPlanId::new(index);
        }
        let id = HostTargetPlanId::new(self.host_targets.len());
        self.host_targets.push(target);
        id
    }

    #[must_use]
    pub fn host_target(&self, id: HostTargetPlanId) -> Option<&HostTargetPlan> {
        self.host_targets.get(id.index())
    }

    pub fn push_instruction(&mut self, instruction: Instruction) {
        self.instructions.push(instruction);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LinkedFrameDebugInfo {
    pub slots: Vec<LinkedFrameSlotInfo>,
}

impl LinkedFrameDebugInfo {
    pub fn push_slot(&mut self, slot: LinkedFrameSlotInfo) {
        self.slots.push(slot);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkedFrameSlotInfo {
    pub name: DebugNameId,
    pub register: Register,
    pub span: Option<Span>,
}

impl LinkedFrameSlotInfo {
    #[must_use]
    pub const fn new(name: DebugNameId, register: Register, span: Option<Span>) -> Self {
        Self {
            name,
            register,
            span,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instruction {
    pub kind: InstructionKind,
    pub span: Option<Span>,
}

impl Instruction {
    #[must_use]
    pub fn new(kind: InstructionKind) -> Self {
        Self { kind, span: None }
    }

    #[must_use]
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum InstructionKind {
    LoadConst {
        dst: Register,
        constant: ConstantId,
    },
    Move {
        dst: Register,
        src: Register,
    },
    Not {
        dst: Register,
        src: Register,
    },
    Truthy {
        dst: Register,
        src: Register,
    },
    Negate {
        dst: Register,
        src: Register,
    },
    Add {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    Sub {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    Mul {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    Div {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    Rem {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    Equal {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    NotEqual {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    Less {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    LessEqual {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    Greater {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    GreaterEqual {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    JumpIfFalse {
        condition: Register,
        target: InstructionOffset,
    },
    JumpIfNotMissing {
        value: Register,
        target: InstructionOffset,
    },
    Jump {
        target: InstructionOffset,
    },
    CallNative {
        dst: Option<Register>,
        native: NativeHandle,
        debug_name: DebugNameId,
        args: Vec<Register>,
    },
    CallFunction {
        dst: Register,
        function: ScriptFunctionHandle,
        debug_name: DebugNameId,
        args: Vec<CallArgument>,
    },
    MakeClosure {
        dst: Register,
        function: ScriptFunctionHandle,
        captures: Vec<Register>,
    },
    CallClosure {
        dst: Register,
        callee: Register,
        args: Vec<Register>,
    },
    CallMethod {
        dst: Register,
        receiver: Register,
        dispatch: MethodDispatchHandle,
        debug_name: DebugNameId,
        args: Vec<CallArgument>,
    },
    TryPropagate {
        dst: Register,
        src: Register,
    },
    MakeArray {
        dst: Register,
        elements: Vec<Register>,
    },
    MakeMap {
        dst: Register,
        entries: Vec<(ConstantId, Register)>,
    },
    MakeRange {
        dst: Register,
        start: Register,
        end: Register,
        inclusive: bool,
    },
    MakeRecord {
        dst: Register,
        ty: TypeHandle,
        fields: Vec<(FieldSlot, Register)>,
    },
    MakeEnum {
        dst: Register,
        enum_ty: TypeHandle,
        variant: VariantHandle,
        fields: Vec<(FieldSlot, Register)>,
    },
    GetRecordSlot {
        dst: Register,
        record: Register,
        field: FieldSlot,
    },
    SetRecordSlot {
        record: Register,
        field: FieldSlot,
        src: Register,
    },
    GetEnumSlot {
        dst: Register,
        value: Register,
        field: FieldSlot,
    },
    GetIndex {
        dst: Register,
        base: Register,
        index: Register,
    },
    SetIndex {
        base: Register,
        index: Register,
        src: Register,
    },
    IterInit {
        dst: Register,
        iterable: Register,
    },
    IterNext {
        iterator: Register,
        dst: Register,
        jump_if_done: InstructionOffset,
    },
    RangeNext {
        cursor: Register,
        end: Register,
        done: Register,
        inclusive: bool,
        dst: Register,
        jump_if_done: InstructionOffset,
    },
    EnumTagEqual {
        dst: Register,
        value: Register,
        enum_ty: TypeHandle,
        variant: VariantHandle,
    },
    LoadGlobal {
        dst: Register,
        slot: GlobalSlot,
        debug_name: DebugNameId,
        cache_site: Option<CacheSiteId>,
    },
    HostRead {
        dst: Register,
        root: Register,
        target: HostTargetPlanId,
        dynamic_args: Vec<Register>,
        cache_site: CacheSiteId,
    },
    HostWrite {
        root: Register,
        target: HostTargetPlanId,
        dynamic_args: Vec<Register>,
        src: Register,
        cache_site: CacheSiteId,
    },
    HostMutate {
        root: Register,
        target: HostTargetPlanId,
        dynamic_args: Vec<Register>,
        op: HostMutationOp,
        rhs: Register,
        cache_site: CacheSiteId,
    },
    HostRemove {
        root: Register,
        target: HostTargetPlanId,
        dynamic_args: Vec<Register>,
        cache_site: CacheSiteId,
    },
    HostCall {
        dst: Option<Register>,
        root: Register,
        target: HostTargetPlanId,
        dynamic_args: Vec<Register>,
        method: MethodDispatchHandle,
        debug_name: DebugNameId,
        args: Vec<Register>,
        cache_site: CacheSiteId,
    },
    Return {
        src: Register,
    },
}

#[cfg(test)]
mod tests {
    use vela_common::{GlobalSlot, HostTypeId};
    use vela_def::FieldId;
    use vela_host::target::HostTargetPlan;

    use super::*;
    use crate::{CacheSiteId, Constant, Register};

    #[test]
    fn linked_program_stores_debug_names_in_side_table() {
        let mut program = LinkedProgram::new();
        let main_name = program.intern_debug_name("main");
        let param_name = program.intern_debug_name("amount");
        let code = LinkedCodeObject::new(main_name, 2).with_params(vec![param_name]);

        let main = program.push_function(code);
        program.set_entry_point(main_name, main);

        assert_eq!(program.debug_name(main_name), "main");
        assert_eq!(program.debug_name(param_name), "amount");
        assert_eq!(program.entry_point(main_name), Some(main));
        assert_eq!(
            program.function(main).expect("main function").params,
            [param_name]
        );
    }

    #[test]
    fn linked_call_instructions_use_runtime_handles_and_debug_name_ids() {
        let mut program = LinkedProgram::new();
        let native_name = program.intern_debug_name("math::abs");
        let script_name = program.intern_debug_name("main::helper");
        let method_name = program.intern_debug_name("Map::get_or");

        let native = NativeHandle::new(2);
        let script = ScriptFunctionHandle::new(3);
        let method = MethodDispatchHandle::new(4);

        let native_call = InstructionKind::CallNative {
            dst: Some(Register(0)),
            native,
            debug_name: native_name,
            args: vec![Register(1)],
        };
        let script_call = InstructionKind::CallFunction {
            dst: Register(2),
            function: script,
            debug_name: script_name,
            args: vec![CallArgument::Register(Register(1))],
        };
        let method_call = InstructionKind::CallMethod {
            dst: Register(3),
            receiver: Register(2),
            dispatch: method,
            debug_name: method_name,
            args: vec![CallArgument::Missing],
        };

        assert!(matches!(
            native_call,
            InstructionKind::CallNative {
                native: id,
                debug_name,
                ..
            } if id == native && debug_name == native_name
        ));
        assert!(matches!(
            script_call,
            InstructionKind::CallFunction {
                function: id,
                debug_name,
                ..
            } if id == script && debug_name == script_name
        ));
        assert!(matches!(
            method_call,
            InstructionKind::CallMethod {
                dispatch: id,
                debug_name,
                ..
            } if id == method && debug_name == method_name
        ));
    }

    #[test]
    fn linked_field_and_global_instructions_use_slots() {
        let mut program = LinkedProgram::new();
        let global_name = program.intern_debug_name("main::score");
        let score_slot = FieldSlot::new(7);
        let player_type = TypeHandle::new(1);
        let variant = VariantHandle::new(2);

        let record = InstructionKind::MakeRecord {
            dst: Register(0),
            ty: player_type,
            fields: vec![(score_slot, Register(1))],
        };
        let tag_check = InstructionKind::EnumTagEqual {
            dst: Register(2),
            value: Register(0),
            enum_ty: player_type,
            variant,
        };
        let global = InstructionKind::LoadGlobal {
            dst: Register(3),
            slot: GlobalSlot::new(5),
            debug_name: global_name,
            cache_site: None,
        };

        assert!(matches!(
            record,
            InstructionKind::MakeRecord { ty, fields, .. }
                if ty == player_type && fields == vec![(score_slot, Register(1))]
        ));
        assert!(matches!(
            tag_check,
            InstructionKind::EnumTagEqual {
                enum_ty,
                variant: id,
                ..
            } if enum_ty == player_type && id == variant
        ));
        assert!(matches!(
            global,
            InstructionKind::LoadGlobal {
                slot,
                debug_name,
                ..
            } if slot == GlobalSlot::new(5) && debug_name == global_name
        ));
    }

    #[test]
    fn linked_code_interns_host_targets_by_plan_handle() {
        let mut program = LinkedProgram::new();
        let name = program.intern_debug_name("host");
        let mut code = LinkedCodeObject::new(name, 1);
        let target = HostTargetPlan::new(HostTypeId::new(1)).field(FieldId::new(2));

        let first = code.intern_host_target(target.clone());
        let second = code.intern_host_target(target);
        code.push_instruction(Instruction::new(InstructionKind::HostRead {
            dst: Register(0),
            root: Register(0),
            target: first,
            dynamic_args: Vec::new(),
            cache_site: CacheSiteId::new(0),
        }));

        assert_eq!(first, second);
        assert_eq!(code.host_targets.len(), 1);
        assert!(matches!(
            code.instructions[0].kind,
            InstructionKind::HostRead { target, .. } if target == first
        ));
    }

    #[test]
    fn linked_map_literals_reference_key_constants_not_inline_strings() {
        let mut program = LinkedProgram::new();
        let name = program.intern_debug_name("map");
        let mut code = LinkedCodeObject::new(name, 2);
        let key = code.push_constant(Constant::String("score".to_owned()));

        code.push_instruction(Instruction::new(InstructionKind::MakeMap {
            dst: Register(1),
            entries: vec![(key, Register(0))],
        }));

        assert!(matches!(
            code.instructions[0].kind,
            InstructionKind::MakeMap { ref entries, .. } if entries == &vec![(key, Register(0))]
        ));
    }
}
