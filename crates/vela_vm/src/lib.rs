//! Register VM for Vela bytecode.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::Arc;

use vela_bytecode::{CodeObject, Constant, InstructionKind, Program, Register};
use vela_host::{
    HostError, HostErrorKind, HostPath, HostRef, HostValue, PatchTx, ScriptStateAdapter,
};
use vela_reflect::{self as reflect, ReflectError, ReflectErrorKind, TypeRegistry};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Map(BTreeMap<String, Value>),
    HostRef(HostRef),
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
    RegisterOutOfBounds {
        register: Register,
    },
    ConstantOutOfBounds {
        constant: usize,
    },
    InstructionOutOfBounds {
        offset: usize,
    },
    TypeMismatch {
        operation: &'static str,
    },
    DivisionByZero,
    UnknownNative {
        name: String,
    },
    UnknownFunction {
        name: String,
    },
    ArityMismatch {
        name: String,
        expected: usize,
        actual: usize,
    },
    Host(HostErrorKind),
    Reflect(ReflectErrorKind),
    MissingReturn,
}

pub type VmResult<T> = Result<T, VmError>;

impl From<HostError> for VmError {
    fn from(value: HostError) -> Self {
        Self::new(VmErrorKind::Host(value.kind))
    }
}

impl From<ReflectError> for VmError {
    fn from(value: ReflectError) -> Self {
        Self::new(VmErrorKind::Reflect(value.kind))
    }
}

pub type NativeFunction = Arc<dyn Fn(&[Value]) -> VmResult<Value> + Send + Sync + 'static>;
pub type HostNativeFunction = Arc<
    dyn for<'host> Fn(&[Value], &mut HostExecution<'host>) -> VmResult<Value>
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone, Default)]
pub struct Vm {
    natives: HashMap<String, NativeFunction>,
    host_natives: HashMap<String, HostNativeFunction>,
}

