//! Register bytecode for Vela code objects.

pub mod cache_site;
pub mod compiler;
pub mod linked;
pub mod linker;
pub mod program_image;
pub mod script_methods;
pub mod verification;

use std::collections::BTreeMap;

use vela_common::{GlobalSlot, HostMethodId, PrimitiveTag, ShapeId, Span};
use vela_def::{DefPath, FunctionId, MethodId};
use vela_hir::ids::HirLocalId;
use vela_hir::module_graph::ModuleGraph;
use vela_host::resolved::HostMutationOp;
use vela_host::target::HostTargetPlan;

pub use cache_site::{CacheSiteDesc, CacheSiteId, CacheSiteKind, CacheSiteLayout};
pub use linked::{
    FieldSlot, GuardContext, GuardKind, GuardLocation, Instruction, InstructionKind,
    LinkedCodeObject, LinkedFrameDebugInfo, LinkedFrameSlotInfo, LinkedMethodDispatch,
    LinkedMethodDispatchKind, LinkedNativeFunction, LinkedProgram, LinkedType, LinkedVariant,
    MethodDispatchHandle, NativeHandle, ParameterTypeGuard, ScriptFunctionHandle, TypeGuard,
    TypeGuardPlan, TypeGuardPlanId, TypeHandle, VariantHandle,
};
pub use linker::{LinkError, Linker};
pub use program_image::ProgramImage;
pub use vela_registry::DebugNameId;

use crate::script_methods::ScriptMethodTable;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnlinkedProgram {
    functions: Vec<UnlinkedCodeObject>,
    function_by_name: BTreeMap<String, FunctionIndex>,
    function_by_id: BTreeMap<FunctionId, FunctionIndex>,
    global_names: Vec<String>,
    global_slots: BTreeMap<String, GlobalSlot>,
    script_methods: ScriptMethodTable,
    script_metadata: Option<ModuleGraph>,
}

impl UnlinkedProgram {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_function(&mut self, function: UnlinkedCodeObject) {
        if let Some(index) = self.function_by_name.get(&function.name).copied() {
            self.functions[index.0] = function;
            return;
        }

        let insertion = self
            .functions
            .binary_search_by(|existing| existing.name.as_str().cmp(function.name.as_str()))
            .unwrap_or_else(|index| index);
        self.functions.insert(insertion, function);
        self.rebuild_function_index();
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
    pub fn function(&self, name: &str) -> Option<&UnlinkedCodeObject> {
        self.function_by_name
            .get(name)
            .and_then(|index| self.function_by_index(*index))
    }

    #[must_use]
    pub fn function_by_index(&self, index: FunctionIndex) -> Option<&UnlinkedCodeObject> {
        self.functions.get(index.0)
    }

    #[must_use]
    pub fn function_by_id(&self, id: FunctionId) -> Option<&UnlinkedCodeObject> {
        self.function_by_id
            .get(&id)
            .and_then(|index| self.function_by_index(*index))
    }

    #[must_use]
    pub fn function_mut(&mut self, name: &str) -> Option<&mut UnlinkedCodeObject> {
        let index = self.function_by_name.get(name).copied()?;
        self.functions.get_mut(index.0)
    }

    pub fn functions(&self) -> impl Iterator<Item = &UnlinkedCodeObject> {
        self.functions.iter()
    }

    pub fn function_names(&self) -> impl Iterator<Item = &str> {
        self.functions.iter().map(|function| function.name.as_str())
    }

    #[must_use]
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    pub fn into_functions(self) -> impl Iterator<Item = (String, UnlinkedCodeObject)> {
        self.functions.into_iter().map(|function| {
            let name = function.name.clone();
            (name, function)
        })
    }

    #[must_use]
    pub fn script_method(&self, type_name: &str, method: &str) -> Option<&UnlinkedCodeObject> {
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
    pub fn script_method_by_id(
        &self,
        type_name: &str,
        method_id: MethodId,
    ) -> Option<&UnlinkedCodeObject> {
        let method = self.script_methods.get_by_id(type_name, method_id)?;
        self.function(&method.function)
    }

    fn rebuild_function_index(&mut self) {
        self.function_by_name.clear();
        self.function_by_id.clear();
        self.function_by_name.extend(
            self.functions
                .iter()
                .enumerate()
                .map(|(index, function)| (function.name.clone(), FunctionIndex(index))),
        );
        self.function_by_id
            .extend(self.functions.iter().enumerate().map(|(index, function)| {
                (
                    function_id_for_script_name(&function.name),
                    FunctionIndex(index),
                )
            }));
    }
}

pub trait UnlinkedProgramCode {
    fn function(&self, name: &str) -> Option<&UnlinkedCodeObject>;

