//! Minimal AST-to-bytecode compiler for the M2 VM loop.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::num::{ParseFloatError, ParseIntError};

use vela_common::{Diagnostic, FieldId, SourceId, Span};
use vela_hir::{
    BindingMap, BindingResolution, DeclarationKind, FunctionSignature, HirDeclId, HirLocalId,
    ImportResolution, LocalBindingKind, ModuleGraph, ModuleId, ModulePath, ModuleSource,
};
use vela_syntax::{
    AssignOp, BinaryOp, Block, ElseBranch, Expr, ExprKind, FunctionItem, IfExpr, ItemKind, Literal,
    MapEntry, MatchExpr, Pattern, RecordPatternField, SourceFile, Stmt, StmtKind, UnaryOp,
    parse_source,
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
    SemanticDiagnostics(Vec<Diagnostic>),
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

#[derive(Clone, Debug)]
struct CompilerFacts {
    script_function_symbols: BTreeMap<HirDeclId, String>,
    type_symbols: BTreeMap<HirDeclId, String>,
    const_values: BTreeMap<HirDeclId, Constant>,
    options: CompilerOptions,
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
    let semantic = parse_semantic_source(source, text)?;
    let script_function_symbols = semantic.script_function_symbols();
    let type_symbols = semantic.type_symbols();
    let const_values = semantic.const_values()?;
    let facts = CompilerFacts {
        script_function_symbols,
        type_symbols,
        const_values,
        options: options.clone(),
    };
    let (function, signature, bindings) = semantic.function(function_name).ok_or_else(|| {
        CompileError::new(CompileErrorKind::FunctionNotFound(function_name.to_owned()))
    })?;

    Compiler::new(function.name.clone(), function, signature, bindings, facts)?.compile()
}

pub fn compile_program_source(source: SourceId, text: &str) -> CompileResult<Program> {
    compile_program_source_with_options(source, text, &CompilerOptions::default())
}

pub fn compile_program_source_with_options(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
) -> CompileResult<Program> {
    let semantic = parse_semantic_source(source, text)?;
    let script_functions = semantic.script_function_names();
    let script_function_symbols = semantic.script_function_symbols();
    let type_symbols = semantic.type_symbols();
    let const_values = semantic.const_values()?;
    let facts = CompilerFacts {
        script_function_symbols,
        type_symbols,
        const_values,
        options: options.clone(),
    };
    let mut program = Program::new();

    for name in &script_functions {
        let (function, signature, bindings) = semantic
            .function(name)
            .expect("HIR function declarations come from parsed function items");
        program.insert_function(
            Compiler::new(
                function.name.clone(),
                function,
                signature,
                bindings,
                facts.clone(),
            )?
            .compile()?,
        );
    }

    Ok(program)
}

pub fn compile_module_sources(sources: &[ModuleSource]) -> CompileResult<Program> {
    compile_module_sources_with_options(sources, &CompilerOptions::default())
}

pub fn compile_module_sources_with_options(
    sources: &[ModuleSource],
    options: &CompilerOptions,
) -> CompileResult<Program> {
    let semantic = parse_semantic_modules(sources)?;
    let script_functions = semantic.script_function_declarations();
    let script_function_symbols = semantic.script_function_symbols();
    let type_symbols = semantic.type_symbols();
    let const_values = semantic.const_values()?;
    let facts = CompilerFacts {
        script_function_symbols,
        type_symbols,
        const_values,
        options: options.clone(),
    };
    let mut program = Program::new();

    for declaration in script_functions {
        let (function, signature, bindings) = semantic
            .function(declaration)
            .expect("HIR function declaration comes from parsed function item");
        let code_name = facts
            .script_function_symbols
            .get(&declaration)
            .expect("script function symbol exists for declaration")
            .clone();
        program.insert_function(
            Compiler::new(code_name, function, signature, bindings, facts.clone())?.compile()?,
        );
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

struct SemanticSource {
    parsed: SourceFile,
    graph: ModuleGraph,
    module: ModuleId,
}

struct SemanticModules {
    parsed: BTreeMap<ModuleId, SourceFile>,
    graph: ModuleGraph,
    modules: Vec<ModuleId>,
}

impl SemanticSource {
    fn function(&self, name: &str) -> Option<(&FunctionItem, &FunctionSignature, &BindingMap)> {
        let declaration = self.function_declaration(name)?;
        let signature = self.graph.function_signature(declaration)?;
        let bindings = self.graph.bindings(declaration)?;
        let function = self.parsed.items.iter().find_map(|item| match &item.kind {
            ItemKind::Function(function) if function.name == name => Some(function),
            _ => None,
        })?;
        Some((function, signature, bindings))
    }

    fn script_function_names(&self) -> BTreeSet<String> {
        let Some(declarations) = self.graph.module(self.module) else {
            return BTreeSet::new();
        };
        declarations
            .names()
            .filter_map(|name| {
                let declaration = declarations.get(name)?;
                let declaration = self.graph.declaration(declaration)?;
                (declaration.kind == DeclarationKind::Function).then(|| name.to_owned())
            })
            .collect()
    }

    fn script_function_symbols(&self) -> BTreeMap<HirDeclId, String> {
        let Some(declarations) = self.graph.module(self.module) else {
            return BTreeMap::new();
        };
        declarations
            .names()
            .filter_map(|name| {
                let declaration = declarations.get(name)?;
                let metadata = self.graph.declaration(declaration)?;
                (metadata.kind == DeclarationKind::Function).then(|| (declaration, name.to_owned()))
            })
            .collect()
    }

    fn type_symbols(&self) -> BTreeMap<HirDeclId, String> {
        let Some(declarations) = self.graph.module(self.module) else {
            return BTreeMap::new();
        };
        declarations
            .names()
            .filter_map(|name| {
                let declaration = declarations.get(name)?;
                let metadata = self.graph.declaration(declaration)?;
                matches!(
                    metadata.kind,
                    DeclarationKind::Struct | DeclarationKind::Enum
                )
                .then(|| (declaration, name.to_owned()))
            })
            .collect()
    }

    fn const_values(&self) -> CompileResult<BTreeMap<HirDeclId, Constant>> {
        let mut values_by_declaration = BTreeMap::new();
        let mut values_by_name = BTreeMap::new();
        for item in &self.parsed.items {
            let ItemKind::Const(item) = &item.kind else {
                continue;
            };
            let Some(declaration) = self
                .graph
                .module(self.module)
                .and_then(|m| m.get(&item.name))
            else {
                continue;
            };
            if let Some(value) = evaluate_const_expr(&item.value, &values_by_name)? {
                values_by_declaration.insert(declaration, value.clone());
                values_by_name.insert(item.name.clone(), value);
            }
        }
        Ok(values_by_declaration)
    }

    fn function_declaration(&self, name: &str) -> Option<HirDeclId> {
        let declaration = self.graph.module(self.module)?.get(name)?;
        let metadata = self.graph.declaration(declaration)?;
        (metadata.kind == DeclarationKind::Function).then_some(declaration)
    }
}

impl SemanticModules {
    fn function(
        &self,
        declaration: HirDeclId,
    ) -> Option<(&FunctionItem, &FunctionSignature, &BindingMap)> {
        let metadata = self.graph.declaration(declaration)?;
        let signature = self.graph.function_signature(declaration)?;
        let bindings = self.graph.bindings(declaration)?;
        let parsed = self.parsed.get(&metadata.module)?;
        let function = parsed.items.iter().find_map(|item| match &item.kind {
            ItemKind::Function(function) if function.name == metadata.name => Some(function),
            _ => None,
        })?;
        Some((function, signature, bindings))
    }

    fn script_function_declarations(&self) -> BTreeSet<HirDeclId> {
        self.modules
            .iter()
            .filter_map(|module| self.graph.module(*module))
            .flat_map(|declarations| {
                declarations.names().filter_map(|name| {
                    let declaration = declarations.get(name)?;
                    let metadata = self.graph.declaration(declaration)?;
                    (metadata.kind == DeclarationKind::Function).then_some(declaration)
                })
            })
            .collect()
    }

    fn script_function_symbols(&self) -> BTreeMap<HirDeclId, String> {
        self.modules
            .iter()
            .filter_map(|module| {
                let path = self.graph.module_path(*module)?.join();
                let declarations = self.graph.module(*module)?;
                Some((path, declarations))
            })
            .flat_map(|(path, declarations)| {
                declarations.names().filter_map(move |name| {
                    let declaration = declarations.get(name)?;
                    let metadata = self.graph.declaration(declaration)?;
                    (metadata.kind == DeclarationKind::Function)
                        .then(|| (declaration, format!("{path}.{}", metadata.name)))
                })
            })
            .collect()
    }

    fn type_symbols(&self) -> BTreeMap<HirDeclId, String> {
        self.modules
            .iter()
            .filter_map(|module| {
                let path = self.graph.module_path(*module)?.join();
                let declarations = self.graph.module(*module)?;
                Some((path, declarations))
            })
            .flat_map(|(path, declarations)| {
                declarations.names().filter_map(move |name| {
                    let declaration = declarations.get(name)?;
                    let metadata = self.graph.declaration(declaration)?;
                    matches!(
                        metadata.kind,
                        DeclarationKind::Struct | DeclarationKind::Enum
                    )
                    .then(|| (declaration, format!("{path}.{}", metadata.name)))
                })
            })
            .collect()
    }

    fn const_values(&self) -> CompileResult<BTreeMap<HirDeclId, Constant>> {
        let mut values_by_declaration = BTreeMap::new();
        loop {
            let mut progressed = false;
            for module in &self.modules {
                let mut previous_values = BTreeMap::new();
                let Some(parsed) = self.parsed.get(module) else {
                    continue;
                };
                for item in &parsed.items {
                    let ItemKind::Const(item) = &item.kind else {
                        continue;
                    };
                    let Some(declaration) =
                        self.graph.module(*module).and_then(|m| m.get(&item.name))
                    else {
                        continue;
                    };
                    if let Some(value) = values_by_declaration.get(&declaration).cloned() {
                        previous_values.insert(item.name.clone(), value);
                        continue;
                    }

                    let mut values_by_name =
                        self.imported_const_values(*module, &values_by_declaration);
                    values_by_name.extend(previous_values.clone());
                    if let Some(value) = evaluate_const_expr(&item.value, &values_by_name)? {
                        values_by_declaration.insert(declaration, value.clone());
                        previous_values.insert(item.name.clone(), value);
                        progressed = true;
                    }
                }
            }
            if !progressed {
                break;
            }
        }
        Ok(values_by_declaration)
    }

    fn imported_const_values(
        &self,
        module: ModuleId,
        values_by_declaration: &BTreeMap<HirDeclId, Constant>,
    ) -> BTreeMap<String, Constant> {
        let mut values = BTreeMap::new();
        let Some(imports) = self.graph.imports(module) else {
            return values;
        };
        for import in imports {
            let Some(ImportResolution::Declaration(declaration)) = import.resolution else {
                continue;
            };
            let Some(metadata) = self.graph.declaration(declaration) else {
                continue;
            };
            if metadata.kind != DeclarationKind::Const {
                continue;
            }
            let Some(value) = values_by_declaration.get(&declaration).cloned() else {
                continue;
            };
            let Some(name) = import.alias.clone().or_else(|| import.path.last().cloned()) else {
                continue;
            };
            values.insert(name, value);
        }
        values
    }
}

fn parse_semantic_source(source: SourceId, text: &str) -> CompileResult<SemanticSource> {
    let parsed = parse_checked_source(source, text)?;
    let mut graph = ModuleGraph::new();
    let module = graph.add_parsed_source(source, ModulePath::from_dotted("main"), parsed.clone());
    graph.resolve_imports();
    if graph.diagnostics().is_empty() {
        Ok(SemanticSource {
            parsed,
            graph,
            module,
        })
    } else {
        Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
            graph.diagnostics().to_vec(),
        )))
    }
}

