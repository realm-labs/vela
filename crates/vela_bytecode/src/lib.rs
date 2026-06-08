//! Register bytecode for Vela code objects.

pub mod cache_site;
pub mod compiler;
pub mod script_methods;
pub mod verification;

use std::collections::BTreeMap;

use vela_common::{FieldId, FunctionId, GlobalSlot, HostMethodId, MethodId, Span};
use vela_hir::ids::HirLocalId;
use vela_hir::module_graph::ModuleGraph;

pub use cache_site::{CacheSiteDesc, CacheSiteId, CacheSiteKind, CacheSiteLayout};

use crate::script_methods::ScriptMethodTable;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Program {
    pub functions: BTreeMap<String, CodeObject>,
    global_names: Vec<String>,
    global_slots: BTreeMap<String, GlobalSlot>,
    script_methods: ScriptMethodTable,
    script_metadata: Option<ModuleGraph>,
}

impl Program {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_function(&mut self, function: CodeObject) {
        self.functions.insert(function.name.clone(), function);
    }

    pub fn set_global_layout(&mut self, names: impl IntoIterator<Item = String>) {
        self.global_names.clear();
        self.global_slots.clear();
        for name in names {
            if self.global_slots.contains_key(&name) {
                continue;
            }
            let slot = GlobalSlot::new(self.global_names.len());
            self.global_slots.insert(name.clone(), slot);
            self.global_names.push(name);
        }
    }

    #[must_use]
    pub fn global_slot(&self, name: &str) -> Option<GlobalSlot> {
        self.global_slots.get(name).copied()
    }

    #[must_use]
    pub fn global_name(&self, slot: GlobalSlot) -> Option<&str> {
        self.global_names.get(slot.get()).map(String::as_str)
    }

    #[must_use]
    pub fn global_names(&self) -> &[String] {
        &self.global_names
    }

    pub fn verify(&self) -> Result<(), verification::VerificationError> {
        verification::verify_program(self)
    }

    pub fn insert_script_method(
        &mut self,
        type_name: impl Into<String>,
        method: impl Into<String>,
        method_id: MethodId,
        function: impl Into<String>,
    ) {
        self.script_methods
            .insert(type_name, method, method_id, function);
    }

    #[must_use]
    pub fn with_script_metadata(mut self, graph: ModuleGraph) -> Self {
        self.script_metadata = Some(graph);
        self
    }

    pub fn set_script_metadata(&mut self, graph: ModuleGraph) {
        self.script_metadata = Some(graph);
    }

    pub fn set_script_methods(&mut self, methods: ScriptMethodTable) {
        self.script_methods = methods;
    }

    #[must_use]
    pub fn script_metadata(&self) -> Option<&ModuleGraph> {
        self.script_metadata.as_ref()
    }

    #[must_use]
    pub fn script_methods(&self) -> &ScriptMethodTable {
        &self.script_methods
    }

    #[must_use]
    pub fn function(&self, name: &str) -> Option<&CodeObject> {
        self.functions.get(name)
    }

    #[must_use]
    pub fn script_method(&self, type_name: &str, method: &str) -> Option<&CodeObject> {
        let method = self.script_methods.get(type_name, method)?;
        self.function(&method.function)
    }

    #[must_use]
    pub fn script_method_id(&self, type_name: &str, method: &str) -> Option<MethodId> {
        self.script_methods
            .get(type_name, method)
            .map(|method| method.id)
    }