    fn function_by_index(&self, _index: FunctionIndex) -> Option<&UnlinkedCodeObject> {
        None
    }

    fn function_by_id(&self, _id: FunctionId) -> Option<&UnlinkedCodeObject> {
        None
    }

    fn script_method(&self, type_name: &str, method: &str) -> Option<&UnlinkedCodeObject>;

    fn script_method_id(&self, type_name: &str, method: &str) -> Option<MethodId>;

    fn script_method_by_id(
        &self,
        type_name: &str,
        method_id: MethodId,
    ) -> Option<&UnlinkedCodeObject>;
}

impl UnlinkedProgramCode for UnlinkedProgram {
    fn function(&self, name: &str) -> Option<&UnlinkedCodeObject> {
        UnlinkedProgram::function(self, name)
    }

    fn function_by_id(&self, id: FunctionId) -> Option<&UnlinkedCodeObject> {
        UnlinkedProgram::function_by_id(self, id)
    }

    fn script_method(&self, type_name: &str, method: &str) -> Option<&UnlinkedCodeObject> {
        UnlinkedProgram::script_method(self, type_name, method)
    }

    fn script_method_id(&self, type_name: &str, method: &str) -> Option<MethodId> {
        UnlinkedProgram::script_method_id(self, type_name, method)
    }