fn parse_semantic_modules(sources: &[ModuleSource]) -> CompileResult<SemanticModules> {
    let mut parsed = BTreeMap::new();
    let mut graph = ModuleGraph::new();
    let mut modules = Vec::new();
    let mut syntax_diagnostics = Vec::new();

    for source in sources {
        let source_file = parse_source(source.id, &source.text);
        if !source_file.diagnostics.is_empty() {
            syntax_diagnostics.extend(source_file.diagnostics.clone());
        }
        let module = graph.add_parsed_source(source.id, source.path.clone(), source_file.clone());
        parsed.insert(module, source_file);
        modules.push(module);
    }

    if !syntax_diagnostics.is_empty() {
        return Err(CompileError::new(CompileErrorKind::SyntaxDiagnostics(
            syntax_diagnostics,
        )));
    }

    graph.resolve_imports();
    if graph.diagnostics().is_empty() {
        Ok(SemanticModules {
            parsed,
            graph,
            modules,
        })
    } else {
        Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
            graph.diagnostics().to_vec(),
        )))
    }
}

struct Compiler<'ast> {
    code: CodeObject,
    locals: HashMap<String, Register>,
    hir_locals: HashMap<HirLocalId, Register>,
    bindings: &'ast BindingMap,
    next_register: u16,
    body: &'ast Block,
    facts: CompilerFacts,
}