    #[must_use]
    pub fn script_method_by_id(&self, type_name: &str, method_id: MethodId) -> Option<&CodeObject> {
        let method = self.script_methods.get_by_id(type_name, method_id)?;
        self.function(&method.function)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CodeObject {
    pub name: String,
    pub params: Vec<String>,
    pub param_defaults: Vec<bool>,
    pub capture_count: u16,
    pub register_count: u16,
    pub frame: FrameDebugInfo,
    pub cache_sites: CacheSiteLayout,
    pub constants: Vec<Constant>,
    pub nested_functions: Vec<CodeObject>,
    pub instructions: Vec<Instruction>,
}

impl CodeObject {
    #[must_use]
    pub fn new(name: impl Into<String>, register_count: u16) -> Self {
        Self {
            name: name.into(),
            params: Vec::new(),
            param_defaults: Vec::new(),
            capture_count: 0,
            register_count,
            frame: FrameDebugInfo::default(),
            cache_sites: CacheSiteLayout::default(),
            constants: Vec::new(),
            nested_functions: Vec::new(),
            instructions: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_params(mut self, params: Vec<String>) -> Self {
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

    pub fn push_instruction(&mut self, instruction: Instruction) {
        self.instructions.push(instruction);
    }

    pub fn push_nested_function(&mut self, function: CodeObject) -> FunctionIndex {
        let index = FunctionIndex(self.nested_functions.len());
        self.nested_functions.push(function);
        index
    }

    #[must_use]
    pub fn nested_function(&self, index: FunctionIndex) -> Option<&CodeObject> {
        self.nested_functions.get(index.0)
    }

    pub fn push_cache_site(
        &mut self,
        kind: CacheSiteKind,
        instruction_offset: InstructionOffset,
    ) -> CacheSiteId {
        self.cache_sites
            .push(kind, self.name.clone(), instruction_offset)
    }

    pub fn verify(&self) -> Result<(), verification::VerificationError> {
        verification::verify_code_object(self)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FrameDebugInfo {
    pub slots: Vec<FrameSlotInfo>,
}

impl FrameDebugInfo {
    pub fn push_slot(&mut self, slot: FrameSlotInfo) {
        if self
            .slots
            .iter()
            .any(|existing| existing.same_binding(&slot))
        {
            return;
        }
        self.slots.push(slot);
    }

    #[must_use]
    pub fn slot(&self, name: &str, kind: FrameSlotKind) -> Option<&FrameSlotInfo> {
        self.slots
            .iter()
            .find(|slot| slot.name == name && slot.kind == kind)
    }

    pub fn slots_for_register(&self, register: Register) -> impl Iterator<Item = &FrameSlotInfo> {
        self.slots
            .iter()
            .filter(move |slot| slot.register == register)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameSlotInfo {
    pub name: String,
    pub register: Register,
    pub kind: FrameSlotKind,
    pub local: Option<HirLocalId>,
    pub span: Option<Span>,
}

impl FrameSlotInfo {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        register: Register,
        kind: FrameSlotKind,
        local: Option<HirLocalId>,
        span: Option<Span>,
    ) -> Self {
        Self {
            name: name.into(),
            register,
            kind,
            local,
            span,
        }
    }

    fn same_binding(&self, other: &Self) -> bool {
        if self.local.is_some() || other.local.is_some() {
            return self.local == other.local && self.local.is_some();
        }
        self.name == other.name
            && self.register == other.register
            && self.kind == other.kind
            && self.span == other.span
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameSlotKind {
    Capture,
    Parameter,
    Local,
    ForBinding,
    LambdaParameter,
    PatternBinding,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Constant {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Constant>),
    Map(Vec<(String, Constant)>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Register(pub u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConstantId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InstructionOffset(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionIndex(pub usize);

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
        name: String,
        native: Option<FunctionId>,
        args: Vec<Register>,
    },
    CallFunction {
        dst: Register,
        name: String,
        args: Vec<CallArgument>,
    },
    MakeClosure {
        dst: Register,
        function: FunctionIndex,
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
        method: String,
        value_method_id: Option<HostMethodId>,
        args: Vec<CallArgument>,
    },
    CallMethodId {
        dst: Register,
        receiver: Register,
        method: String,
        method_id: MethodId,
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
        entries: Vec<(String, Register)>,
    },
    MakeRange {
        dst: Register,
        start: Register,
        end: Register,
        inclusive: bool,
    },
    MakeRecord {
        dst: Register,
        type_name: String,
        fields: Vec<(String, Register)>,
    },
    MakeEnum {
        dst: Register,
        enum_name: String,
        variant: String,
        fields: Vec<(String, Register)>,
    },
    GetRecordField {
        dst: Register,
        record: Register,
        field: String,
    },
    GetRecordSlot {
        dst: Register,
        record: Register,
        field: String,
        slot: usize,
    },
    SetRecordField {
        record: Register,
        field: String,
        src: Register,
    },
    SetRecordSlot {
        record: Register,
        field: String,
        slot: usize,
        src: Register,
    },
    GetEnumField {
        dst: Register,
        value: Register,
        field: String,
    },
    GetEnumSlot {
        dst: Register,
        value: Register,
        field: String,
        slot: usize,
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
        enum_name: String,
        variant: String,
    },
    LoadGlobal {
        dst: Register,
        global: String,
        slot: Option<GlobalSlot>,
        cache_site: Option<CacheSiteId>,
    },
    GetHostField {
        dst: Register,
        root: Register,
        field: FieldId,
    },
    GetHostPath {
        dst: Register,
        root: Register,
        segments: Vec<HostPathSegment>,
    },
    SetHostField {
        root: Register,
        field: FieldId,
        src: Register,
    },
    SetHostPath {
        root: Register,
        segments: Vec<HostPathSegment>,
        src: Register,
    },
    AddHostField {
        root: Register,
        field: FieldId,
        rhs: Register,
    },
    SubHostField {
        root: Register,
        field: FieldId,
        rhs: Register,
    },
    MulHostField {
        root: Register,
        field: FieldId,
        rhs: Register,
    },
    DivHostField {
        root: Register,
        field: FieldId,
        rhs: Register,
    },
    RemHostField {
        root: Register,
        field: FieldId,
        rhs: Register,
    },
    AddHostPath {
        root: Register,
        segments: Vec<HostPathSegment>,
        rhs: Register,
    },
    SubHostPath {
        root: Register,
        segments: Vec<HostPathSegment>,
        rhs: Register,
    },
    MulHostPath {
        root: Register,
        segments: Vec<HostPathSegment>,
        rhs: Register,
    },
    DivHostPath {
        root: Register,
        segments: Vec<HostPathSegment>,
        rhs: Register,
    },
    RemHostPath {
        root: Register,
        segments: Vec<HostPathSegment>,
        rhs: Register,
    },
    PushHostPath {
        root: Register,
        segments: Vec<HostPathSegment>,
        value: Register,
    },
    RemoveHostPath {
        root: Register,
        segments: Vec<HostPathSegment>,
    },
    CallHostMethod {
        dst: Option<Register>,
        root: Register,
        segments: Vec<HostPathSegment>,
        method: HostMethodId,
        args: Vec<Register>,
    },
    Return {
        src: Register,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostPathSegment {
    Field(FieldId),
    VariantField(FieldId),
    Value(Register),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallArgument {
    Register(Register),
    Missing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_object_records_constants_and_instructions() {
        let mut code = CodeObject::new("answer", 2);
        let constant = code.push_constant(Constant::Int(42));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant,
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(0),
        }));

        assert_eq!(code.name, "answer");
        assert!(code.params.is_empty());
        assert_eq!(code.register_count, 2);
        assert!(code.cache_sites.is_empty());
        assert_eq!(code.constants, [Constant::Int(42)]);
        assert_eq!(code.instructions.len(), 2);
    }

    #[test]
    fn program_indexes_functions_by_name() {
        let mut program = Program::new();
        program.insert_function(CodeObject::new("main", 0));

        assert!(program.function("main").is_some());
        assert!(program.function("missing").is_none());
    }
}
