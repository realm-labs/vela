//! Register bytecode for Vela code objects.

pub mod compiler;

use std::collections::BTreeMap;

use vela_common::{FieldId, HostMethodId, Span};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Program {
    pub functions: BTreeMap<String, CodeObject>,
}

impl Program {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_function(&mut self, function: CodeObject) {
        self.functions.insert(function.name.clone(), function);
    }

    #[must_use]
    pub fn function(&self, name: &str) -> Option<&CodeObject> {
        self.functions.get(name)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CodeObject {
    pub name: String,
    pub params: Vec<String>,
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
            register_count,
            constants: Vec::new(),
            instructions: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_params(mut self, params: Vec<String>) -> Self {
        self.params = params;
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
        args: Vec<Register>,
    },
    MakeArray {
        dst: Register,
        elements: Vec<Register>,
    },
    MakeMap {
        dst: Register,
        entries: Vec<(String, Register)>,
    },
    MakeRecord {
        dst: Register,
        type_name: String,
        fields: Vec<(String, Register)>,
    },
    GetRecordField {
        dst: Register,
        record: Register,
        field: String,
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
        method: HostMethodId,
        args: Vec<Register>,
    },
    Return {
        src: Register,
    },
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