impl<'ast> Compiler<'ast> {
    fn new(
        code_name: String,
        function: &'ast FunctionItem,
        signature: &FunctionSignature,
        bindings: &'ast BindingMap,
        facts: CompilerFacts,
    ) -> CompileResult<Self> {
        let param_count = u16::try_from(signature.params.len())
            .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?;
        let param_names = signature
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<_>>();
        let mut locals = HashMap::new();
        let mut hir_locals = HashMap::new();
        let parameter_locals = bindings
            .locals()
            .filter(|local| local.kind == LocalBindingKind::Parameter)
            .map(|local| local.id)
            .collect::<Vec<_>>();
        for (index, param) in param_names.iter().enumerate() {
            let register = u16::try_from(index)
                .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?;
            locals.insert(param.clone(), Register(register));
            if let Some(local) = parameter_locals.get(index).copied() {
                hir_locals.insert(local, Register(register));
            }
        }

        Ok(Self {
            code: CodeObject::new(code_name, 0).with_params(param_names),
            locals,
            hir_locals,
            bindings,
            next_register: param_count,
            body: &function.body,
            facts,
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
            StmtKind::Let { name, value, .. } => {
                let register = if let Some(value) = value {
                    self.compile_expr(value)?
                } else {
                    self.emit_constant(Constant::Null)?
                };
                self.locals.insert(name.clone(), register);
                if let Some(local) =
                    self.bindings
                        .local_named_at(name, LocalBindingKind::Let, stmt.span)
                {
                    self.hir_locals.insert(local, register);
                }
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
                if let ExprKind::Match(match_expr) = &expr.kind {
                    return self.compile_match(match_expr);
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
            ExprKind::Path(path) => self.compile_path_expr(expr.span, path),
            ExprKind::Binary { op, left, right } => self.compile_binary(*op, left, right),
            ExprKind::Unary { op, expr } => self.compile_unary(*op, expr),
            ExprKind::Field { base, name } => {
                let root = self.compile_expr(base)?;
                let dst = self.alloc_register()?;
                if let Some(field) = self.facts.options.host_fields.get(name).copied() {
                    self.emit(InstructionKind::GetHostField { dst, root, field });
                } else {
                    self.emit(InstructionKind::GetRecordField {
                        dst,
                        record: root,
                        field: name.clone(),
                    });
                }
                Ok(dst)
            }
            ExprKind::Call { callee, args } => {
                let fallback_name = callable_name(callee)?;
                let arg_registers = args
                    .iter()
                    .map(|arg| self.compile_expr(&arg.value))
                    .collect::<CompileResult<Vec<_>>>()?;
                let dst = self.alloc_register()?;
                if let Some(name) = self.script_function_call_name(callee) {
                    self.emit(InstructionKind::CallFunction {
                        dst,
                        name,
                        args: arg_registers,
                    });
                } else {
                    self.emit(InstructionKind::CallNative {
                        dst: Some(dst),
                        name: fallback_name,
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
            ExprKind::Record { path, fields } => {
                let fields = fields
                    .iter()
                    .map(|field| self.compile_record_field(field))
                    .collect::<CompileResult<Vec<_>>>()?;
                let dst = self.alloc_register()?;
                if let Some((enum_name, variant)) = enum_variant_path(path) {
                    let enum_name = self.type_symbol_at_span(expr.span).unwrap_or(enum_name);
                    self.emit(InstructionKind::MakeEnum {
                        dst,
                        enum_name,
                        variant,
                        fields,
                    });
                } else {
                    let type_name = self
                        .type_symbol_at_span(expr.span)
                        .unwrap_or_else(|| path.join("."));
                    self.emit(InstructionKind::MakeRecord {
                        dst,
                        type_name,
                        fields,
                    });
                }
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
            | ExprKind::Index { .. }
            | ExprKind::Try(_)
            | ExprKind::Lambda { .. }
            | ExprKind::Error => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "expression",
            ))),
            ExprKind::Match(_) => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "match expression",
            ))),
        }
    }

    fn script_function_call_name(&self, callee: &Expr) -> Option<String> {
        let Some(BindingResolution::Declaration(declaration)) =
            self.bindings.resolution_at_span(callee.span)
        else {
            return None;
        };
        self.facts.script_function_symbols.get(declaration).cloned()
    }

    fn type_symbol_at_span(&self, span: Span) -> Option<String> {
        let Some(BindingResolution::Declaration(declaration)) =
            self.bindings.resolution_at_span(span)
        else {
            return None;
        };
        self.facts.type_symbols.get(declaration).cloned()
    }

    fn type_symbol_for_pattern(&self, path: &[String]) -> Option<String> {
        let Some(BindingResolution::Declaration(declaration)) =
            self.bindings.pattern_resolution(path)
        else {
            return None;
        };
        self.facts.type_symbols.get(declaration).cloned()
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

    fn compile_local_path(&mut self, span: Span, path: &[String]) -> CompileResult<Register> {
        let [name] = path else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "path expression",
            )));
        };
        self.local_register_at_span(span, name)
    }

    fn compile_path_expr(&mut self, span: Span, path: &[String]) -> CompileResult<Register> {
        if let Some(value) = self.const_value_at_span(span) {
            return self.emit_constant(value);
        }
        if path.len() == 1 {
            return self.compile_local_path(span, path);
        }
        self.compile_path_access(span, path)
    }

    fn local_register_at_span(&mut self, span: Span, name: &str) -> CompileResult<Register> {
        if let Some(BindingResolution::Local(local)) = self.bindings.resolution_at_span(span)
            && let Some(register) = self.hir_locals.get(local).copied()
        {
            return Ok(register);
        }
        if let Some(value) = self.const_value_at_span(span) {
            return self.emit_constant(value);
        }
        self.locals
            .get(name)
            .copied()
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnknownLocal(name.to_owned())))
    }

    fn const_value_at_span(&self, span: Span) -> Option<Constant> {
        let BindingResolution::Declaration(declaration) = self.bindings.resolution_at_span(span)?
        else {
            return None;
        };
        self.facts.const_values.get(declaration).cloned()
    }

    fn compile_path_access(&mut self, span: Span, path: &[String]) -> CompileResult<Register> {
        if path.len() != 2 {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "path expression",
            )));
        }
        let root = self.local_register_at_span(span, &path[0])?;
        let dst = self.alloc_register()?;
        if let Some(field) = self.facts.options.host_fields.get(&path[1]).copied() {
            self.emit(InstructionKind::GetHostField { dst, root, field });
        } else {
            self.emit(InstructionKind::GetRecordField {
                dst,
                record: root,
                field: path[1].clone(),
            });
        }
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
            ExprKind::Path(path) => self.compile_host_path_parts(target.span, path),
            _ => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "assignment target",
            ))),
        }
    }

    fn compile_host_path_parts(
        &mut self,
        span: Span,
        path: &[String],
    ) -> CompileResult<(Register, FieldId)> {
        if path.len() != 2 {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "host path",
            )));
        }
        let root = self.local_register_at_span(span, &path[0])?;
        let field = self.host_field(&path[1])?;
        Ok((root, field))
    }

    fn compile_literal(&mut self, literal: &Literal) -> CompileResult<Register> {
        self.emit_constant(compile_literal_constant(literal)?)
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

    fn compile_unary(&mut self, op: UnaryOp, expr: &Expr) -> CompileResult<Register> {
        let src = self.compile_expr(expr)?;
        let dst = self.alloc_register()?;
        let instruction = match op {
            UnaryOp::Not => InstructionKind::Not { dst, src },
            UnaryOp::Negate => InstructionKind::Negate { dst, src },
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

    fn compile_match(&mut self, match_expr: &MatchExpr) -> CompileResult<bool> {
        let scrutinee = self.compile_expr(&match_expr.scrutinee)?;
        let mut end_jumps = Vec::new();
        let mut all_arms_return = !match_expr.arms.is_empty();

        for arm in &match_expr.arms {
            if arm.guard.is_some() {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "match guard",
                )));
            }
            let next_arm_jump = self.compile_match_pattern(scrutinee, &arm.pattern)?;
            let previous_locals = self.locals.clone();
            let previous_hir_locals = self.hir_locals.clone();
            self.bind_match_fields(scrutinee, &arm.pattern, arm.body.span)?;
            let arm_returned = match &arm.body.kind {
                ExprKind::Block(block) => self.compile_statements(&block.statements)?,
                _ => {
                    self.compile_expr(&arm.body)?;
                    false
                }
            };
            self.locals = previous_locals;
            self.hir_locals = previous_hir_locals;
            all_arms_return &= arm_returned;
            if !arm_returned {
                end_jumps.push(self.emit_jump());
            }
            if let Some(next_arm_jump) = next_arm_jump {
                self.patch_jump(next_arm_jump, self.current_offset())?;
            } else {
                break;
            }
        }

        for jump in end_jumps {
            self.patch_jump(jump, self.current_offset())?;
        }

        Ok(all_arms_return)
    }

    fn compile_match_pattern(
        &mut self,
        scrutinee: Register,
        pattern: &Pattern,
    ) -> CompileResult<Option<usize>> {
        match pattern {
            Pattern::Wildcard => Ok(None),
            Pattern::Path(path) | Pattern::RecordVariant { path, .. } => {
                let Some((enum_name, variant)) = enum_variant_path(path) else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "match pattern",
                    )));
                };
                let enum_name = self.type_symbol_for_pattern(path).unwrap_or(enum_name);
                let condition = self.alloc_register()?;
                self.emit(InstructionKind::EnumTagEqual {
                    dst: condition,
                    value: scrutinee,
                    enum_name,
                    variant,
                });
                Ok(Some(self.emit_jump_if_false(condition)))
            }
            Pattern::Literal(_) | Pattern::Binding(_) | Pattern::TupleVariant { .. } => Err(
                CompileError::new(CompileErrorKind::UnsupportedSyntax("match pattern")),
            ),
        }
    }

    fn bind_match_fields(
        &mut self,
        scrutinee: Register,
        pattern: &Pattern,
        body_span: Span,
    ) -> CompileResult<()> {
        let Pattern::RecordVariant { fields, .. } = pattern else {
            return Ok(());
        };
        for field in fields {
            let binding = record_pattern_binding(field)?;
            let dst = self.alloc_register()?;
            self.emit(InstructionKind::GetEnumField {
                dst,
                value: scrutinee,
                field: field.name.clone(),
            });
            self.locals.insert(binding.clone(), dst);
            if let Some(local) =
                self.bindings
                    .local_named_at(&binding, LocalBindingKind::Pattern, body_span)
            {
                self.hir_locals.insert(local, dst);
            }
        }
        Ok(())
    }

    fn compile_map_entry(&mut self, entry: &MapEntry) -> CompileResult<(String, Register)> {
        let key = map_key_name(&entry.key)?;
        let value = self.compile_expr(&entry.value)?;
        Ok((key, value))
    }

    fn compile_record_field(
        &mut self,
        field: &vela_syntax::RecordField,
    ) -> CompileResult<(String, Register)> {
        let value = if let Some(value) = &field.value {
            self.compile_expr(value)?
        } else {
            self.local_register_at_span(field.span, &field.name)?
        };
        Ok((field.name.clone(), value))
    }

    fn host_field(&self, name: &str) -> CompileResult<FieldId> {
        self.facts
            .options
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

fn enum_variant_path(path: &[String]) -> Option<(String, String)> {
    let (variant, enum_path) = path.split_last()?;
    if enum_path.is_empty() {
        return None;
    }
    Some((enum_path.join("."), variant.clone()))
}

fn record_pattern_binding(field: &RecordPatternField) -> CompileResult<String> {
    match &field.pattern {
        None => Ok(field.name.clone()),
        Some(Pattern::Binding(name)) => Ok(name.clone()),
        Some(_) => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "record pattern",
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

fn compile_literal_constant(literal: &Literal) -> CompileResult<Constant> {
    Ok(match literal {
        Literal::Null => Constant::Null,
        Literal::Bool(value) => Constant::Bool(*value),
        Literal::Int(value) => Constant::Int(parse_int(value)?),
        Literal::Float(value) => Constant::Float(parse_float(value)?),
        Literal::String(value) => Constant::String(value.clone()),
    })
}

fn evaluate_const_expr(
    expr: &Expr,
    values_by_name: &BTreeMap<String, Constant>,
) -> CompileResult<Option<Constant>> {
    match &expr.kind {
        ExprKind::Literal(literal) => compile_literal_constant(literal).map(Some),
        ExprKind::Path(path) => {
            let [name] = path.as_slice() else {
                return Ok(None);
            };
            Ok(values_by_name.get(name).cloned())
        }
        ExprKind::Unary { op, expr } => {
            let Some(value) = evaluate_const_expr(expr, values_by_name)? else {
                return Ok(None);
            };
            Ok(evaluate_unary_const(*op, value))
        }
        ExprKind::Binary { op, left, right } => {
            let Some(left) = evaluate_const_expr(left, values_by_name)? else {
                return Ok(None);
            };
            let Some(right) = evaluate_const_expr(right, values_by_name)? else {
                return Ok(None);
            };
            Ok(evaluate_binary_const(*op, left, right))
        }
        ExprKind::Block(_)
        | ExprKind::If(_)
        | ExprKind::Match(_)
        | ExprKind::SelfValue
        | ExprKind::Assign { .. }
        | ExprKind::Field { .. }
        | ExprKind::Call { .. }
        | ExprKind::Index { .. }
        | ExprKind::Try(_)
        | ExprKind::Array(_)
        | ExprKind::Map(_)
        | ExprKind::Record { .. }
        | ExprKind::Lambda { .. }
        | ExprKind::Error => Ok(None),
    }
}

fn evaluate_unary_const(op: UnaryOp, value: Constant) -> Option<Constant> {
    match (op, value) {
        (UnaryOp::Negate, Constant::Int(value)) => value.checked_neg().map(Constant::Int),
        (UnaryOp::Negate, Constant::Float(value)) => Some(Constant::Float(-value)),
        (UnaryOp::Not, Constant::Bool(value)) => Some(Constant::Bool(!value)),
        _ => None,
    }
}

fn evaluate_binary_const(op: BinaryOp, left: Constant, right: Constant) -> Option<Constant> {
    match op {
        BinaryOp::Add => evaluate_numeric_const(left, right, i64::checked_add, |a, b| a + b),
        BinaryOp::Sub => evaluate_numeric_const(left, right, i64::checked_sub, |a, b| a - b),
        BinaryOp::Mul => evaluate_numeric_const(left, right, i64::checked_mul, |a, b| a * b),
        BinaryOp::Div => match (left, right) {
            (Constant::Int(_), Constant::Int(0)) => None,
            (Constant::Int(left), Constant::Int(right)) => {
                left.checked_div(right).map(Constant::Int)
            }
            (Constant::Float(_), Constant::Float(0.0)) => None,
            (Constant::Float(left), Constant::Float(right)) => Some(Constant::Float(left / right)),
            _ => None,
        },
        BinaryOp::Rem => match (left, right) {
            (Constant::Int(_), Constant::Int(0)) => None,
            (Constant::Int(left), Constant::Int(right)) => {
                left.checked_rem(right).map(Constant::Int)
            }
            (Constant::Float(_), Constant::Float(0.0)) => None,
            (Constant::Float(left), Constant::Float(right)) => Some(Constant::Float(left % right)),
            _ => None,
        },
        BinaryOp::Equal => Some(Constant::Bool(left == right)),
        BinaryOp::NotEqual => Some(Constant::Bool(left != right)),
        BinaryOp::Less => evaluate_numeric_compare_const(left, right, |a, b| a < b),
        BinaryOp::LessEqual => evaluate_numeric_compare_const(left, right, |a, b| a <= b),
        BinaryOp::Greater => evaluate_numeric_compare_const(left, right, |a, b| a > b),
        BinaryOp::GreaterEqual => evaluate_numeric_compare_const(left, right, |a, b| a >= b),
        BinaryOp::Or | BinaryOp::And => None,
    }
}

fn evaluate_numeric_const(
    left: Constant,
    right: Constant,
    int_op: impl FnOnce(i64, i64) -> Option<i64>,
    float_op: impl FnOnce(f64, f64) -> f64,
) -> Option<Constant> {
    match (left, right) {
        (Constant::Int(left), Constant::Int(right)) => int_op(left, right).map(Constant::Int),
        (Constant::Float(left), Constant::Float(right)) => {
            Some(Constant::Float(float_op(left, right)))
        }
        _ => None,
    }
}

fn evaluate_numeric_compare_const(
    left: Constant,
    right: Constant,
    op: impl FnOnce(f64, f64) -> bool,
) -> Option<Constant> {
    match (left, right) {
        (Constant::Int(left), Constant::Int(right)) => {
            Some(Constant::Bool(op(left as f64, right as f64)))
        }
        (Constant::Float(left), Constant::Float(right)) => Some(Constant::Bool(op(left, right))),
        _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_rejects_duplicate_declarations_from_hir() {
        let error = compile_program_source(
            SourceId::new(1),
            r#"
fn main() { return 1; }
fn main() { return 2; }
"#,
        )
        .expect_err("duplicate function should fail before bytecode generation");

        let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
            panic!("expected semantic diagnostics");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_declaration"))
        );
    }

    #[test]
    fn compiler_rejects_unresolved_names_from_hir_with_candidates() {
        let error = compile_function_source(
            SourceId::new(1),
            r#"
fn main(player) {
    return plaeyr;
}
"#,
            "main",
        )
        .expect_err("unresolved name should fail before bytecode generation");

        let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
            panic!("expected semantic diagnostics");
        };
        let unresolved = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::unresolved_name"))
            .expect("unresolved name diagnostic");

        assert_eq!(unresolved.labels.len(), 1);
        assert!(unresolved.labels[0].message.contains("player"));
    }

    #[test]
    fn compiler_rejects_private_imports_before_codegen() {
        let error = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.main"),
                r#"
use game.reward.secret

fn main() {
    return secret();
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.reward"),
                r#"
fn secret() {
    return 1;
}
"#,
            ),
        ])
        .expect_err("private import should fail before bytecode generation");

        let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
            panic!("expected semantic diagnostics");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::private_import"))
        );
    }

    #[test]
    fn compiler_keeps_valid_program_bytecode_equivalent_after_hir_gate() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
