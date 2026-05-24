//! Register bytecode for Vela code objects.

pub mod compiler;
pub mod script_methods;

use std::collections::BTreeMap;

use vela_common::{FieldId, HostMethodId, MethodId, Span};

use crate::script_methods::ScriptMethodTable;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Program {
    pub functions: BTreeMap<String, CodeObject>,
    script_methods: ScriptMethodTable,
}

impl Program {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_function(&mut self, function: CodeObject) {
        self.functions.insert(function.name.clone(), function);
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
    pub constants: Vec<Constant>,
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
            constants: Vec::new(),
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
}

#[derive(Clone, Debug, PartialEq)]
pub enum Constant {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Register(pub u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConstantId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InstructionOffset(pub usize);

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
        args: Vec<Register>,
    },
    CallFunction {
        dst: Register,
        name: String,
        args: Vec<CallArgument>,
    },
    MakeClosure {
        dst: Register,
        code: Box<CodeObject>,
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
        args: Vec<Register>,
    },
    CallMethodId {
        dst: Register,
        receiver: Register,
        method: String,
        method_id: MethodId,
        args: Vec<Register>,
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
    EnumTagEqual {
        dst: Register,
        value: Register,
        enum_name: String,
        variant: String,
    },
    GetHostField {
        dst: Register,
        root: Register,
        field: FieldId,
    },
    SetHostField {
        root: Register,
        field: FieldId,
        src: Register,
    },
    AddHostField {
        root: Register,
        field: FieldId,
        rhs: Register,
    },
    CallHostMethod {
        dst: Option<Register>,
        root: Register,
        fields: Vec<FieldId>,
        method: HostMethodId,
        args: Vec<Register>,
    },
    Return {
        src: Register,
    },
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
