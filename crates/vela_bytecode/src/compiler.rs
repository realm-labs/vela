//! Minimal AST-to-bytecode compiler for the M2 VM loop.

use std::collections::{BTreeSet, HashMap};
use std::num::{ParseFloatError, ParseIntError};

use vela_common::{Diagnostic, FieldId, SourceId};
use vela_syntax::{
    AssignOp, BinaryOp, Block, ElseBranch, Expr, ExprKind, FunctionItem, IfExpr, ItemKind, Literal,
    MapEntry, SourceFile, Stmt, StmtKind, parse_source,
};

use crate::{
    CodeObject, Constant, Instruction, InstructionKind, InstructionOffset, Program, Register,
};

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompilerOptions {
    host_fields: HashMap<String, FieldId>,
}

impl CompilerOptions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_host_field(mut self, name: impl Into<String>, field: FieldId) -> Self {
        self.host_fields.insert(name.into(), field);
        self
    }
}

pub fn compile_function_source(
    source: SourceId,
    text: &str,
    function_name: &str,
) -> CompileResult<CodeObject> {
    compile_function_source_with_options(source, text, function_name, &CompilerOptions::default())
}

pub fn compile_function_source_with_options(
    source: SourceId,
    text: &str,
    function_name: &str,
    options: &CompilerOptions,
) -> CompileResult<CodeObject> {
    let parsed = parse_checked_source(source, text)?;
    let script_functions = script_function_names(&parsed);

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

    Compiler::new(function, script_functions, options.clone())?.compile()
}

pub fn compile_program_source(source: SourceId, text: &str) -> CompileResult<Program> {
    compile_program_source_with_options(source, text, &CompilerOptions::default())
}

pub fn compile_program_source_with_options(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
) -> CompileResult<Program> {
    let parsed = parse_checked_source(source, text)?;
    let script_functions = script_function_names(&parsed);
    let mut program = Program::new();

    for item in &parsed.items {
        if let ItemKind::Function(function) = &item.kind {
            program.insert_function(
                Compiler::new(function, script_functions.clone(), options.clone())?.compile()?,
            );
        }
    }

    Ok(program)
}

fn parse_checked_source(source: SourceId, text: &str) -> CompileResult<SourceFile> {
    let parsed = parse_source(source, text);
    if parsed.diagnostics.is_empty() {
        Ok(parsed)
    } else {
        Err(CompileError::new(CompileErrorKind::SyntaxDiagnostics(
            parsed.diagnostics,
        )))
    }
}

fn script_function_names(parsed: &SourceFile) -> BTreeSet<String> {
    parsed
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            ItemKind::Function(function) => Some(function.name.clone()),
            _ => None,
        })
        .collect()
}

struct Compiler<'ast> {
    code: CodeObject,
    locals: HashMap<String, Register>,
    next_register: u16,
    body: &'ast Block,
    script_functions: BTreeSet<String>,
    options: CompilerOptions,
}

impl<'ast> Compiler<'ast> {
    fn new(
        function: &'ast FunctionItem,
        script_functions: BTreeSet<String>,
        options: CompilerOptions,
    ) -> CompileResult<Self> {
        let param_count = u16::try_from(function.params.len())
            .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?;
        let mut locals = HashMap::new();
        for (index, param) in function.params.iter().enumerate() {
            let register = u16::try_from(index)
                .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?;
            locals.insert(param.clone(), Register(register));
        }

        Ok(Self {
            code: CodeObject::new(function.name.clone(), 0).with_params(function.params.clone()),
            locals,
            next_register: param_count,
            body: &function.body,
            script_functions,
            options,
        })
    }