const BONUS: int = 5;

trait BonusSource { fn bonus(self) -> int; }
struct Player { level: int }
impl BonusSource for Player {
    fn bonus(self) -> int { return self.level; }
}

fn add_bonus(value) {
    return value + 5;
}

fn main() {
    let base = 10;
    return add_bonus(base) * 2;
}
"#,
        )
        .expect("valid source should compile through HIR gate");
        let main = program.function("main").expect("main function");

        assert_eq!(main.params, Vec::<String>::new());
        assert!(program.function("bonus").is_none());
        assert!(!main.instructions.is_empty());
        assert!(
            main.instructions.iter().any(|instruction| matches!(
                instruction.kind,
                InstructionKind::CallFunction { .. }
            ))
        );
    }

    #[test]
    fn compiler_uses_hir_signatures_for_code_object_params() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main(player: game.Player, amount: int) -> int {
    return amount;
}
"#,
            "main",
        )
        .expect("typed params should compile through HIR signature metadata");

        assert_eq!(code.params, ["player", "amount"]);
    }

    #[test]
    fn compiler_uses_hir_declarations_for_literal_const_reads() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
const BONUS: int = 5;

fn main() {
    return BONUS;
}
"#,
            "main",
        )
        .expect("literal const reads should compile through HIR declaration facts");

        let returned = code
            .instructions
            .iter()
            .find_map(|instruction| match instruction.kind {
                InstructionKind::Return { src } => Some(src),
                _ => None,
            })
            .expect("return instruction");
        let constant = code.instructions.iter().find_map(|instruction| {
            let InstructionKind::LoadConst { dst, constant } = instruction.kind else {
                return None;
            };
            (dst == returned).then_some(constant)
        });

        assert_eq!(
            constant.map(|constant| &code.constants[constant.0]),
            Some(&Constant::Int(5))
        );
    }

    #[test]
    fn compiler_evaluates_pure_scalar_const_expressions() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
