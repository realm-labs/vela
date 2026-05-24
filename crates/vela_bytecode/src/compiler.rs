//! Minimal AST-to-bytecode compiler for the M2 VM loop.

use std::collections::HashMap;
use std::num::{ParseFloatError, ParseIntError};

use vela_common::{Diagnostic, SourceId};
use vela_syntax::{
    BinaryOp, Block, Expr, ExprKind, ItemKind, Literal, Stmt, StmtKind, parse_source,
};

use crate::{CodeObject, Constant, Instruction, InstructionKind, Register};

#[derive(Clone, Debug, PartialEq)]
pub struct CompileError {
    pub kind: CompileErrorKind,
}

impl CompileError {
    fn new(kind: CompileErrorKind) -> Self {
        Self { kind }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CompileErrorKind {
    SyntaxDiagnostics(Vec<Diagnostic>),
    FunctionNotFound(String),
    UnknownLocal(String),
    InvalidIntLiteral { literal: String, error: String },
    InvalidFloatLiteral { literal: String, error: String },
    RegisterOverflow,
    UnsupportedSyntax(&'static str),
}

pub type CompileResult<T> = Result<T, CompileError>;

pub fn compile_function_source(
    source: SourceId,
    text: &str,
    function_name: &str,
) -> CompileResult<CodeObject> {
    let parsed = parse_source(source, text);
    if !parsed.diagnostics.is_empty() {
        return Err(CompileError::new(CompileErrorKind::SyntaxDiagnostics(
            parsed.diagnostics,
        )));
    }

    let function = parsed
        .items
        .iter()
        .find_map(|item| match &item.kind {
            ItemKind::Function(function) if function.name == function_name => Some(function),
            _ => None,
        })
        .ok_or_else(|| {
            CompileError::new(CompileErrorKind::FunctionNotFound(function_name.to_owned()))
        })?;

    Compiler::new(function.name.clone()).compile_block(&function.body)
}

struct Compiler {
    code: CodeObject,
    locals: HashMap<String, Register>,
    next_register: u16,
}

impl Compiler {
    fn new(name: String) -> Self {
        Self {
            code: CodeObject::new(name, 0),
            locals: HashMap::new(),
            next_register: 0,
        }
    }

    fn compile_block(mut self, block: &Block) -> CompileResult<CodeObject> {
        let returned = self.compile_statements(&block.statements)?;
        if !returned {
            let null = self.emit_constant(Constant::Null)?;
            self.emit(InstructionKind::Return { src: null });
        }
        self.code.register_count = self.next_register;
        Ok(self.code)
    }

    fn compile_statements(&mut self, statements: &[Stmt]) -> CompileResult<bool> {
        for stmt in statements {
            if self.compile_statement(stmt)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn compile_statement(&mut self, stmt: &Stmt) -> CompileResult<bool> {
        match &stmt.kind {
            StmtKind::Let { name, value } => {
                let register = if let Some(value) = value {
                    self.compile_expr(value)?
                } else {
                    self.emit_constant(Constant::Null)?
                };
                self.locals.insert(name.clone(), register);
                Ok(false)
            }
            StmtKind::Return(value) => {
                let register = if let Some(value) = value {
                    self.compile_expr(value)?
                } else {
                    self.emit_constant(Constant::Null)?
                };
                self.emit(InstructionKind::Return { src: register });
                Ok(true)
            }
            StmtKind::Expr(expr) => {
                self.compile_expr(expr)?;
                Ok(false)
            }
            StmtKind::Block(block) => self.compile_statements(&block.statements),
            StmtKind::Break | StmtKind::Continue | StmtKind::For { .. } => Err(CompileError::new(
                CompileErrorKind::UnsupportedSyntax("control-flow statement"),
            )),
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> CompileResult<Register> {
        match &expr.kind {
            ExprKind::Literal(literal) => self.compile_literal(literal),
            ExprKind::Path(path) if path.len() == 1 => {
                self.locals.get(&path[0]).copied().ok_or_else(|| {
                    CompileError::new(CompileErrorKind::UnknownLocal(path[0].clone()))
                })
            }
            ExprKind::Binary { op, left, right } => self.compile_binary(*op, left, right),
            ExprKind::Call { callee, args } => {
                let name = native_name(callee)?;
                let arg_registers = args
                    .iter()
                    .map(|arg| self.compile_expr(&arg.value))
                    .collect::<CompileResult<Vec<_>>>()?;
                let dst = self.alloc_register()?;
                self.emit(InstructionKind::CallNative {
                    dst: Some(dst),
                    name,
                    args: arg_registers,
                });
                Ok(dst)
            }
            ExprKind::Block(block) => {
                let returned = self.compile_statements(&block.statements)?;
                if returned {
                    Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "return inside block expression",
                    )))
                } else {
                    self.emit_constant(Constant::Null)
                }
            }
            ExprKind::Assign { .. } => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "assignment expression",
            ))),
            ExprKind::Path(_)
            | ExprKind::SelfValue
            | ExprKind::Unary { .. }
            | ExprKind::Field { .. }
            | ExprKind::Index { .. }
            | ExprKind::Try(_)
            | ExprKind::Array(_)
            | ExprKind::Map(_)
            | ExprKind::Record { .. }
            | ExprKind::Lambda { .. }
            | ExprKind::If(_)
            | ExprKind::Match(_)
            | ExprKind::Error => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "expression",
            ))),
        }
    }

    fn compile_literal(&mut self, literal: &Literal) -> CompileResult<Register> {
        let constant = match literal {
            Literal::Null => Constant::Null,
            Literal::Bool(value) => Constant::Bool(*value),
            Literal::Int(value) => Constant::Int(parse_int(value)?),
            Literal::Float(value) => Constant::Float(parse_float(value)?),
            Literal::String(value) => Constant::String(value.clone()),
        };
        self.emit_constant(constant)
    }

    fn compile_binary(
        &mut self,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
    ) -> CompileResult<Register> {
        let lhs = self.compile_expr(left)?;
        let rhs = self.compile_expr(right)?;
        let dst = self.alloc_register()?;
        let instruction = match op {
            BinaryOp::Add => InstructionKind::Add { dst, lhs, rhs },
            BinaryOp::Sub => InstructionKind::Sub { dst, lhs, rhs },
            BinaryOp::Mul => InstructionKind::Mul { dst, lhs, rhs },
            BinaryOp::Div => InstructionKind::Div { dst, lhs, rhs },
            BinaryOp::Equal => InstructionKind::Equal { dst, lhs, rhs },
            BinaryOp::Less => InstructionKind::Less { dst, lhs, rhs },
            BinaryOp::Greater => InstructionKind::Less {
                dst,
                lhs: rhs,
                rhs: lhs,
            },
            BinaryOp::Or
            | BinaryOp::And
            | BinaryOp::NotEqual
            | BinaryOp::LessEqual
            | BinaryOp::GreaterEqual
            | BinaryOp::Rem => {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "binary operator",
                )));
            }
        };
        self.emit(instruction);
        Ok(dst)
    }

    fn emit_constant(&mut self, constant: Constant) -> CompileResult<Register> {
        let dst = self.alloc_register()?;
        let constant = self.code.push_constant(constant);
        self.emit(InstructionKind::LoadConst { dst, constant });
        Ok(dst)
    }

    fn alloc_register(&mut self) -> CompileResult<Register> {
        let register = self.next_register;
        self.next_register = self
            .next_register
            .checked_add(1)
            .ok_or_else(|| CompileError::new(CompileErrorKind::RegisterOverflow))?;
        Ok(Register(register))
    }

    fn emit(&mut self, kind: InstructionKind) {
        self.code.push_instruction(Instruction::new(kind));
    }
}

fn native_name(callee: &Expr) -> CompileResult<String> {
    match &callee.kind {
        ExprKind::Path(path) => Ok(path.join(".")),
        _ => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "native callee",
        ))),
    }
}

fn parse_int(value: &str) -> CompileResult<i64> {
    value
        .replace('_', "")
        .parse()
        .map_err(|error: ParseIntError| {
            CompileError::new(CompileErrorKind::InvalidIntLiteral {
                literal: value.to_owned(),
                error: error.to_string(),
            })
        })
}

fn parse_float(value: &str) -> CompileResult<f64> {
    value
        .replace('_', "")
        .parse()
        .map_err(|error: ParseFloatError| {
            CompileError::new(CompileErrorKind::InvalidFloatLiteral {
                literal: value.to_owned(),
                error: error.to_string(),
            })
        })
}