    fn compile(mut self) -> CompileResult<CodeObject> {
        let returned = self.compile_statements(&self.body.statements)?;
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
                if let ExprKind::If(if_expr) = &expr.kind {
                    return self.compile_if(if_expr);
                }
                if let ExprKind::Assign { .. } = &expr.kind {
                    self.compile_assignment(expr)?;
                    return Ok(false);
                }
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
            ExprKind::Path(path) => self.compile_host_path(path),
            ExprKind::Binary { op, left, right } => self.compile_binary(*op, left, right),
            ExprKind::Field { base, name } => {
                let root = self.compile_expr(base)?;
                let field = self.host_field(name)?;
                let dst = self.alloc_register()?;
                self.emit(InstructionKind::GetHostField { dst, root, field });
                Ok(dst)
            }
            ExprKind::Call { callee, args } => {
                let name = callable_name(callee)?;
                let arg_registers = args
                    .iter()
                    .map(|arg| self.compile_expr(&arg.value))
                    .collect::<CompileResult<Vec<_>>>()?;
                let dst = self.alloc_register()?;
                if self.script_functions.contains(&name) {
                    self.emit(InstructionKind::CallFunction {
                        dst,
                        name,
                        args: arg_registers,
                    });
                } else {
                    self.emit(InstructionKind::CallNative {
                        dst: Some(dst),
                        name,
                        args: arg_registers,
                    });
                }
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
            ExprKind::Array(items) => {
                let elements = items
                    .iter()
                    .map(|item| self.compile_expr(item))
                    .collect::<CompileResult<Vec<_>>>()?;
                let dst = self.alloc_register()?;
                self.emit(InstructionKind::MakeArray { dst, elements });
                Ok(dst)
            }
            ExprKind::Map(entries) => {
                let entries = entries
                    .iter()
                    .map(|entry| self.compile_map_entry(entry))
                    .collect::<CompileResult<Vec<_>>>()?;
                let dst = self.alloc_register()?;
                self.emit(InstructionKind::MakeMap { dst, entries });
                Ok(dst)
            }
            ExprKind::If(if_expr) => {
                let returned = self.compile_if(if_expr)?;
                if returned {
                    Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "returning if expression",
                    )))
                } else {
                    self.emit_constant(Constant::Null)
                }
            }
            ExprKind::Assign { .. } => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "assignment expression",
            ))),
            ExprKind::SelfValue
            | ExprKind::Unary { .. }
            | ExprKind::Index { .. }
            | ExprKind::Try(_)
            | ExprKind::Record { .. }
            | ExprKind::Lambda { .. }
            | ExprKind::Match(_)
            | ExprKind::Error => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "expression",
            ))),
        }
    }

    fn compile_assignment(&mut self, expr: &Expr) -> CompileResult<()> {
        let ExprKind::Assign { op, target, value } = &expr.kind else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "assignment statement",
            )));
        };
        let (root, field) = self.compile_host_assignment_target(target)?;
        let src = self.compile_expr(value)?;
        match op {
            AssignOp::Set => self.emit(InstructionKind::SetHostField { root, field, src }),
            AssignOp::Add => self.emit(InstructionKind::AddHostField {
                root,
                field,
                rhs: src,
            }),
            AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "compound assignment operator",
                )));
            }
        }
        Ok(())
    }

    fn compile_host_path(&mut self, path: &[String]) -> CompileResult<Register> {
        let (root, field) = self.compile_host_path_parts(path)?;
        let dst = self.alloc_register()?;
        self.emit(InstructionKind::GetHostField { dst, root, field });
        Ok(dst)
    }

    fn compile_host_assignment_target(
        &mut self,
        target: &Expr,
    ) -> CompileResult<(Register, FieldId)> {
        match &target.kind {
            ExprKind::Field { base, name } => {
                let root = self.compile_expr(base)?;
                let field = self.host_field(name)?;
                Ok((root, field))
            }
            ExprKind::Path(path) => self.compile_host_path_parts(path),
            _ => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "assignment target",
            ))),
        }
    }

    fn compile_host_path_parts(&mut self, path: &[String]) -> CompileResult<(Register, FieldId)> {
        if path.len() != 2 {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "host path",
            )));
        }
        let root =
            self.locals.get(&path[0]).copied().ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnknownLocal(path[0].clone()))
            })?;
        let field = self.host_field(&path[1])?;
        Ok((root, field))
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
            BinaryOp::Rem => InstructionKind::Rem { dst, lhs, rhs },
            BinaryOp::Equal => InstructionKind::Equal { dst, lhs, rhs },
            BinaryOp::NotEqual => InstructionKind::NotEqual { dst, lhs, rhs },
            BinaryOp::Less => InstructionKind::Less { dst, lhs, rhs },
            BinaryOp::LessEqual => InstructionKind::LessEqual { dst, lhs, rhs },
            BinaryOp::Greater => InstructionKind::Greater { dst, lhs, rhs },
            BinaryOp::GreaterEqual => InstructionKind::GreaterEqual { dst, lhs, rhs },
            BinaryOp::Or | BinaryOp::And => {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "binary operator",
                )));
            }
        };
        self.emit(instruction);
        Ok(dst)
    }

    fn compile_if(&mut self, if_expr: &IfExpr) -> CompileResult<bool> {
        let condition = self.compile_expr(&if_expr.condition)?;
        let jump_to_else = self.emit_jump_if_false(condition);

        let then_returned = self.compile_statements(&if_expr.then_branch.statements)?;
        let jump_to_end = if then_returned {
            None
        } else {
            Some(self.emit_jump())
        };

        self.patch_jump(jump_to_else, self.current_offset())?;

        let else_returned = match &if_expr.else_branch {
            Some(ElseBranch::Block(block)) => self.compile_statements(&block.statements)?,
            Some(ElseBranch::If(if_expr)) => self.compile_if(if_expr)?,
            None => false,
        };

        if let Some(jump_to_end) = jump_to_end {
            self.patch_jump(jump_to_end, self.current_offset())?;
        }

        Ok(then_returned && else_returned)
    }

    fn compile_map_entry(&mut self, entry: &MapEntry) -> CompileResult<(String, Register)> {
        let key = map_key_name(&entry.key)?;
        let value = self.compile_expr(&entry.value)?;
        Ok((key, value))
    }

    fn host_field(&self, name: &str) -> CompileResult<FieldId> {
        self.options
            .host_fields
            .get(name)
            .copied()
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax("host field")))
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

    fn emit_jump_if_false(&mut self, condition: Register) -> usize {
        let offset = self.current_offset();
        self.emit(InstructionKind::JumpIfFalse {
            condition,
            target: InstructionOffset(usize::MAX),
        });
        offset
    }

    fn emit_jump(&mut self) -> usize {
        let offset = self.current_offset();
        self.emit(InstructionKind::Jump {
            target: InstructionOffset(usize::MAX),
        });
        offset
    }

    fn patch_jump(&mut self, offset: usize, target: usize) -> CompileResult<()> {
        let instruction =
            self.code.instructions.get_mut(offset).ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnsupportedSyntax("jump patch"))
            })?;
        match &mut instruction.kind {
            InstructionKind::JumpIfFalse {
                target: jump_target,
                ..
            }
            | InstructionKind::Jump {
                target: jump_target,
            } => {
                *jump_target = InstructionOffset(target);
                Ok(())
            }
            _ => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "jump patch",
            ))),
        }
    }

    fn current_offset(&self) -> usize {
        self.code.instructions.len()
    }
}

fn callable_name(callee: &Expr) -> CompileResult<String> {
    match &callee.kind {
        ExprKind::Path(path) => Ok(path.join(".")),
        _ => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "callable expression",
        ))),
    }
}

fn map_key_name(key: &Expr) -> CompileResult<String> {
    match &key.kind {
        ExprKind::Literal(Literal::String(value))
        | ExprKind::Literal(Literal::Int(value))
        | ExprKind::Literal(Literal::Float(value)) => Ok(value.clone()),
        ExprKind::Path(path) => Ok(path.join(".")),
        _ => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "map key",
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