const BASE: int = 10;
const BONUS: int = BASE + 5 * 2;

fn main() {
    return BONUS;
}
"#,
            "main",
        )
        .expect("pure scalar const expressions should compile");

        let returned = code
            .instructions
            .iter()
            .find_map(|instruction| match instruction.kind {
                InstructionKind::Return { src } => Some(src),
                _ => None,
            })
            .expect("return instruction");
        let constant = code.instructions.iter().find_map(|instruction| {
            let InstructionKind::LoadConst { dst, constant } = instruction.kind else {
                return None;
            };
            (dst == returned).then_some(constant)
        });

        assert_eq!(
            constant.map(|constant| &code.constants[constant.0]),
            Some(&Constant::Int(20))
        );
    }

    #[test]
    fn compiler_evaluates_imported_scalar_const_expressions_across_modules() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.main"),
                r#"
use game.tuning.BONUS as REWARD

fn main() {
    return REWARD + 1;
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.tuning"),
                r#"
use game.base.BASE as START

pub const BONUS: int = START + 1;
"#,
            ),
            ModuleSource::new(
                SourceId::new(3),
                ModulePath::from_dotted("game.base"),
                r#"
pub const BASE: int = 4;
"#,
            ),
        ])
        .expect("imported scalar const expressions should compile across modules");
        let main = program
            .function("game.main.main")
            .expect("qualified main function");

        assert!(main.constants.contains(&Constant::Int(5)));
    }

    #[test]
    fn compiler_uses_hir_local_bindings_for_shadowed_registers() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = 1;
    {
        let value = 2;
    }
    return value;
}
"#,
            "main",
        )
        .expect("shadowed locals should compile through HIR bindings");

        let returned = code
            .instructions
            .iter()
            .find_map(|instruction| match instruction.kind {
                InstructionKind::Return { src } => Some(src),
                _ => None,
            })
            .expect("return instruction");

        assert_eq!(returned, Register(0));
    }

    #[test]
    fn compiler_uses_hir_bindings_for_record_shorthand_fields() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = 1;
    {
        let value = 2;
    }
    return Reward { value };
}
"#,
            "main",
        )
        .expect("record shorthand should compile through HIR bindings");

        let value_register = code
            .instructions
            .iter()
            .find_map(|instruction| match &instruction.kind {
                InstructionKind::MakeRecord { fields, .. } => fields
                    .iter()
                    .find_map(|(name, register)| (name == "value").then_some(*register)),
                _ => None,
            })
            .expect("record shorthand field register");

        assert_eq!(value_register, Register(0));
    }

    #[test]
    fn compiler_uses_hir_bindings_for_match_pattern_fields() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main(reward) {
    let amount = 100;
    match reward {
        Reward.Granted { amount } => {
            {
                let amount = 2;
            }
            return amount;
        }
        _ => {
            return 0;
        }
    }
}
"#,
            "main",
        )
        .expect("match pattern bindings should compile through HIR bindings");

        let pattern_register = code
            .instructions
            .iter()
            .find_map(|instruction| match instruction.kind {
                InstructionKind::GetEnumField { dst, ref field, .. } if field == "amount" => {
                    Some(dst)
                }
                _ => None,
            })
            .expect("pattern field register");

        assert!(code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::Return { src } if src == pattern_register
        )));
    }

    #[test]
    fn compiler_uses_hir_callee_resolution_for_shadowed_function_names() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn helper() {
    return 1;
}