    fn script_method_by_id(
        &self,
        type_name: &str,
        method_id: MethodId,
    ) -> Option<&UnlinkedCodeObject> {
        UnlinkedProgram::script_method_by_id(self, type_name, method_id)
    }
}

pub(crate) fn function_id_for_script_name(name: &str) -> FunctionId {
    let mut segments = name.split("::").collect::<Vec<_>>();
    let function = segments.pop().unwrap_or(name);
    FunctionId::from_def_id(DefPath::function("script", segments, function).id())
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnlinkedCodeObject {
    pub name: String,
    pub params: Vec<String>,
    pub param_defaults: Vec<bool>,
    pub capture_count: u16,
    pub register_count: u16,
    pub frame: FrameDebugInfo,
    pub cache_sites: CacheSiteLayout,
    pub constants: Vec<Constant>,
    pub host_targets: Vec<HostTargetPlan>,
    pub param_guards: Vec<UnlinkedParameterTypeGuard>,
    pub return_guard: Option<UnlinkedTypeGuard>,
    pub nested_functions: Vec<UnlinkedCodeObject>,
    pub instructions: Vec<UnlinkedInstruction>,
}

impl UnlinkedCodeObject {
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
            host_targets: Vec::new(),
            param_guards: Vec::new(),
            return_guard: None,
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

    pub fn push_param_guard(&mut self, parameter: u16, guard: UnlinkedTypeGuard) {
        self.param_guards
            .push(UnlinkedParameterTypeGuard { parameter, guard });
    }

    pub fn set_return_guard(&mut self, guard: UnlinkedTypeGuard) {
        self.return_guard = Some(guard);
    }

    pub fn push_instruction(&mut self, instruction: UnlinkedInstruction) {
        self.instructions.push(instruction);
    }

    pub fn push_nested_function(&mut self, function: UnlinkedCodeObject) -> FunctionIndex {
        let index = FunctionIndex(self.nested_functions.len());
        self.nested_functions.push(function);
        index
    }

    #[must_use]
    pub fn nested_function(&self, index: FunctionIndex) -> Option<&UnlinkedCodeObject> {
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
    Scalar(vela_common::ScalarValue),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<Constant>),
    Map(Vec<(String, Constant)>),
}

impl Constant {
    #[must_use]
    pub const fn i64(value: i64) -> Self {
        Self::Scalar(vela_common::ScalarValue::I64(value))
    }

    #[must_use]
    pub const fn f64(value: f64) -> Self {
        Self::Scalar(vela_common::ScalarValue::F64(value))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Register(pub u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConstantId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InstructionOffset(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionIndex(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct HostTargetPlanId(u32);

impl HostTargetPlanId {
    #[must_use]
    pub fn new(value: usize) -> Self {
        Self(u32::try_from(value).expect("host target plan count exceeds u32::MAX"))
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnlinkedParameterTypeGuard {
    pub parameter: u16,
    pub guard: UnlinkedTypeGuard,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnlinkedTypeGuard {
    pub plan: UnlinkedTypeGuardPlan,
    pub context: UnlinkedGuardContext,
}

impl UnlinkedTypeGuard {
    #[must_use]
    pub fn new(plan: UnlinkedTypeGuardPlan, context: UnlinkedGuardContext) -> Self {
        Self { plan, context }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnlinkedTypeGuardPlan {
    Primitive(PrimitiveTag),
    Type(String),
    Variant {
        enum_name: String,
        variant: String,
    },
    Shape {
        type_name: String,
        shape_id: ShapeId,
    },
    HostType(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnlinkedGuardContext {
    pub kind: GuardKind,
    pub location: GuardLocation,
    pub debug_name: String,
}

impl UnlinkedGuardContext {
    #[must_use]
    pub fn new(kind: GuardKind, location: GuardLocation, debug_name: impl Into<String>) -> Self {
        Self {
            kind,
            location,
            debug_name: debug_name.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryLiteralOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryLiteralSide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum I64CompareOp {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnlinkedInstruction {
    pub kind: UnlinkedInstructionKind,
    pub span: Option<Span>,
}

impl UnlinkedInstruction {
    #[must_use]
    pub fn new(kind: UnlinkedInstructionKind) -> Self {
        Self { kind, span: None }
    }

    #[must_use]
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum UnlinkedInstructionKind {
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
    I64Add {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    I64Sub {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    I64Mul {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    I64Rem {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    I64AddImm {
        dst: Register,
        lhs: Register,
        imm: i64,
    },
    I64SubImm {
        dst: Register,
        lhs: Register,
        imm: i64,
    },
    I64MulImm {
        dst: Register,
        lhs: Register,
        imm: i64,
    },
    I64RemImm {
        dst: Register,
        lhs: Register,
        imm: i64,
    },
    I64CmpImm {
        dst: Register,
        op: I64CompareOp,
        lhs: Register,
        imm: i64,
    },
    I64CmpImmJumpIfFalse {
        op: I64CompareOp,
        lhs: Register,
        imm: i64,
        target: InstructionOffset,
    },
    BinaryIntLiteral {
        dst: Register,
        op: BinaryLiteralOp,
        value: Register,
        literal: String,
        side: BinaryLiteralSide,
    },
    BinaryFloatLiteral {
        dst: Register,
        op: BinaryLiteralOp,
        value: Register,
        literal: String,
        side: BinaryLiteralSide,
    },
    GuardType {
        src: Register,
        guard: UnlinkedTypeGuard,
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
        native: FunctionId,
        cache_site: Option<CacheSiteId>,
        args: Vec<Register>,
    },
    CallFunction {
        dst: Register,
        target: FunctionId,
        name: String,
        mode: ScriptCallMode,
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
    CallDynamicMethod {
        dst: Register,
        receiver: Register,
        method: String,
        args: Vec<DynamicCallArgument>,
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
    I64RangeNext {
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
        method: HostMethodId,
        args: Vec<Register>,
        cache_site: CacheSiteId,
    },
    Return {
        src: Register,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptCallMode {
    Checked,
    Unchecked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallArgument {
    Register(Register),
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicCallArgument {
    pub name: Option<String>,
    pub value: Register,
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::HostTypeId;
    use vela_def::FieldId;
    use vela_host::target::HostTargetPlan;

    #[test]
    fn code_object_records_constants_and_instructions() {
        let mut code = UnlinkedCodeObject::new("answer", 2);
        let constant = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(42)));
        code.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::LoadConst {
                dst: Register(0),
                constant,
            },
        ));
        code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
            src: Register(0),
        }));

        assert_eq!(code.name, "answer");
        assert!(code.params.is_empty());
        assert_eq!(code.register_count, 2);
        assert!(code.cache_sites.is_empty());
        assert_eq!(
            code.constants,
            [Constant::Scalar(vela_common::ScalarValue::I64(42))]
        );
        assert_eq!(code.instructions.len(), 2);
    }

    #[test]
    fn code_object_interns_host_target_plans() {
        let mut code = UnlinkedCodeObject::new("host", 1);
        let target = HostTargetPlan::new(HostTypeId::new(1)).field(FieldId::new(2));

        let first = code.intern_host_target(target.clone());
        let second = code.intern_host_target(target.clone());
        let third = code.intern_host_target(target.const_index(0));

        assert_eq!(first, second);
        assert_ne!(first, third);
        assert_eq!(code.host_targets.len(), 2);
        assert_eq!(code.host_target(first), code.host_targets.first());
    }

    #[test]
    fn program_indexes_functions_by_name() {
        let mut program = UnlinkedProgram::new();
        program.insert_function(UnlinkedCodeObject::new("zeta", 0));
        program.insert_function(UnlinkedCodeObject::new("main", 0));

        assert!(program.function("main").is_some());
        assert!(program.function("missing").is_none());
        assert_eq!(program.function_count(), 2);
        assert_eq!(
            program
                .function_by_index(FunctionIndex(0))
                .map(|function| function.name.as_str()),
            Some("main")
        );
        assert_eq!(
            program
                .function_by_index(FunctionIndex(1))
                .map(|function| function.name.as_str()),
            Some("zeta")
        );
    }
}