pub struct HostExecution<'host> {
    pub adapter: &'host mut dyn ScriptStateAdapter,
    pub tx: &'host mut PatchTx,
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

    pub fn register_host_native(
        &mut self,
        name: impl Into<String>,
        function: impl for<'host> Fn(&[Value], &mut HostExecution<'host>) -> VmResult<Value>
        + Send
        + Sync
        + 'static,
    ) {
        self.host_natives.insert(name.into(), Arc::new(function));
    }

    pub fn register_reflection_natives(&mut self, registry: Arc<TypeRegistry>) {
        let type_of_registry = Arc::clone(&registry);
        self.register_host_native("reflect.type_of", move |args, _host| {
            expect_arity("reflect.type_of", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.type_of")?;
            Ok(reflect::type_of(&type_of_registry, &target)
                .map_or(Value::Null, |desc| Value::String(desc.key.name.clone())))
        });

        let fields_registry = Arc::clone(&registry);
        self.register_host_native("reflect.fields", move |args, _host| {
            expect_arity("reflect.fields", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.fields")?;
            let Some(desc) = reflect::type_of(&fields_registry, &target) else {
                return Ok(Value::Null);
            };
            let fields = reflect::fields(&fields_registry, &desc.key)
                .unwrap_or(&[])
                .iter()
                .map(|field| Value::String(field.name.clone()))
                .collect();
            Ok(Value::Array(fields))
        });

        let get_registry = Arc::clone(&registry);
        self.register_host_native("reflect.get", move |args, host| {
            expect_arity("reflect.get", args, 2)?;
            let target = value_to_reflect(&args[0], "reflect.get")?;
            let field = expect_string(&args[1], "reflect.get")?;
            let adapter: &dyn ScriptStateAdapter = &*host.adapter;
            let mut ctx = reflect::ReflectContext {
                registry: &get_registry,
                adapter,
                tx: &mut *host.tx,
            };
            let value = reflect::get(&mut ctx, &target, field)?;
            value_from_reflect(value)
        });

        let set_registry = Arc::clone(&registry);
        self.register_host_native("reflect.set", move |args, host| {
            expect_arity("reflect.set", args, 3)?;
            let target = value_to_reflect(&args[0], "reflect.set")?;
            let field = expect_string(&args[1], "reflect.set")?;
            let value = value_to_reflect(&args[2], "reflect.set")?;
            let adapter: &dyn ScriptStateAdapter = &*host.adapter;
            let mut ctx = reflect::ReflectContext {
                registry: &set_registry,
                adapter,
                tx: &mut *host.tx,
            };
            reflect::set(&mut ctx, &target, field, value)?;
            Ok(Value::Null)
        });

        let call_registry = Arc::clone(&registry);
        self.register_host_native("reflect.call", move |args, host| {
            if args.len() < 2 {
                return Err(VmError::new(VmErrorKind::ArityMismatch {
                    name: "reflect.call".to_owned(),
                    expected: 2,
                    actual: args.len(),
                }));
            }
            let target = value_to_reflect(&args[0], "reflect.call")?;
            let method = expect_string(&args[1], "reflect.call")?;
            let call_args = args[2..]
                .iter()
                .map(|arg| value_to_reflect(arg, "reflect.call"))
                .collect::<VmResult<Vec<_>>>()?;
            let adapter: &dyn ScriptStateAdapter = &*host.adapter;
            let mut ctx = reflect::ReflectContext {
                registry: &call_registry,
                adapter,
                tx: &mut *host.tx,
            };
            let value = reflect::call(&mut ctx, &target, method, call_args)?;
            value_from_reflect(value)
        });

        self.register_host_native("reflect.implements", move |args, _host| {
            expect_arity("reflect.implements", args, 2)?;
            let target = value_to_reflect(&args[0], "reflect.implements")?;
            let trait_name = expect_string(&args[1], "reflect.implements")?;
            Ok(Value::Bool(reflect::implements(
                &registry, &target, trait_name,
            )?))
        });
    }

    pub fn run(&self, code: &CodeObject) -> VmResult<Value> {
        self.execute(code, None, &[], None)
    }

    pub fn run_program(&self, program: &Program, entry: &str, args: &[Value]) -> VmResult<Value> {
        let code = program.function(entry).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
        self.execute(code, Some(program), args, None)
    }

    pub fn run_with_host(
        &self,
        code: &CodeObject,
        host: &mut HostExecution<'_>,
    ) -> VmResult<Value> {
        self.execute(code, None, &[], Some(host))
    }

    pub fn run_program_with_host(
        &self,
        program: &Program,
        entry: &str,
        args: &[Value],
        host: &mut HostExecution<'_>,
    ) -> VmResult<Value> {
        let code = program.function(entry).ok_or_else(|| {
            VmError::new(VmErrorKind::UnknownFunction {
                name: entry.to_owned(),
            })
        })?;
        self.execute(code, Some(program), args, Some(host))
    }

    fn execute(
        &self,
        code: &CodeObject,
        program: Option<&Program>,
        args: &[Value],
        mut host: Option<&mut HostExecution<'_>>,
    ) -> VmResult<Value> {
        if code.params.len() != args.len() {
            return Err(VmError::new(VmErrorKind::ArityMismatch {
                name: code.name.clone(),
                expected: code.params.len(),
                actual: args.len(),
            }));
        }

        let mut frame = CallFrame::new(code.register_count);
        for (index, arg) in args.iter().enumerate() {
            frame.write(
                Register(u16::try_from(index).map_err(|_| {
                    VmError::new(VmErrorKind::RegisterOutOfBounds {
                        register: Register(u16::MAX),
                    })
                })?),
                arg.clone(),
            )?;
        }
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
                InstructionKind::Rem { dst, lhs, rhs } => {
                    let value = rem_numeric(frame.read(*lhs)?, frame.read(*rhs)?)?;
                    frame.write(*dst, value)?;
                }
                InstructionKind::Equal { dst, lhs, rhs } => {
                    let value = Value::Bool(frame.read(*lhs)? == frame.read(*rhs)?);
                    frame.write(*dst, value)?;
                }
                InstructionKind::NotEqual { dst, lhs, rhs } => {
                    let value = Value::Bool(frame.read(*lhs)? != frame.read(*rhs)?);
                    frame.write(*dst, value)?;
                }
                InstructionKind::Less { dst, lhs, rhs } => {
                    let value =
                        compare_numeric(frame.read(*lhs)?, frame.read(*rhs)?, "less", |a, b| {
                            a < b
                        })?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::LessEqual { dst, lhs, rhs } => {
                    let value = compare_numeric(
                        frame.read(*lhs)?,
                        frame.read(*rhs)?,
                        "less_equal",
                        |a, b| a <= b,
                    )?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::Greater { dst, lhs, rhs } => {
                    let value = compare_numeric(
                        frame.read(*lhs)?,
                        frame.read(*rhs)?,
                        "greater",
                        |a, b| a > b,
                    )?;
                    frame.write(*dst, Value::Bool(value))?;
                }
                InstructionKind::GreaterEqual { dst, lhs, rhs } => {
                    let value = compare_numeric(
                        frame.read(*lhs)?,
                        frame.read(*rhs)?,
                        "greater_equal",
                        |a, b| a >= b,
                    )?;
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
                    let values = args
                        .iter()
                        .map(|register| frame.read(*register).cloned())
                        .collect::<VmResult<Vec<_>>>()?;
                    let result = if let Some(native) = self.natives.get(name) {
                        native(&values)?
                    } else if let Some(native) = self.host_natives.get(name) {
                        let host = host.as_deref_mut().ok_or_else(|| {
                            VmError::new(VmErrorKind::TypeMismatch {
                                operation: "host context",
                            })
                        })?;
                        native(&values, host)?
                    } else {
                        return Err(VmError::new(VmErrorKind::UnknownNative {
                            name: name.clone(),
                        }));
                    };
                    if let Some(dst) = dst {
                        frame.write(*dst, result)?;
                    }
                }
                InstructionKind::CallFunction { dst, name, args } => {
                    let program = program.ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownFunction { name: name.clone() })
                    })?;
                    let function = program.function(name).ok_or_else(|| {
                        VmError::new(VmErrorKind::UnknownFunction { name: name.clone() })
                    })?;
                    let values = args
                        .iter()
                        .map(|register| frame.read(*register).cloned())
                        .collect::<VmResult<Vec<_>>>()?;
                    let result =
                        self.execute(function, Some(program), &values, host.as_deref_mut())?;
                    frame.write(*dst, result)?;
                }
                InstructionKind::MakeArray { dst, elements } => {
                    let values = elements
                        .iter()
                        .map(|register| frame.read(*register).cloned())
                        .collect::<VmResult<Vec<_>>>()?;
                    frame.write(*dst, Value::Array(values))?;
                }
                InstructionKind::MakeMap { dst, entries } => {
                    let mut values = BTreeMap::new();
                    for (key, register) in entries {
                        values.insert(key.clone(), frame.read(*register)?.clone());
                    }
                    frame.write(*dst, Value::Map(values))?;
                }
                InstructionKind::GetHostField { dst, root, field } => {
                    let root = expect_host_ref(frame.read(*root)?, "get_host_field")?;
                    let path = HostPath::new(root).field(*field);
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    let value = host.tx.read_path(host.adapter, &path)?;
                    frame.write(*dst, value_from_host(value))?;
                }
                InstructionKind::SetHostField { root, field, src } => {
                    let root = expect_host_ref(frame.read(*root)?, "set_host_field")?;
                    let value = value_to_host(frame.read(*src)?, "set_host_field")?;
                    let path = HostPath::new(root).field(*field);
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    host.tx.set_path(path, value, instruction.span)?;
                }
                InstructionKind::AddHostField { root, field, rhs } => {
                    let root = expect_host_ref(frame.read(*root)?, "add_host_field")?;
                    let value = value_to_host(frame.read(*rhs)?, "add_host_field")?;
                    let path = HostPath::new(root).field(*field);
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    let base_value = host.tx.read_path(host.adapter, &path)?;
                    host.tx
                        .add_path(path, value, base_value, instruction.span)?;
                }
                InstructionKind::CallHostMethod {
                    dst,
                    root,
                    method,
                    args,
                } => {
                    let root = expect_host_ref(frame.read(*root)?, "call_host_method")?;
                    let path = HostPath::new(root);
                    let values = args
                        .iter()
                        .map(|register| value_to_host(frame.read(*register)?, "call_host_method"))
                        .collect::<VmResult<Vec<_>>>()?;
                    let host = host.as_deref_mut().ok_or_else(|| {
                        VmError::new(VmErrorKind::TypeMismatch {
                            operation: "host context",
                        })
                    })?;
                    host.tx
                        .call_method(path, *method, values, instruction.span)?;
                    if let Some(dst) = dst {
                        frame.write(*dst, Value::Null)?;
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

fn rem_numeric(lhs: &Value, rhs: &Value) -> VmResult<Value> {
    match (lhs, rhs) {
        (Value::Int(_), Value::Int(0)) => Err(VmError::new(VmErrorKind::DivisionByZero)),
        (Value::Int(lhs), Value::Int(rhs)) => Ok(Value::Int(lhs % rhs)),
        (Value::Float(_), Value::Float(rhs)) if *rhs == 0.0 => {
            Err(VmError::new(VmErrorKind::DivisionByZero))
        }
        (Value::Float(lhs), Value::Float(rhs)) => Ok(Value::Float(lhs % rhs)),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation: "rem" })),
    }
}

fn expect_host_ref(value: &Value, operation: &'static str) -> VmResult<HostRef> {
    match value {
        Value::HostRef(value) => Ok(*value),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn value_from_host(value: HostValue) -> Value {
    match value {
        HostValue::Null => Value::Null,
        HostValue::Bool(value) => Value::Bool(value),
        HostValue::Int(value) => Value::Int(value),
        HostValue::Float(value) => Value::Float(value),
        HostValue::String(value) => Value::String(value),
    }
}

fn value_to_host(value: &Value, operation: &'static str) -> VmResult<HostValue> {
    match value {
        Value::Null => Ok(HostValue::Null),
        Value::Bool(value) => Ok(HostValue::Bool(*value)),
        Value::Int(value) => Ok(HostValue::Int(*value)),
        Value::Float(value) => Ok(HostValue::Float(*value)),
        Value::String(value) => Ok(HostValue::String(value.clone())),
        Value::Array(_) | Value::Map(_) | Value::HostRef(_) => {
            Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
        }
    }
}

fn value_to_reflect(value: &Value, operation: &'static str) -> VmResult<reflect::ReflectValue> {
    match value {
        Value::HostRef(host_ref) => Ok(reflect::ReflectValue::HostRef(*host_ref)),
        Value::Map(values) => {
            let values = values
                .iter()
                .map(|(key, value)| Ok((key.clone(), value_to_reflect(value, operation)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(reflect::ReflectValue::Record(values))
        }
        Value::Array(_) => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
        Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_) => Ok(
            reflect::ReflectValue::Host(value_to_host(value, operation)?),
        ),
    }
}

fn value_from_reflect(value: reflect::ReflectValue) -> VmResult<Value> {
    match value {
        reflect::ReflectValue::Host(value) => Ok(value_from_host(value)),
        reflect::ReflectValue::HostRef(host_ref) => Ok(Value::HostRef(host_ref)),
        reflect::ReflectValue::Record(values) => {
            let values = values
                .into_iter()
                .map(|(key, value)| Ok((key, value_from_reflect(value)?)))
                .collect::<VmResult<BTreeMap<_, _>>>()?;
            Ok(Value::Map(values))
        }
    }
}

fn expect_string<'a>(value: &'a Value, operation: &'static str) -> VmResult<&'a str> {
    match value {
        Value::String(value) => Ok(value),
        Value::Null
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::Array(_)
        | Value::Map(_)
        | Value::HostRef(_) => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn expect_arity(name: &str, args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(VmError::new(VmErrorKind::ArityMismatch {
            name: name.to_owned(),
            expected,
            actual: args.len(),
        }))
    }
}

fn compare_numeric(
    lhs: &Value,
    rhs: &Value,
    operation: &'static str,
    compare: impl FnOnce(f64, f64) -> bool,
) -> VmResult<bool> {
    match (lhs, rhs) {
        (Value::Int(lhs), Value::Int(rhs)) => Ok(compare(*lhs as f64, *rhs as f64)),
        (Value::Float(lhs), Value::Float(rhs)) => Ok(compare(*lhs, *rhs)),
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
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use vela_bytecode::compiler::{
        CompilerOptions, compile_function_source, compile_program_source,
        compile_program_source_with_options,
    };
    use vela_bytecode::{ConstantId, Instruction, InstructionOffset};
    use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, TypeId};
    use vela_host::{HostValue, MockStateAdapter, PatchOp};
    use vela_reflect::{FieldDesc, MethodDesc, TraitDesc, TypeDesc, TypeKey};

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

    #[test]
    fn runs_compiled_script_function_calls() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn add_bonus(value) {
    return value + 5;
}

fn main() {
    let base = 10;
    return add_bonus(base) * 2;
}
"#,
        )
        .expect("compile program source");

        assert_eq!(
            Vm::new().run_program(&program, "main", &[]),
            Ok(Value::Int(30))
        );
    }

    #[test]
    fn passes_arguments_to_program_entry() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn double(value) {
    return value * 2;
}
"#,
        )
        .expect("compile program source");

        assert_eq!(
            Vm::new().run_program(&program, "double", &[Value::Int(9)]),
            Ok(Value::Int(18))
        );
    }

    #[test]
    fn runs_compiled_array_literal_source() {
        let code = compile_function_source(
            SourceId::new(1),
            "fn main() { return [1, 2 + 3, \"gold\"]; }",
            "main",
        )
        .expect("compile array literal source");

        assert_eq!(
            Vm::new().run(&code),
            Ok(Value::Array(vec![
                Value::Int(1),
                Value::Int(5),
                Value::String("gold".into())
            ]))
        );
    }

    #[test]
    fn runs_compiled_map_literal_source() {
        let code = compile_function_source(
            SourceId::new(1),
            "fn main() { return {\"level\": 2, exp: 10 + 5}; }",
            "main",
        )
        .expect("compile map literal source");
        let mut expected = BTreeMap::new();
        expected.insert("level".into(), Value::Int(2));
        expected.insert("exp".into(), Value::Int(15));

        assert_eq!(Vm::new().run(&code), Ok(Value::Map(expected)));
    }

    #[test]
    fn runs_compiled_if_then_branch_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    if 2 < 3 {
        return 10;
    } else {
        return 20;
    }
}
"#,
            "main",
        )
        .expect("compile if source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(10)));
    }

    #[test]
    fn runs_compiled_if_else_branch_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    if 3 < 2 {
        return 10;
    } else {
        return 20;
    }
}
"#,
            "main",
        )
        .expect("compile if source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(20)));
    }

    #[test]
    fn runs_compiled_comparison_and_remainder_source() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    if 10 % 4 == 2 {
        if 3 >= 3 {
            if 2 <= 5 {
                if 5 != 6 {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
            "main",
        )
        .expect("compile operator source");

        assert_eq!(Vm::new().run(&code), Ok(Value::Int(1)));
    }

    #[test]
    fn reads_host_field_through_patch_transaction() {
        let (program, host_ref) = host_read_program();
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        let result = Vm::new().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(host_ref)],
            &mut host,
        );

        assert_eq!(result, Ok(Value::Int(9)));
    }

    #[test]
    fn set_host_field_records_patch_and_overlay_read() {
        let host_ref = player_ref(3);
        let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
        let ten = code.push_constant(Constant::Int(10));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: ten,
        }));
        code.push_instruction(Instruction::new(InstructionKind::SetHostField {
            root: Register(0),
            field: level_field(),
            src: Register(1),
        }));
        code.push_instruction(Instruction::new(InstructionKind::GetHostField {
            dst: Register(2),
            root: Register(0),
            field: level_field(),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(2),
        }));
        let mut program = Program::new();
        program.insert_function(code);
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(10)));
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(9))
        );
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
        tx.apply(&mut adapter).expect("apply patches");
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(10))
        );
    }

    #[test]
    fn add_host_field_records_patch_and_overlay_read() {
        let host_ref = player_ref(3);
        let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
        let one = code.push_constant(Constant::Int(1));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: one,
        }));
        code.push_instruction(Instruction::new(InstructionKind::AddHostField {
            root: Register(0),
            field: level_field(),
            rhs: Register(1),
        }));
        code.push_instruction(Instruction::new(InstructionKind::GetHostField {
            dst: Register(2),
            root: Register(0),
            field: level_field(),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(2),
        }));
        let mut program = Program::new();
        program.insert_function(code);
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(10)));
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));
        tx.apply(&mut adapter).expect("apply patches");
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(10))
        );
    }

    #[test]
    fn host_field_read_rejects_stale_generation() {
        let (program, _host_ref) = host_read_program();
        let fresh_ref = player_ref(3);
        let stale_ref = player_ref(2);
        let mut adapter = host_adapter(fresh_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        let error = Vm::new()
            .run_program_with_host(&program, "main", &[Value::HostRef(stale_ref)], &mut host)
            .expect_err("stale host read");

        assert_eq!(
            error.kind,
            VmErrorKind::Host(vela_host::HostErrorKind::StaleGeneration {
                expected: 2,
                actual: 3
            })
        );
    }

    #[test]
    fn compiled_source_mutates_host_field_through_patch_tx() {
        let host_ref = player_ref(3);
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.level = 10;
    player.level += 1;
    return player.level;
}
"#,
            &CompilerOptions::new().with_host_field("level", level_field()),
        )
        .expect("compile host field source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Int(11)));
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(9))
        );
        assert_eq!(tx.patches().len(), 2);
        assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
        assert_eq!(tx.patches()[1].op, PatchOp::Add(HostValue::Int(1)));
        tx.apply(&mut adapter).expect("apply patches");
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(11))
        );
    }

    #[test]
    fn compiled_source_uses_reflection_natives_for_host_state() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    if reflect.type_of(player) == "Player" {
        if reflect.implements(player, "Damageable") {
            reflect.set(player, "level", 10);
            return reflect.get(player, "level");
        }
    }
    return 0;
}
"#,
        )
        .expect("compile reflection source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        };

        assert_eq!(result, Ok(Value::Int(10)));
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(9))
        );
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
        tx.apply(&mut adapter).expect("apply reflection patch");
        assert_eq!(
            adapter.read_path(&level_path(host_ref)),
            Ok(HostValue::Int(10))
        );
    }

    #[test]
    fn compiled_source_reflection_fields_returns_metadata() {
        let host_ref = player_ref(3);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return reflect.fields(player);
}
"#,
        )
        .expect("compile reflection fields source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));
        let mut host = HostExecution {
            adapter: &mut adapter,
            tx: &mut tx,
        };

        let result =
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host);

        assert_eq!(
            result,
            Ok(Value::Array(vec![
                Value::String("id".into()),
                Value::String("level".into())
            ]))
        );
    }

    #[test]
    fn compiled_source_reflect_call_records_host_method_patch() {
        let host_ref = player_ref(3);
        let method = HostMethodId::new(5);
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn main(player) {
    reflect.call(player, "grant_exp", 20);
    return 1;
}
"#,
        )
        .expect("compile reflection call source");
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        adapter.insert_method_return(method, HostValue::Null);
        let mut tx = PatchTx::new();
        let mut vm = Vm::new();
        vm.register_reflection_natives(Arc::new(reflection_registry()));

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            vm.run_program_with_host(&program, "main", &[Value::HostRef(host_ref)], &mut host)
        };

        assert_eq!(result, Ok(Value::Int(1)));
        assert!(adapter.method_calls().is_empty());
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::CallHostMethod {
                method,
                args: vec![HostValue::Int(20)]
            }
        );
        tx.apply(&mut adapter).expect("apply reflection call");
        assert_eq!(
            adapter.method_calls(),
            &[(HostPath::new(host_ref), method, vec![HostValue::Int(20)])]
        );
    }

    #[test]
    fn call_host_method_records_patch_and_applies_later() {
        let host_ref = player_ref(3);
        let method = HostMethodId::new(8);
        let mut code = CodeObject::new("main", 3).with_params(vec!["player".into()]);
        let gold = code.push_constant(Constant::String("gold".into()));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(1),
            constant: gold,
        }));
        code.push_instruction(Instruction::new(InstructionKind::CallHostMethod {
            dst: Some(Register(2)),
            root: Register(0),
            method,
            args: vec![Register(1)],
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(2),
        }));
        let mut program = Program::new();
        program.insert_function(code);
        let mut adapter = host_adapter(host_ref, HostValue::Int(9));
        adapter.insert_method_return(method, HostValue::Null);
        let mut tx = PatchTx::new();

        let result = {
            let mut host = HostExecution {
                adapter: &mut adapter,
                tx: &mut tx,
            };
            Vm::new().run_program_with_host(
                &program,
                "main",
                &[Value::HostRef(host_ref)],
                &mut host,
            )
        };

        assert_eq!(result, Ok(Value::Null));
        assert!(adapter.method_calls().is_empty());
        assert_eq!(tx.patches().len(), 1);
        assert_eq!(
            tx.patches()[0].op,
            PatchOp::CallHostMethod {
                method,
                args: vec![HostValue::String("gold".into())]
            }
        );
        tx.apply(&mut adapter).expect("apply method call");
        assert_eq!(
            adapter.method_calls(),
            &[(
                HostPath::new(host_ref),
                method,
                vec![HostValue::String("gold".into())]
            )]
        );
    }

    fn host_read_program() -> (Program, HostRef) {
        let host_ref = player_ref(3);
        let mut code = CodeObject::new("main", 2).with_params(vec!["player".into()]);
        code.push_instruction(Instruction::new(InstructionKind::GetHostField {
            dst: Register(1),
            root: Register(0),
            field: level_field(),
        }));
        code.push_instruction(Instruction::new(InstructionKind::Return {
            src: Register(1),
        }));
        let mut program = Program::new();
        program.insert_function(code);
        (program, host_ref)
    }

    fn host_adapter(host_ref: HostRef, value: HostValue) -> MockStateAdapter {
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(level_path(host_ref), value);
        adapter
    }

    fn reflection_registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(FieldId::new(1), "id"))
                .field(FieldDesc::new(level_field(), "level").writable(true))
                .method(MethodDesc::new(HostMethodId::new(5), "grant_exp"))
                .trait_impl(TraitDesc::new("Damageable")),
        );
        registry
    }

    fn player_ref(generation: u32) -> HostRef {
        HostRef::new(HostTypeId::new(1), HostObjectId::new(7), generation)
    }

    fn level_path(host_ref: HostRef) -> HostPath {
        HostPath::new(host_ref).field(level_field())
    }

    fn level_field() -> FieldId {
        FieldId::new(2)
    }
}