fn main() {
    let helper = 2;
    return helper();
}
"#,
            "main",
        )
        .expect("shadowed callee name should compile through HIR binding facts");

        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::CallNative { name, .. } if name == "helper"
        )));
        assert!(!code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::CallFunction { name, .. } if name == "helper"
        )));
    }

    #[test]
    fn compiler_emits_script_calls_for_imported_aliases_across_modules() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.main"),
                r#"
use game.reward.grant as give_reward

fn main() {
    return give_reward(4);
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.reward"),
                r#"
pub fn grant(amount) {
    return amount + 1;
}
"#,
            ),
        ])
        .expect("cross-module imported script function should compile");

        let main = program
            .function("game.main.main")
            .expect("qualified main function");
        assert!(program.function("game.reward.grant").is_some());
        assert!(main.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::CallFunction { name, .. } if name == "game.reward.grant"
        )));
        assert!(!main.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::CallNative { name, .. } if name == "give_reward"
        )));
    }

    #[test]
    fn compiler_keeps_same_named_functions_in_separate_modules() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.main"),
                r#"
use game.reward.main as reward_main

fn main() {
    return reward_main();
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.reward"),
                r#"
pub fn main() {
    return 7;
}
"#,
            ),
        ])
        .expect("same-named cross-module functions should compile");

        assert!(program.function("game.main.main").is_some());
        assert!(program.function("game.reward.main").is_some());
        let main = program
            .function("game.main.main")
            .expect("qualified main function");
        assert!(main.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::CallFunction { name, .. } if name == "game.reward.main"
        )));
    }

    #[test]
    fn compiler_uses_hir_type_symbols_for_imported_constructors() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.main"),
                r#"
