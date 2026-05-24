//! Register VM for Vela bytecode.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::Arc;

use vela_bytecode::{CodeObject, Constant, InstructionKind, Register};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Map(BTreeMap<String, Value>),
}

impl From<&Constant> for Value {
    fn from(value: &Constant) -> Self {
        match value {
            Constant::Null => Self::Null,
            Constant::Bool(value) => Self::Bool(*value),
            Constant::Int(value) => Self::Int(*value),
            Constant::Float(value) => Self::Float(*value),
            Constant::String(value) => Self::String(value.clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VmError {
    pub kind: VmErrorKind,
}

impl VmError {
    fn new(kind: VmErrorKind) -> Self {
        Self { kind }
    }
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl std::error::Error for VmError {}

#[derive(Clone, Debug, PartialEq)]
pub enum VmErrorKind {
    RegisterOutOfBounds { register: Register },
    ConstantOutOfBounds { constant: usize },
    InstructionOutOfBounds { offset: usize },
    TypeMismatch { operation: &'static str },
    DivisionByZero,
    UnknownNative { name: String },
    MissingReturn,
}

pub type VmResult<T> = Result<T, VmError>;

pub type NativeFunction = Arc<dyn Fn(&[Value]) -> VmResult<Value> + Send + Sync + 'static>;

#[derive(Clone, Default)]
pub struct Vm {
    natives: HashMap<String, NativeFunction>,
}

impl Vm {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_native(
        &mut self,
        name: impl Into<String>,
        function: impl Fn(&[Value]) -> VmResult<Value> + Send + Sync + 'static,
    ) {
        self.natives.insert(name.into(), Arc::new(function));
    }

    pub fn run(&self, code: &CodeObject) -> VmResult<Value> {
        let mut frame = CallFrame::new(code.register_count);
        let mut ip = 0_usize;

        while ip < code.instructions.len() {
            let instruction = &code.instructions[ip];
            ip = ip.saturating_add(1);

            match &instruction.kind {
                InstructionKind::LoadConst { dst, constant } => {
                    let constant_value = code.constants.get(constant.0).ok_or(VmError {
                        kind: VmErrorKind::ConstantOutOfBounds {
                            constant: constant.0,
                        },
                    })?;
                    frame.write(*dst, Value::from(constant_value))?;
                }
                InstructionKind::Move { dst, src } => {
                    let value = frame.read(*src)?.clone();
                    frame.write(*dst, value)?;
                }
                InstructionKind::Add { dst, lhs, rhs } => {
                    let value =
                        binary_numeric(frame.read(*lhs)?, frame.read(*rhs)?, "add", |a, b| a + b)?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Sub { dst, lhs, rhs } => {
                    let value =
                        binary_numeric(frame.read(*lhs)?, frame.read(*rhs)?, "sub", |a, b| a - b)?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Mul { dst, lhs, rhs } => {
                    let value =
                        binary_numeric(frame.read(*lhs)?, frame.read(*rhs)?, "mul", |a, b| a * b)?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Div { dst, lhs, rhs } => {
                    let value = div_numeric(frame.read(*lhs)?, frame.read(*rhs)?)?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Equal { dst, lhs, rhs } => {
                    let value = Value::Bool(frame.read(*lhs)? == frame.read(*rhs)?);
                    frame.write(*dst, value)?;
                }
                InstructionKind::Less { dst, lhs, rhs } => {
                    let value = compare_numeric(frame.read(*lhs)?, frame.read(*rhs)?, "less")?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::JumpIfFalse { condition, target } => {
                    if !is_truthy(frame.read(*condition)?) {
                        validate_jump(code, target.0)?;
                        ip = target.0;
                    }
                }
                InstructionKind::Jump { target } => {
                    validate_jump(code, target.0)?;
                    ip = target.0;
                }
                InstructionKind::CallNative { dst, name, args } => {
                    let native = self.natives.get(name).ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownNative { name: name.clone() })
                    })?;
                    let values = args
                        .iter()
                        .map(|register| frame.read(*register).cloned())
                        .collect::<VmResult<Vec<_>>>()?;
                    let result = native(&values)?;
                    if let Some(dst) = dst {
                        frame.write(*dst, result)?;
                    }
                }
                InstructionKind::Return { src } => return Ok(frame.read(*src)?.clone()),
            }
        }

        Err(VmError::new(VmErrorKind::MissingReturn))
    }
}

#[derive(Clone, Debug)]
struct CallFrame {
    registers: Vec<Value>,
}

impl CallFrame {
    fn new(register_count: u16) -> Self {
        Self {
            registers: vec![Value::Null; usize::from(register_count)],
        }
    }

    fn read(&self, register: Register) -> VmResult<&Value> {
        self.registers
            .get(usize::from(register.0))
            .ok_or_else(|| VmError::new(VmErrorKind::RegisterOutOfBounds { register }))
    }

    fn write(&mut self, register: Register, value: Value) -> VmResult<()> {
        let slot = self
            .registers
            .get_mut(usize::from(register.0))
            .ok_or(VmError {
                kind: VmErrorKind::RegisterOutOfBounds { register },
            })?;
        *slot = value;
        Ok(())
    }
}

fn binary_numeric(
    lhs: &Value,
    rhs: &Value,
    operation: &'static str,
    int_op: impl FnOnce(i64, i64) -> i64,
) -> VmResult<Value> {
    match (lhs, rhs) {
        (Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(int_op(*lhs, *rhs))),
        (Value::Float(lhs), Value::Float(rhs)) => {
            Ok(Value::Float(int_op_float(*lhs, *rhs, operation)?))
        }
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn int_op_float(lhs: f64, rhs: f64, operation: &'static str) -> VmResult<f64> {
    match operation {
        "add" => Ok(lhs + rhs),
        "sub" => Ok(lhs - rhs),
        "mul" => Ok(lhs * rhs),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn div_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    match (lhs, rhs) {
        (Value::Int(_), Value::Int(0)) => Err(VmError::new(VmErrorKind::DivisionByZero)),
        (Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(lhs / rhs)),
        (Value::Float(_), Value::Float(rhs)) if *rhs == 0.0 => {
            Err(VmError::new(VmErrorKind::DivisionByZero))
        }
        (Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Float(lhs / rhs)),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation: "div" })),
    }
}

fn compare_numeric(lhs: &Value, rhs: &Value, operation: &'static str) -> VmResult<bool> {
    match (lhs, rhs) {
        (Value::Int(lhs), Value::Int(rhs)) => Ok(lhs < rhs),
        (Value::Float(lhs), Value::Float(rhs)) => Ok(lhs < rhs),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Null | Value::Bool(false))
}

fn validate_jump(code: &CodeObject, offset: usize) -> VmResult<()> {
    if offset <= code.instructions.len() {
        Ok(())
    } else {
        Err(VmError::new(VmErrorKind::InstructionOutOfBounds { offset }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_bytecode::compiler::compile_function_source;
    use vela_bytecode::{ConstantId, Instruction, InstructionOffset};
    use vela_common::SourceId;

    #[test]
    fn runs_basic_arithmetic() {
        let mut code = CodeObject::new("calc", 5);
        let two = code.push_constant(Constant::Int(2));
        let three = code.push_constant(Constant::Int(3));
        let four = code.push_constant(Constant::Int(4));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: two,
        }));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: three,
        }));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(2),
            constant: four,
        }));
        code.push_instruction(Instruction::new(InstructionKind::Mul {
            dst: Register(3),
            lhs: Register(1),
            rhs: Register(2),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Add {
            dst: Register(4),
            lhs: Register(0),
            rhs: Register(3),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(4),
        }));

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(14)));
    }

    #[test]
    fn branches_on_false_conditions() {
        let mut code = CodeObject::new("branch", 3);
        let false_id = code.push_constant(Constant::Bool(false));
        let one = code.push_constant(Constant::Int(1));
        let two = code.push_constant(Constant::Int(2));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: false_id,
        }));
        code.push_instruction(Instruction::new(InstructionKind::JumpIfFalse {
            condition: Register(0),
            target: InstructionOffset(4),
        }));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: one,
        }));
        code.push_instruction(Instruction::new(InstructionKind::Jump {
            target: InstructionOffset(5),
        }));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: two,
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(1),
        }));

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(2)));
    }

    #[test]
    fn calls_registered_native_functions() {
        let mut vm = Vm::new();
        vm.register_native("log", |args| {
            assert_eq!(args, [Value::String("level up".into())]);
            Ok(Value::Null)
        });

        let mut code = CodeObject::new("native", 2);
        code.push_constant(Constant::String("level up".into()));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: ConstantId(0),
        }));
        code.push_instruction(Instruction::new(InstructionKind::CallNative {
            dst: Some(Register(1)),
            name: "log".into(),
            args: vec![Register(0)],
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(1),
        }));

        assert_eq!(vm.run(&code), Ok(Value::Null));
    }

    #[test]
    fn runs_compiled_arithmetic_source() {
        let code = compile_function_source(
            SourceId::new(1),
            "fn main() { let base = 2; return base + 3 * 4; }",
            "main",
        )
        .expect("compile arithmetic source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(14)));
    }

    #[test]
    fn runs_compiled_native_call_source() {
        let mut vm = Vm::new();
        vm.register_native("log", |args| {
            assert_eq!(args, [Value::String("compiled".into())]);
            Ok(Value::Int(7))
        });

        let code = compile_function_source(
            SourceId::new(1),
            "fn main() { return log(\"compiled\"); }",
            "main",
        )
        .expect("compile native call source");

        assert_eq!(vm.run(&code), Ok(Value::Int(7)));
    }
}