use game.reward.Reward as Prize
use game.damage.Damage as Hit

fn make_reward() {
    return Prize { count: 2 };
}

fn make_damage() {
    return Hit.Physical { amount: 7 };
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.reward"),
                r#"
pub struct Reward { count: int }
"#,
            ),
            ModuleSource::new(
                SourceId::new(3),
                ModulePath::from_dotted("game.damage"),
                r#"
pub enum Damage { Physical }
"#,
            ),
        ])
        .expect("imported constructors should compile through HIR type symbols");
        let reward = program
            .function("game.main.make_reward")
            .expect("qualified reward function");
        let damage = program
            .function("game.main.make_damage")
            .expect("qualified damage function");

        assert!(reward.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::MakeRecord { type_name, .. } if type_name == "game.reward.Reward"
        )));
        assert!(damage.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::MakeEnum { enum_name, variant, .. }
                if enum_name == "game.damage.Damage" && variant == "Physical"
        )));
    }

    #[test]
    fn compiler_uses_hir_type_symbols_for_imported_match_patterns() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.main"),
                r#"
use game.damage.Damage as Hit

fn main() {
    let damage = Hit.Physical { amount: 7 };
    match damage {
        Hit.Physical { amount } => { return amount; },
        _ => { return 0; },
    }
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.damage"),
                r#"
pub enum Damage { Physical }
"#,
            ),
        ])
        .expect("imported match patterns should compile through HIR type symbols");
        let main = program
            .function("game.main.main")
            .expect("qualified main function");

        assert!(main.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::EnumTagEqual { enum_name, variant, .. }
                if enum_name == "game.damage.Damage" && variant == "Physical"
        )));
    }

    #[test]
    fn compiler_uses_hir_facts_for_qualified_function_and_const_paths() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.main"),
                r#"
fn main() {
    return game.reward.grant() + game.config.BONUS;
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.reward"),
                r#"
pub fn grant() {
    return 4;
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(3),
                ModulePath::from_dotted("game.config"),
                r#"
pub const BONUS: int = 5;
"#,
            ),
        ])
        .expect("qualified function and const paths should compile");
        let main = program
            .function("game.main.main")
            .expect("qualified main function");

        assert!(main.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::CallFunction { name, .. } if name == "game.reward.grant"
        )));
        assert!(main.constants.contains(&Constant::Int(5)));
    }

    #[test]
    fn compiler_lowers_unary_operators() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    return !false == true && -5 < 0;
}
"#,
            "main",
        )
        .expect_err("logical operators are still unsupported");

        let CompileErrorKind::UnsupportedSyntax("binary operator") = code.kind else {
            panic!("expected logical operator to remain unsupported");
        };

        let code = compile_function_source(
            SourceId::new(2),
            r#"
fn main() {
    if !false {
        return -5;
    }
    return 0;
}
"#,
            "main",
        )
        .expect("unary operators should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| { matches!(instruction.kind, InstructionKind::Not { .. }) })
        );
        assert!(
            code.instructions
                .iter()
                .any(|instruction| { matches!(instruction.kind, InstructionKind::Negate { .. }) })
        );
    }

    #[test]
    fn compiler_rejects_top_level_mutation_as_syntax_before_codegen() {
        let error = compile_program_source(
            SourceId::new(1),
            r#"
player.level = 10;
fn main(player) { return player.level; }
"#,
        )
        .expect_err("top-level mutation should not reach bytecode generation");

        let CompileErrorKind::SyntaxDiagnostics(diagnostics) = error.kind else {
            panic!("expected syntax diagnostics");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("expected item"))
        );
    }

    #[test]
    fn compiler_rejects_top_level_const_side_effects_from_hir() {
        let error = compile_program_source(
            SourceId::new(1),
            r#"
const BAD = register_event("monster.kill");
fn main() { return 1; }
"#,
        )
        .expect_err("side-effecting const initializer should fail before bytecode generation");

        let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
            panic!("expected semantic diagnostics");
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_deref() == Some("hir::top_level_side_effect")
        }));
    }

    #[test]
    fn compiler_rejects_generic_type_hints_before_codegen() {
        let error = compile_program_source(
            SourceId::new(1),
            r#"
fn main(values: Array<int>) {
    return values;
}
"#,
        )
        .expect_err("generic type hints should fail in syntax validation");

        let CompileErrorKind::SyntaxDiagnostics(diagnostics) = error.kind else {
            panic!("expected syntax diagnostics");
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code.as_deref() == Some("syntax::generic_type_hint")
            })
        );
    }
}
