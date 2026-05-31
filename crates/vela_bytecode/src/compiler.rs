//! Minimal AST-to-bytecode compiler for the M2 VM loop.

mod assignments;
mod call_args;
mod calls;
mod const_eval;
mod constructors;
mod control_flow;
mod error;
mod expressions;
mod field_slots;
mod host_paths;
mod lambdas;
mod map_literals;
mod methods;
mod operators;
mod options;
mod paths;
mod patterns;
mod schema_defaults;
mod script_impls;
mod script_types;
mod value_flow;

use std::collections::{BTreeMap, BTreeSet, HashMap};

#[cfg(test)]
use vela_common::{FieldId, HostMethodId};
use vela_common::{MethodId, SourceId, Span};
use vela_hir::{
    BindingMap, BindingResolution, DeclarationKind, FunctionSignature, HirDeclId, HirLocalId,
    HirTypeHint, ImportResolution, LocalBindingKind, ModuleGraph, ModuleId, ModulePath,
    ModuleSource, ParamHint,
};
use vela_syntax::{
    Argument, Block, Expr, ExprKind, FunctionItem, ItemKind, Param, SourceFile, parse_source,
};

#[cfg(test)]
use crate::HostPathSegment;
use crate::{
    CodeObject, Constant, Instruction, InstructionKind, InstructionOffset, Program, Register,
};
use const_eval::evaluate_const_expr;
use control_flow::LoopContext;
pub use error::{CompileError, CompileErrorKind, CompileResult};
use field_slots::ScriptFieldSlots;
use lambdas::LambdaCapture;
pub use options::CompilerOptions;
use patterns::enum_variant_path;
use schema_defaults::{ScriptSchemaDefaults, source_schema_defaults};
use script_types::{
    ScriptTypeFact, ScriptTypeFlow, expression_script_fact, expression_script_type,
    type_hint_script_type,
};

#[derive(Clone, Debug)]
struct CompilerFacts {
    script_function_symbols: BTreeMap<HirDeclId, String>,
    script_function_signatures: BTreeMap<HirDeclId, Vec<ParamHint>>,
    script_method_ids: BTreeMap<(String, String), MethodId>,
    script_method_signatures: BTreeMap<(String, String), Vec<ParamHint>>,
    script_field_slots: ScriptFieldSlots,
    schema_defaults: ScriptSchemaDefaults,
    type_symbols: BTreeMap<HirDeclId, String>,
    const_values: BTreeMap<HirDeclId, Constant>,
    options: CompilerOptions,
}

impl CompilerFacts {
    fn known_type_names(&self) -> Vec<String> {
        self.type_symbols
            .values()
            .cloned()
            .chain(self.options.host_types.iter().cloned())
            .collect()
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
    let semantic = parse_semantic_source(source, text)?;
    let script_function_symbols = semantic.script_function_symbols();
    let script_function_signatures = semantic.script_function_signatures();
    let type_symbols = semantic.type_symbols();
    let script_field_slots = semantic.script_field_slots(&type_symbols);
    let const_values = semantic.const_values()?;
    let schema_defaults = semantic.schema_defaults(&type_symbols, &const_values);
    let facts = CompilerFacts {
        script_function_symbols,
        script_function_signatures,
        script_method_ids: BTreeMap::new(),
        script_method_signatures: BTreeMap::new(),
        script_field_slots,
        schema_defaults,
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
    let script_function_signatures = semantic.script_function_signatures();
    let script_impl_methods = semantic.script_impl_methods();
    let script_method_ids = script_method_ids(&script_impl_methods);
    let script_method_signatures = script_method_signatures(&script_impl_methods);
    let type_symbols = semantic.type_symbols();
    let script_field_slots = semantic.script_field_slots(&type_symbols);
    let const_values = semantic.const_values()?;
    let schema_defaults = semantic.schema_defaults(&type_symbols, &const_values);
    let facts = CompilerFacts {
        script_function_symbols,
        script_function_signatures,
        script_method_ids,
        script_method_signatures,
        script_field_slots,
        schema_defaults,
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
    insert_script_impl_methods(&mut program, script_impl_methods, &facts)?;

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
    let script_function_signatures = semantic.script_function_signatures();
    let script_impl_methods = semantic.script_impl_methods();
    let script_method_ids = script_method_ids(&script_impl_methods);
    let script_method_signatures = script_method_signatures(&script_impl_methods);
    let type_symbols = semantic.type_symbols();
    let script_field_slots = semantic.script_field_slots(&type_symbols);
    let const_values = semantic.const_values()?;
    let schema_defaults = semantic.schema_defaults(&type_symbols, &const_values);
    let facts = CompilerFacts {
        script_function_symbols,
        script_function_signatures,
        script_method_ids,
        script_method_signatures,
        script_field_slots,
        schema_defaults,
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
    insert_script_impl_methods(&mut program, script_impl_methods, &facts)?;

    Ok(program)
}

fn insert_script_impl_methods(
    program: &mut Program,
    methods: Vec<script_impls::ScriptImplMethod<'_>>,
    facts: &CompilerFacts,
) -> CompileResult<()> {
    for method in methods {
        program.insert_script_method(
            method.target_type.clone(),
            method.method_name.clone(),
            method.method_id,
            method.symbol.clone(),
        );
        program.insert_function(
            Compiler::new_script_method_body(
                method.symbol,
                method.params,
                method.signature,
                method.body,
                method.bindings,
                &method.target_type,
                facts.clone(),
            )?
            .compile()?,
        );
    }
    Ok(())
}

fn script_method_ids(
    methods: &[script_impls::ScriptImplMethod<'_>],
) -> BTreeMap<(String, String), MethodId> {
    methods
        .iter()
        .map(|method| {
            (
                (method.target_type.clone(), method.method_name.clone()),
                method.method_id,
            )
        })
        .collect()
}

fn script_method_signatures(
    methods: &[script_impls::ScriptImplMethod<'_>],
) -> BTreeMap<(String, String), Vec<ParamHint>> {
    methods
        .iter()
        .map(|method| {
            (
                (method.target_type.clone(), method.method_name.clone()),
                method.signature.params.clone(),
            )
        })
        .collect()
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

    fn script_function_signatures(&self) -> BTreeMap<HirDeclId, Vec<ParamHint>> {
        self.script_function_symbols()
            .keys()
            .filter_map(|declaration| {
                self.graph
                    .function_signature(*declaration)
                    .map(|signature| (*declaration, signature.params.clone()))
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

    fn script_field_slots(&self, type_symbols: &BTreeMap<HirDeclId, String>) -> ScriptFieldSlots {
        ScriptFieldSlots::from_graph(&self.graph, type_symbols)
    }

    fn schema_defaults(
        &self,
        type_symbols: &BTreeMap<HirDeclId, String>,
        const_values: &BTreeMap<HirDeclId, Constant>,
    ) -> ScriptSchemaDefaults {
        source_schema_defaults(
            &self.parsed,
            &self.graph,
            self.module,
            type_symbols,
            self.const_values_by_name(const_values),
        )
    }

    fn const_values_by_name(
        &self,
        const_values: &BTreeMap<HirDeclId, Constant>,
    ) -> BTreeMap<String, Constant> {
        let mut values = BTreeMap::new();
        let Some(declarations) = self.graph.module(self.module) else {
            return values;
        };
        for item in &self.parsed.items {
            let ItemKind::Const(item) = &item.kind else {
                continue;
            };
            let Some(declaration) = declarations.get(&item.name) else {
                continue;
            };
            let Some(value) = const_values.get(&declaration).cloned() else {
                continue;
            };
            values.insert(item.name.clone(), value);
        }
        values
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

    fn script_impl_methods(&self) -> Vec<script_impls::ScriptImplMethod<'_>> {
        script_impls::source_methods(&self.parsed, &self.graph, self.module)
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

    fn script_function_signatures(&self) -> BTreeMap<HirDeclId, Vec<ParamHint>> {
        self.script_function_symbols()
            .keys()
            .filter_map(|declaration| {
                self.graph
                    .function_signature(*declaration)
                    .map(|signature| (*declaration, signature.params.clone()))
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

    fn script_field_slots(&self, type_symbols: &BTreeMap<HirDeclId, String>) -> ScriptFieldSlots {
        ScriptFieldSlots::from_graph(&self.graph, type_symbols)
    }

    fn schema_defaults(
        &self,
        type_symbols: &BTreeMap<HirDeclId, String>,
        const_values: &BTreeMap<HirDeclId, Constant>,
    ) -> ScriptSchemaDefaults {
        let mut defaults = ScriptSchemaDefaults::default();
        for module in &self.modules {
            let Some(parsed) = self.parsed.get(module) else {
                continue;
            };
            defaults.merge(source_schema_defaults(
                parsed,
                &self.graph,
                *module,
                type_symbols,
                self.const_values_by_name(*module, const_values),
            ));
        }
        defaults
    }

    fn const_values_by_name(
        &self,
        module: ModuleId,
        const_values: &BTreeMap<HirDeclId, Constant>,
    ) -> BTreeMap<String, Constant> {
        let mut values = self.imported_const_values(module, const_values);
        let Some(parsed) = self.parsed.get(&module) else {
            return values;
        };
        let Some(declarations) = self.graph.module(module) else {
            return values;
        };
        for item in &parsed.items {
            let ItemKind::Const(item) = &item.kind else {
                continue;
            };
            let Some(declaration) = declarations.get(&item.name) else {
                continue;
            };
            let Some(value) = const_values.get(&declaration).cloned() else {
                continue;
            };
            values.insert(item.name.clone(), value);
        }
        values
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

    fn script_impl_methods(&self) -> Vec<script_impls::ScriptImplMethod<'_>> {
        script_impls::module_methods(&self.parsed, &self.graph)
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
    script_types: ScriptTypeFlow,
    bindings: &'ast BindingMap,
    next_register: u16,
    param_defaults: Vec<Option<Expr>>,
    body: &'ast Block,
    facts: CompilerFacts,
    loop_stack: Vec<LoopContext>,
}

impl<'ast> Compiler<'ast> {
    fn new(
        code_name: String,
        function: &'ast FunctionItem,
        signature: &FunctionSignature,
        bindings: &'ast BindingMap,
        facts: CompilerFacts,
    ) -> CompileResult<Self> {
        Self::new_body(
            code_name,
            &function.params,
            signature,
            &function.body,
            bindings,
            facts,
        )
    }

    fn new_body(
        code_name: String,
        params: &'ast [Param],
        signature: &FunctionSignature,
        body: &'ast Block,
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
        let param_defaults = params
            .iter()
            .map(|param| param.default_value.is_some())
            .collect::<Vec<_>>();
        let mut locals = HashMap::new();
        let mut hir_locals = HashMap::new();
        let mut script_types = ScriptTypeFlow::default();
        let parameter_locals = bindings
            .locals()
            .filter(|local| local.kind == LocalBindingKind::Parameter)
            .map(|local| local.id)
            .collect::<Vec<_>>();
        let known_type_names = facts.known_type_names();
        for (index, param) in signature.params.iter().enumerate() {
            let register = u16::try_from(index)
                .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?;
            locals.insert(param.name.clone(), Register(register));
            let script_type = param
                .type_hint
                .as_ref()
                .and_then(|hint| type_hint_script_type(hint, known_type_names.iter()));
            if let Some(local) = parameter_locals.get(index).copied() {
                hir_locals.insert(local, Register(register));
                script_types.set_local(local, &param.name, script_type);
            } else {
                script_types.set_name(&param.name, script_type);
            }
        }

        Ok(Self {
            code: CodeObject::new(code_name, 0)
                .with_params(param_names)
                .with_param_defaults(param_defaults),
            locals,
            hir_locals,
            script_types,
            bindings,
            next_register: param_count,
            param_defaults: params
                .iter()
                .map(|param| param.default_value.clone())
                .collect(),
            body,
            facts,
            loop_stack: Vec::new(),
        })
    }

    fn new_script_method_body(
        code_name: String,
        params: &'ast [Param],
        signature: &FunctionSignature,
        body: &'ast Block,
        bindings: &'ast BindingMap,
        receiver_type: &str,
        facts: CompilerFacts,
    ) -> CompileResult<Self> {
        let mut compiler = Self::new_body(code_name, params, signature, body, bindings, facts)?;
        compiler
            .script_types
            .set_name("self", Some(receiver_type.to_owned()));
        Ok(compiler)
    }

    fn new_lambda(
        name: String,
        _lambda_span: Span,
        params: &[Param],
        fallback_body: &'ast Block,
        captures: &[LambdaCapture],
        bindings: &'ast BindingMap,
        facts: CompilerFacts,
    ) -> CompileResult<Self> {
        let capture_count = u16::try_from(captures.len())
            .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?;
        let param_count = u16::try_from(params.len())
            .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?;
        let param_names = params
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<_>>();
        let param_default_flags = vec![false; params.len()];
        let mut locals = HashMap::new();
        let mut hir_locals = HashMap::new();
        let mut script_types = ScriptTypeFlow::default();

        for (index, capture) in captures.iter().enumerate() {
            let register = Register(
                u16::try_from(index)
                    .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?,
            );
            locals.insert(capture.name.clone(), register);
            hir_locals.insert(capture.local, register);
        }
        let known_type_names = facts.known_type_names();
        for (index, param) in params.iter().enumerate() {
            let register = Register(
                capture_count
                    .checked_add(
                        u16::try_from(index)
                            .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?,
                    )
                    .ok_or_else(|| CompileError::new(CompileErrorKind::RegisterOverflow))?,
            );
            locals.insert(param.name.clone(), register);
            let script_type = param.type_hint.as_ref().and_then(|hint| {
                type_hint_script_type(&HirTypeHint::from_syntax(hint), known_type_names.iter())
            });
            if let Some(local) =
                bindings.local_named_at(&param.name, LocalBindingKind::LambdaParameter, param.span)
            {
                hir_locals.insert(local, register);
                script_types.set_local(local, &param.name, script_type);
            } else {
                script_types.set_name(&param.name, script_type);
            }
        }

        Ok(Self {
            code: CodeObject::new(name, 0)
                .with_params(param_names)
                .with_param_defaults(param_default_flags)
                .with_capture_count(capture_count),
            locals,
            hir_locals,
            script_types,
            bindings,
            next_register: capture_count
                .checked_add(param_count)
                .ok_or_else(|| CompileError::new(CompileErrorKind::RegisterOverflow))?,
            param_defaults: vec![None; params.len()],
            body: fallback_body,
            facts,
            loop_stack: Vec::new(),
        })
    }

    fn compile(mut self) -> CompileResult<CodeObject> {
        self.compile_param_defaults()?;
        let returned = self.compile_statements(&self.body.statements)?;
        if !returned {
            let null = self.emit_constant(Constant::Null)?;
            self.emit(InstructionKind::Return { src: null });
        }
        self.code.register_count = self.next_register;
        Ok(self.code)
    }

    fn compile_param_defaults(&mut self) -> CompileResult<()> {
        for index in 0..self.param_defaults.len() {
            let Some(default_value) = self.param_defaults[index].clone() else {
                continue;
            };
            let param = Register(
                self.code
                    .capture_count
                    .checked_add(
                        u16::try_from(index)
                            .map_err(|_| CompileError::new(CompileErrorKind::RegisterOverflow))?,
                    )
                    .ok_or_else(|| CompileError::new(CompileErrorKind::RegisterOverflow))?,
            );
            let skip_default = self.emit_jump_if_not_missing(param);
            let value = self.compile_expr(&default_value)?;
            self.emit(InstructionKind::Move {
                dst: param,
                src: value,
            });
            self.patch_jump(skip_default, self.current_offset())?;
        }
        Ok(())
    }

    fn tuple_enum_constructor_call(&self, callee: &Expr) -> Option<(String, String)> {
        let ExprKind::Path(path) = &callee.kind else {
            return None;
        };
        let (_, variant) = enum_variant_path(path)?;
        let enum_name = self.type_symbol_at_span(callee.span)?;
        Some((enum_name, variant))
    }

    fn script_function_call(&self, callee: &Expr) -> Option<(HirDeclId, String)> {
        let Some(BindingResolution::Declaration(declaration)) =
            self.bindings.resolution_at_span(callee.span)
        else {
            return None;
        };
        self.facts
            .script_function_symbols
            .get(declaration)
            .cloned()
            .map(|name| (*declaration, name))
    }

    fn local_callee(&self, callee: &Expr) -> Option<HirLocalId> {
        let Some(BindingResolution::Local(local)) = self.bindings.resolution_at_span(callee.span)
        else {
            return None;
        };
        Some(*local)
    }

    fn type_symbol_at_span(&self, span: Span) -> Option<String> {
        let Some(BindingResolution::Declaration(declaration)) =
            self.bindings.resolution_at_span(span)
        else {
            return None;
        };
        self.facts.type_symbols.get(declaration).cloned()
    }

    fn host_method_receiver_type(&self, callee: &Expr) -> Option<String> {
        match &callee.kind {
            ExprKind::Field { base, .. } => self.script_type_for_expr(base),
            ExprKind::Path(path) => {
                let [receiver, _method] = path.as_slice() else {
                    return None;
                };
                self.script_types.name(receiver)
            }
            _ => None,
        }
    }

    fn script_record_field_slot_for_receiver(&self, receiver: &Expr, field: &str) -> Option<usize> {
        let type_name = self.script_type_for_expr(receiver)?;
        self.script_record_field_slot_for_type(&type_name, field)
    }

    fn script_enum_field_slot_for_receiver(&self, receiver: &Expr, field: &str) -> Option<usize> {
        let fact = self.script_fact_for_expr(receiver)?;
        let variant = fact.enum_variant.as_deref()?;
        self.facts
            .script_field_slots
            .enum_variant(&fact.type_name, variant, field)
    }

    fn script_record_field_slot_for_type(&self, type_name: &str, field: &str) -> Option<usize> {
        self.facts.script_field_slots.record(type_name, field)
    }

    fn script_type_for_receiver_path(&self, receiver_path: &[String]) -> Option<String> {
        let [receiver] = receiver_path else {
            return None;
        };
        self.script_types.name(receiver)
    }

    fn script_method_id_for_type(&self, type_name: &str, method: &str) -> Option<MethodId> {
        self.facts
            .script_method_ids
            .get(&(type_name.to_owned(), method.to_owned()))
            .copied()
    }

    fn script_method_params(&self, type_name: &str, method: &str) -> Option<Vec<ParamHint>> {
        self.facts
            .script_method_signatures
            .get(&(type_name.to_owned(), method.to_owned()))
            .cloned()
    }

    fn script_type_for_expr(&self, expr: &Expr) -> Option<String> {
        expression_script_type(
            expr,
            |span| self.type_symbol_at_span(span),
            |span| self.script_types.local_at_span(self.bindings, span),
            |name| self.script_types.name(name),
        )
    }

    fn script_fact_for_expr(&self, expr: &Expr) -> Option<ScriptTypeFact> {
        expression_script_fact(
            expr,
            |span| self.type_symbol_at_span(span),
            |span| self.script_types.local_fact_at_span(self.bindings, span),
            |name| self.script_types.name_fact(name),
        )
    }

    fn emit_constant(&mut self, constant: Constant) -> CompileResult<Register> {
        let dst = self.alloc_register()?;
        let constant = self.code.push_constant(constant);
        self.emit(InstructionKind::LoadConst { dst, constant });
        Ok(dst)
    }

    fn emit_bool_constant_to(&mut self, dst: Register, value: bool) {
        self.emit_constant_to(dst, Constant::Bool(value));
    }

    fn emit_constant_to(&mut self, dst: Register, value: Constant) {
        let constant = self.code.push_constant(value);
        self.emit(InstructionKind::LoadConst { dst, constant });
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

    fn emit_spanned(&mut self, kind: InstructionKind, span: Span) {
        self.code
            .push_instruction(Instruction::new(kind).with_span(span));
    }

    fn emit_jump_if_false(&mut self, condition: Register) -> usize {
        let offset = self.current_offset();
        self.emit(InstructionKind::JumpIfFalse {
            condition,
            target: InstructionOffset(usize::MAX),
        });
        offset
    }

    fn emit_jump_if_not_missing(&mut self, value: Register) -> usize {
        let offset = self.current_offset();
        self.emit(InstructionKind::JumpIfNotMissing {
            value,
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

    fn emit_iter_next(&mut self, iterator: Register, dst: Register) -> usize {
        let offset = self.current_offset();
        self.emit(InstructionKind::IterNext {
            iterator,
            dst,
            jump_if_done: InstructionOffset(usize::MAX),
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
            | InstructionKind::JumpIfNotMissing {
                target: jump_target,
                ..
            }
            | InstructionKind::Jump {
                target: jump_target,
            }
            | InstructionKind::IterNext {
                jump_if_done: jump_target,
                ..
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

fn reject_named_args(args: &[Argument], context: &'static str) -> CompileResult<()> {
    if args.iter().any(|arg| arg.name.is_some()) {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            context,
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CallArgument;
    use vela_common::MethodId;

    fn semantic_diagnostic_codes(error: CompileError) -> Vec<String> {
        let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
            panic!("expected semantic diagnostics");
        };
        diagnostics
            .into_iter()
            .filter_map(|diagnostic| diagnostic.code)
            .collect()
    }

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
    fn compiler_rejects_duplicate_parameters_from_hir() {
        let error = compile_program_source(
            SourceId::new(1),
            r#"
fn main(amount, amount) {
    return amount;
}
"#,
        )
        .expect_err("duplicate parameter should fail before bytecode generation");

        let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
            panic!("expected semantic diagnostics");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_parameter"))
        );
    }

    #[test]
    fn compiler_rejects_duplicate_schema_members_from_hir() {
        let error = compile_program_source(
            SourceId::new(1),
            r#"
struct Reward {
    count: int,
    count: string
}

enum QuestProgress {
    Active { quest_id: int, quest_id: string },
    Active
}

trait Rewardable {
    fn reward(self, amount);
    fn reward(self, bonus);
}

fn main() {
    return 1;
}
"#,
        )
        .expect_err("duplicate schema members should fail before bytecode generation");

        let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
            panic!("expected semantic diagnostics");
        };
        for code in [
            "hir::duplicate_field",
            "hir::duplicate_variant",
            "hir::duplicate_variant_field",
            "hir::duplicate_trait_method",
        ] {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code.as_deref() == Some(code)),
                "missing diagnostic {code}: {diagnostics:?}"
            );
        }
    }

    #[test]
    fn compiler_rejects_missing_required_constructor_fields() {
        let error = compile_program_source(
            SourceId::new(1),
            r#"
struct Reward {
    item_id: string,
    count: int = 1,
}

fn main() {
    return Reward { count: 2 };
}
"#,
        )
        .expect_err("missing required constructor field should fail");

        assert_eq!(
            semantic_diagnostic_codes(error),
            ["compiler::missing_constructor_field"]
        );
    }

    #[test]
    fn compiler_rejects_unknown_constructor_fields() {
        let error = compile_program_source(
            SourceId::new(1),
            r#"
struct Reward {
    item_id: string,
    count: int,
}

fn main() {
    return Reward { item_id: "gold", count: 2, bonus: 5 };
}
"#,
        )
        .expect_err("unknown constructor field should fail");

        assert_eq!(
            semantic_diagnostic_codes(error),
            ["compiler::unknown_constructor_field"]
        );
    }

    #[test]
    fn compiler_rejects_duplicate_constructor_fields() {
        let error = compile_program_source(
            SourceId::new(1),
            r#"
struct Reward {
    item_id: string,
    count: int,
}

fn main() {
    return Reward { item_id: "gold", item_id: "xp", count: 2 };
}
"#,
        )
        .expect_err("duplicate constructor field should fail");

        assert_eq!(
            semantic_diagnostic_codes(error),
            ["compiler::duplicate_constructor_field"]
        );
    }

    #[test]
    fn compiler_rejects_invalid_tuple_constructor_arity() {
        let missing = compile_program_source(
            SourceId::new(1),
            r#"
enum Damage {
    Magical(amount: int, element: string = "arcane"),
}

fn main() {
    return Damage.Magical();
}
"#,
        )
        .expect_err("missing tuple constructor field should fail");
        let extra = compile_program_source(
            SourceId::new(2),
            r#"
enum Damage {
    Magical(amount: int),
}

fn main() {
    return Damage.Magical(1, 2);
}
"#,
        )
        .expect_err("extra tuple constructor field should fail");

        assert_eq!(
            semantic_diagnostic_codes(missing),
            ["compiler::missing_constructor_field"]
        );
        assert_eq!(
            semantic_diagnostic_codes(extra),
            ["compiler::unknown_constructor_field"]
        );
    }

    #[test]
    fn compiler_rejects_unknown_constructor_variants() {
        let error = compile_program_source(
            SourceId::new(1),
            r#"
enum Damage {
    Physical { amount: int },
}

fn main() {
    return Damage.Magical { amount: 7 };
}
"#,
        )
        .expect_err("unknown constructor variant should fail");

        assert_eq!(
            semantic_diagnostic_codes(error),
            ["compiler::unknown_constructor_variant"]
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

        assert_eq!(unresolved.labels.len(), 2);
        assert_eq!(unresolved.labels[0].message, "did you mean `player`?");
        assert_eq!(
            unresolved.labels[1].message,
            "candidate `player` is declared here"
        );
    }

    #[test]
    fn compiler_rejects_unknown_schema_hints_from_hir_with_candidates() {
        let error = compile_function_source(
            SourceId::new(1),
            r#"
struct Player { level: int }

fn main(player: Plyer) {
    return 1;
}
"#,
            "main",
        )
        .expect_err("unknown schema hint should fail before bytecode generation");

        let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
            panic!("expected semantic diagnostics");
        };
        let unknown_schema = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::unknown_schema"))
            .expect("unknown schema diagnostic");

        assert_eq!(unknown_schema.message, "unknown schema `Plyer`");
        assert!(
            unknown_schema
                .labels
                .iter()
                .any(|label| label.message == "candidate `Player` is declared here")
        );
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
    fn compiler_rejects_duplicate_imports_before_codegen() {
        let error = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.reward"),
                "pub fn grant() { return 1; }",
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.config"),
                "pub const BONUS = 2",
            ),
            ModuleSource::new(
                SourceId::new(3),
                ModulePath::from_dotted("game.main"),
                r#"
use game.reward.grant as reward
use game.config.BONUS as reward

fn main() {
    return reward;
}
"#,
            ),
        ])
        .expect_err("duplicate import should fail before bytecode generation");

        let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
            panic!("expected semantic diagnostics");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_import"))
        );
    }

    #[test]
    fn compiler_rejects_import_conflicts_before_codegen() {
        let error = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.reward"),
                "pub fn grant() { return 1; }",
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.main"),
                r#"
use game.reward.grant

fn grant() {
    return 2;
}
"#,
            ),
        ])
        .expect_err("import conflict should fail before bytecode generation");

        let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
            panic!("expected semantic diagnostics");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::import_conflict"))
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
    fn compiler_registers_impl_methods_as_script_dispatch_targets() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    return Player { level: 7 }.bonus(5);
}
"#,
        )
        .expect("impl method should compile as hidden dispatch target");

        let method = program
            .script_method("Player", "bonus")
            .expect("script impl method dispatch target");
        assert_eq!(method.params, ["self", "amount"]);
        let method_id = stable_test_trait_method_id("main.BonusSource", "bonus");
        assert_eq!(program.script_method_id("Player", "bonus"), Some(method_id));
        assert_eq!(
            program
                .script_method_by_id("Player", method_id)
                .expect("script method by stable id")
                .params,
            ["self", "amount"]
        );
        let main = program.function("main").expect("main function");
        assert!(main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::CallMethodId {
                method_id: lowered,
                ..
            } if lowered == method_id
        )));
        assert!(program.function("bonus").is_none());
    }

    #[test]
    fn compiler_lowers_named_and_default_script_method_args() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource {
    fn bonus(self, amount, multiplier = 2, offset = 1) -> int;
}
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount, multiplier = 2, offset = 1) -> int {
        return self.level + amount * multiplier + offset;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(offset = 4, amount = 5);
}
"#,
        )
        .expect("named/default method call should compile");

        let method_id = stable_test_trait_method_id("main.BonusSource", "bonus");
        let main = program.function("main").expect("main function");
        let args = main
            .instructions
            .iter()
            .find_map(|instruction| match &instruction.kind {
                InstructionKind::CallMethodId {
                    method_id: lowered,
                    args,
                    ..
                } if *lowered == method_id => Some(args),
                _ => None,
            })
            .expect("named/default method call should lower by stable id");
        assert_eq!(args.len(), 3);
        assert!(matches!(args[0], CallArgument::Register(_)));
        assert_eq!(args[1], CallArgument::Missing);
        assert!(matches!(args[2], CallArgument::Register(_)));
    }

    #[test]
    fn compiler_registers_host_target_impl_methods_as_script_dispatch_targets() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return reflect.get(self, "level") + amount;
    }
}

fn main(player) {
    return player.bonus(5);
}
"#,
        )
        .expect("host target impl method should compile as hidden dispatch target");

        let method = program
            .script_method("Player", "bonus")
            .expect("host target script impl method dispatch target");
        assert_eq!(method.params, ["self", "amount"]);
        let method_id = stable_test_trait_method_id("main.BonusSource", "bonus");
        assert_eq!(program.script_method_id("Player", "bonus"), Some(method_id));
    }

    #[test]
    fn compiler_registers_trait_default_methods_as_dispatch_targets() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource {
    fn bonus(self, amount) -> int { return self.level + amount; }
}
struct Player { level: int }

impl BonusSource for Player {}

fn main() {
    return Player { level: 7 }.bonus(5);
}
"#,
        )
        .expect("trait default method should compile as hidden dispatch target");

        let method = program
            .script_method("Player", "bonus")
            .expect("trait default method dispatch target");
        assert_eq!(method.params, ["self", "amount"]);
        let method_id = stable_test_trait_method_id("main.BonusSource", "bonus");
        assert_eq!(program.script_method_id("Player", "bonus"), Some(method_id));
        assert!(program.script_method_by_id("Player", method_id).is_some());
        let main = program.function("main").expect("main function");
        assert!(main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::CallMethodId {
                method_id: lowered,
                ..
            } if lowered == method_id
        )));
        assert!(program.function("bonus").is_none());
    }

    #[test]
    fn compiler_specializes_self_method_calls_by_method_id() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource {
    fn label(self) -> string;
    fn summary(self) -> string { return self.label(); }
}
struct Player { name: string }

impl BonusSource for Player {
    fn label(self) -> string {
        return self.name;
    }
}

fn main() {
    return Player { name: "hero" }.summary();
}
"#,
        )
        .expect("self method calls should specialize by method id");

        let label_id = stable_test_trait_method_id("main.BonusSource", "label");
        let summary = program
            .script_method("Player", "summary")
            .expect("trait default summary method");
        assert!(summary.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::CallMethodId {
                method_id: lowered,
                ..
            } if lowered == label_id
        )));
    }

    #[test]
    fn compiler_specializes_captured_receiver_method_calls_by_method_id() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    let bonus = |ignored| player.bonus(5);
    return bonus(null);
}
"#,
        )
        .expect("captured receiver method should specialize by method id");

        let method_id = stable_test_trait_method_id("main.BonusSource", "bonus");
        let main = program.function("main").expect("main function");
        let closure = main
            .instructions
            .iter()
            .find_map(|instruction| match &instruction.kind {
                InstructionKind::MakeClosure { code, .. } => Some(code),
                _ => None,
            })
            .expect("capturing closure code");
        assert!(closure.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::CallMethodId {
                method_id: lowered,
                ..
            } if lowered == method_id
        )));
    }

    #[test]
    fn compiler_specializes_binding_pattern_receiver_method_calls_by_method_id() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    return match player {
        bound => bound.bonus(5),
    };
}
"#,
        )
        .expect("binding pattern receiver method should specialize by method id");

        let method_id = stable_test_trait_method_id("main.BonusSource", "bonus");
        let main = program.function("main").expect("main function");
        assert!(main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::CallMethodId {
                method_id: lowered,
                ..
            } if lowered == method_id
        )));
    }

    #[test]
    fn compiler_specializes_record_variant_field_method_calls_by_method_id() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

enum Event {
    Grant { player: Player },
    None,
}

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let event = Event.Grant { player: Player { level: 7 } };
    return match event {
        Event.Grant { player } => player.bonus(5),
        _ => 0,
    };
}
"#,
        )
        .expect("record variant field receiver method should specialize by method id");

        let method_id = stable_test_trait_method_id("main.BonusSource", "bonus");
        let main = program.function("main").expect("main function");
        assert!(main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::CallMethodId {
                method_id: lowered,
                ..
            } if lowered == method_id
        )));
    }

    #[test]
    fn compiler_specializes_tuple_variant_field_method_calls_by_method_id() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

enum Event {
    Grant(player: Player),
    None,
}

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let event = Event.Grant(Player { level: 7 });
    return match event {
        Event.Grant(player) => player.bonus(5),
        _ => 0,
    };
}
"#,
        )
        .expect("tuple variant field receiver method should specialize by method id");

        let method_id = stable_test_trait_method_id("main.BonusSource", "bonus");
        let main = program.function("main").expect("main function");
        assert!(main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::CallMethodId {
                method_id: lowered,
                ..
            } if lowered == method_id
        )));
    }

    #[test]
    fn compiler_specializes_let_record_method_calls_by_method_id() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(5);
}
"#,
        )
        .expect("let-bound script record method should specialize by method id");

        let method_id = stable_test_trait_method_id("main.BonusSource", "bonus");
        let main = program.function("main").expect("main function");
        assert!(main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::CallMethodId {
                method_id: lowered,
                ..
            } if lowered == method_id
        )));
    }

    #[test]
    fn compiler_specializes_typed_parameter_method_calls_by_method_id() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main(player: Player) {
    return player.bonus(5);
}
"#,
        )
        .expect("typed script parameter method should specialize by method id");

        let method_id = stable_test_trait_method_id("main.BonusSource", "bonus");
        let main = program.function("main").expect("main function");
        assert!(main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::CallMethodId {
                method_id: lowered,
                ..
            } if lowered == method_id
        )));
    }

    #[test]
    fn compiler_specializes_typed_let_method_calls_by_method_id() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn source() {
    return Player { level: 7 };
}

fn main() {
    let player: Player = source();
    return player.bonus(5);
}
"#,
        )
        .expect("typed let method should specialize by method id");

        let method_id = stable_test_trait_method_id("main.BonusSource", "bonus");
        let main = program.function("main").expect("main function");
        assert!(main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::CallMethodId {
                method_id: lowered,
                ..
            } if lowered == method_id
        )));
    }

    #[test]
    fn compiler_specializes_module_typed_parameter_method_calls_by_method_id() {
        let program = compile_module_sources(&[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_dotted("game.model"),
                r#"
pub trait BonusSource { fn bonus(self, amount) -> int; }
pub struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_dotted("game.combat"),
                r#"
use game.model.Player

pub fn main(player: Player) {
    return player.bonus(5);
}
"#,
            ),
        ])
        .expect("module typed parameter method should specialize by method id");

        let method_id = stable_test_trait_method_id("game.model.BonusSource", "bonus");
        let main = program
            .function("game.combat.main")
            .expect("game.combat.main function");
        assert!(main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::CallMethodId {
                method_id: lowered,
                ..
            } if lowered == method_id
        )));
    }

    #[test]
    fn compiler_indexes_script_methods_by_receiver_and_method_id() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
trait BonusSource {
    fn bonus(self) -> int { return self.value; }
}
struct Player { value: int }
struct Monster { value: int }

impl BonusSource for Player {}
impl BonusSource for Monster {}

fn main() {
    return Player { value: 1 }.bonus() + Monster { value: 2 }.bonus();
}
"#,
        )
        .expect("shared trait method id should index per receiver");
        let method_id = stable_test_trait_method_id("main.BonusSource", "bonus");

        assert!(program.script_method_by_id("Player", method_id).is_some());
        assert!(program.script_method_by_id("Monster", method_id).is_some());
        assert!(program.script_method_by_id("Missing", method_id).is_none());
    }

    fn stable_test_trait_method_id(trait_name: &str, method_name: &str) -> MethodId {
        MethodId::new(stable_test_id("trait_method", trait_name, method_name))
    }

    fn stable_test_id(kind: &str, owner: &str, member: &str) -> u32 {
        let mut hash = 0x811c_9dc5;
        for byte in kind
            .bytes()
            .chain([0])
            .chain(owner.bytes())
            .chain([0])
            .chain(member.bytes())
        {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
        if hash == 0 { 1 } else { hash }
    }

    #[test]
    fn compiler_lowers_configured_host_method_calls() {
        let method = HostMethodId::new(5);
        let code = compile_function_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.grant_exp(20);
    return 1;
}
"#,
            "main",
            &CompilerOptions::new().with_host_method("grant_exp", method),
        )
        .expect("host method call should compile");

        assert!(code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::CallHostMethod {
                method: lowered_method,
                ..
            } if lowered_method == method
        )));
    }

    #[test]
    fn compiler_lowers_configured_host_method_calls_on_field_paths() {
        let inventory = FieldId::new(3);
        let method = HostMethodId::new(5);
        let code = compile_function_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.inventory.add("gold", 20);
    return 1;
}
"#,
            "main",
            &CompilerOptions::new()
                .with_host_field("inventory", inventory)
                .with_host_method("add", method),
        )
        .expect("host field method call should compile");

        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::CallHostMethod {
                method: lowered_method,
                segments,
                ..
            } if *lowered_method == method
                && segments.as_slice() == [HostPathSegment::Field(inventory)]
        )));
    }

    #[test]
    fn compiler_lowers_configured_host_method_calls_on_indexed_paths() {
        let inventory = FieldId::new(3);
        let items = FieldId::new(4);
        let method = HostMethodId::new(5);
        let code = compile_function_source_with_options(
            SourceId::new(1),
            r#"
fn main(player, item_id) {
    player.inventory.items[item_id].grant(20);
    return 1;
}
"#,
            "main",
            &CompilerOptions::new()
                .with_host_field("inventory", inventory)
                .with_host_field("items", items)
                .with_host_method("grant", method),
        )
        .expect("indexed host method call should compile");

        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::CallHostMethod {
                method: lowered_method,
                segments,
                ..
            } if *lowered_method == method
                && matches!(
                    segments.as_slice(),
                    [
                        HostPathSegment::Field(first),
                        HostPathSegment::Field(second),
                        HostPathSegment::Value(_)
                    ] if *first == inventory && *second == items
                )
        )));
    }

    #[test]
    fn compiler_lowers_nested_host_field_paths() {
        let stats = FieldId::new(3);
        let level = FieldId::new(4);
        let code = compile_function_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.stats.level += 2;
    return player.stats.level;
}
"#,
            "main",
            &CompilerOptions::new()
                .with_host_field("stats", stats)
                .with_host_field("level", level),
        )
        .expect("nested host field path should compile");

        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::AddHostPath {
                segments,
                ..
            } if segments.as_slice() == [
                HostPathSegment::Field(stats),
                HostPathSegment::Field(level)
            ]
        )));
        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::GetHostPath {
                segments,
                ..
            } if segments.as_slice() == [
                HostPathSegment::Field(stats),
                HostPathSegment::Field(level)
            ]
        )));
    }

    #[test]
    fn compiler_lowers_indexed_host_field_paths() {
        let inventory = FieldId::new(3);
        let items = FieldId::new(4);
        let count = FieldId::new(5);
        let code = compile_function_source_with_options(
            SourceId::new(1),
            r#"
fn main(player, item_id) {
    player.inventory.items[item_id].count += 1;
    return player.inventory.items[item_id].count;
}
"#,
            "main",
            &CompilerOptions::new()
                .with_host_field("inventory", inventory)
                .with_host_field("items", items)
                .with_host_field("count", count),
        )
        .expect("indexed host field path should compile");

        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::AddHostPath {
                segments,
                ..
            } if matches!(
                segments.as_slice(),
                [
                    HostPathSegment::Field(first),
                    HostPathSegment::Field(second),
                    HostPathSegment::Value(_),
                    HostPathSegment::Field(third)
                ] if *first == inventory && *second == items && *third == count
            )
        )));
        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::GetHostPath {
                segments,
                ..
            } if matches!(
                segments.as_slice(),
                [
                    HostPathSegment::Field(first),
                    HostPathSegment::Field(second),
                    HostPathSegment::Value(_),
                    HostPathSegment::Field(third)
                ] if *first == inventory && *second == items && *third == count
            )
        )));
    }

    #[test]
    fn compiler_lowers_host_variant_field_paths() {
        let quest_progress = FieldId::new(3);
        let count = FieldId::new(4);
        let code = compile_function_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.quest_progress.count += 1;
    return player.quest_progress.count;
}
"#,
            "main",
            &CompilerOptions::new()
                .with_host_field("quest_progress", quest_progress)
                .with_host_variant_field("count", count),
        )
        .expect("host variant field path should compile");

        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::AddHostPath {
                segments,
                ..
            } if segments.as_slice() == [
                HostPathSegment::Field(quest_progress),
                HostPathSegment::VariantField(count)
            ]
        )));
        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::GetHostPath {
                segments,
                ..
            } if segments.as_slice() == [
                HostPathSegment::Field(quest_progress),
                HostPathSegment::VariantField(count)
            ]
        )));
    }

    #[test]
    fn compiler_lowers_host_sub_assignments() {
        let stats = FieldId::new(3);
        let level = FieldId::new(4);
        let code = compile_function_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.stats.level -= 2;
    return player.stats.level;
}
"#,
            "main",
            &CompilerOptions::new()
                .with_host_field("stats", stats)
                .with_host_field("level", level),
        )
        .expect("host sub assignment should compile");

        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::SubHostPath {
                segments,
                ..
            } if segments.as_slice() == [
                HostPathSegment::Field(stats),
                HostPathSegment::Field(level)
            ]
        )));
    }

    #[test]
    fn compiler_lowers_host_numeric_compound_assignments() {
        let stats = FieldId::new(3);
        let level = FieldId::new(4);
        let code = compile_function_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.stats.level *= 3;
    player.stats.level /= 2;
    player.stats.level %= 5;
    return player.stats.level;
}
"#,
            "main",
            &CompilerOptions::new()
                .with_host_field("stats", stats)
                .with_host_field("level", level),
        )
        .expect("host numeric compound assignments should compile");

        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::MulHostPath { segments, .. }
                if segments.as_slice() == [
                    HostPathSegment::Field(stats),
                    HostPathSegment::Field(level)
                ]
        )));
        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::DivHostPath { segments, .. }
                if segments.as_slice() == [
                    HostPathSegment::Field(stats),
                    HostPathSegment::Field(level)
                ]
        )));
        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::RemHostPath { segments, .. }
                if segments.as_slice() == [
                    HostPathSegment::Field(stats),
                    HostPathSegment::Field(level)
                ]
        )));
    }

    #[test]
    fn compiler_lowers_host_path_push_calls() {
        let inventory = FieldId::new(3);
        let rewards = FieldId::new(4);
        let code = compile_function_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    player.inventory.rewards.push("gold");
    return 1;
}
"#,
            "main",
            &CompilerOptions::new()
                .with_host_field("inventory", inventory)
                .with_host_field("rewards", rewards),
        )
        .expect("host path push should compile");

        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::PushHostPath {
                segments,
                ..
            } if segments.as_slice() == [
                HostPathSegment::Field(inventory),
                HostPathSegment::Field(rewards)
            ]
        )));
    }

    #[test]
    fn compiler_lowers_host_path_remove_calls() {
        let inventory = FieldId::new(3);
        let items = FieldId::new(4);
        let code = compile_function_source_with_options(
            SourceId::new(1),
            r#"
fn main(player) {
    let item_id = "gold";
    player.inventory.items[item_id].remove();
    return 1;
}
"#,
            "main",
            &CompilerOptions::new()
                .with_host_field("inventory", inventory)
                .with_host_field("items", items),
        )
        .expect("host path remove should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| match &instruction.kind {
                    InstructionKind::RemoveHostPath { segments, .. } => matches!(
                        segments.as_slice(),
                        [
                            HostPathSegment::Field(first),
                            HostPathSegment::Field(second),
                            HostPathSegment::Value(_)
                        ] if *first == inventory && *second == items
                    ),
                    _ => false,
                })
        );
    }

    #[test]
    fn compiler_lowers_radix_ints_and_exponent_floats() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    return 0x10 + 0b10 + 3.5e+1;
}
"#,
            "main",
        )
        .expect("numeric literal source should compile");

        assert!(code.constants.contains(&Constant::Int(16)));
        assert!(code.constants.contains(&Constant::Int(2)));
        assert!(code.constants.contains(&Constant::Float(35.0)));
    }

    #[test]
    fn compiler_rejects_uppercase_radix_prefixes_before_codegen() {
        let error = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    return 0X10 + 0B10;
}
"#,
            "main",
        )
        .expect_err("uppercase radix prefixes should be rejected by syntax validation");

        let CompileErrorKind::SyntaxDiagnostics(diagnostics) = error.kind else {
            panic!("expected syntax diagnostics");
        };
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code.as_deref() == Some("E_LEX_INT"))
        );
        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn compiler_accepts_leading_shebang() {
        let code = compile_function_source(
            SourceId::new(1),
            "#!/usr/bin/env vela\nfn main() { return 7; }\n",
            "main",
        )
        .expect("shebang source should compile");

        assert!(code.constants.contains(&Constant::Int(7)));
    }

    #[test]
    fn compiler_lowers_unicode_string_escapes() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"fn main() { return "\u{41}\u{7a}"; }"#,
            "main",
        )
        .expect("unicode escaped string source should compile");

        assert!(code.constants.contains(&Constant::String("Az".into())));
    }

    #[test]
    fn compiler_lowers_script_value_method_calls() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let values = [1, 2, 3];
    let reward = Reward { item_id: "gold", count: 3 };
    return values.len() + reward.item_id.len();
}
"#,
            "main",
        )
        .expect("script value method call should compile");

        assert!(code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::CallMethod { method, .. } if method == "len"
        )));
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
    fn compiler_lowers_parameter_defaults_and_named_script_args() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn grant(base, amount = 10, bonus = amount + 1) {
    return base + amount + bonus;
}

fn main() {
    return grant(bonus = 5, base = 1);
}
"#,
        )
        .expect("named args and defaults should compile");
        let grant = program.function("grant").expect("grant function");
        let main = program.function("main").expect("main function");

        assert_eq!(grant.param_defaults, [false, true, true]);
        assert!(grant.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::JumpIfNotMissing { .. }
        )));
        assert!(main.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::CallFunction { args, .. }
                if args.len() == 3 && matches!(args[1], CallArgument::Missing)
        )));
    }

    #[test]
    fn compiler_reports_script_call_argument_diagnostics() {
        let unknown = compile_program_source(
            SourceId::new(1),
            r#"
fn grant(base, amount = 10) {
    return base + amount;
}

fn main() {
    return grant(amunt = 2, base = 1);
}
"#,
        )
        .expect_err("unknown named argument should fail");
        let duplicate = compile_program_source(
            SourceId::new(2),
            r#"
fn grant(base, amount = 10) {
    return base + amount;
}

fn main() {
    return grant(1, base = 2);
}
"#,
        )
        .expect_err("duplicate argument should fail");
        let positional_after_named = compile_program_source(
            SourceId::new(3),
            r#"
fn grant(base, amount = 10) {
    return base + amount;
}

fn main() {
    return grant(amount = 2, 1);
}
"#,
        )
        .expect_err("positional argument after named argument should fail");
        let too_many = compile_program_source(
            SourceId::new(4),
            r#"
fn grant(base) {
    return base;
}

fn main() {
    return grant(1, 2);
}
"#,
        )
        .expect_err("too many arguments should fail");
        let missing = compile_program_source(
            SourceId::new(5),
            r#"
fn grant(base, amount = 10) {
    return base + amount;
}

fn main() {
    return grant();
}
"#,
        )
        .expect_err("missing required argument should fail");

        assert_eq!(
            semantic_diagnostic_codes(unknown),
            ["compiler::unknown_named_argument"]
        );
        assert_eq!(
            semantic_diagnostic_codes(duplicate),
            ["compiler::duplicate_argument"]
        );
        assert_eq!(
            semantic_diagnostic_codes(positional_after_named),
            [
                "compiler::positional_after_named_argument",
                "compiler::missing_required_argument"
            ]
        );
        assert_eq!(
            semantic_diagnostic_codes(too_many),
            ["compiler::too_many_arguments"]
        );
        assert_eq!(
            semantic_diagnostic_codes(missing),
            ["compiler::missing_required_argument"]
        );
    }

    #[test]
    fn compiler_lowers_lambdas_with_captures() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn make_adder(base) {
    return |value| value + base;
}

fn main() {
    let add = make_adder(10);
    return add(5);
}
"#,
        )
        .expect("capturing lambda should compile");
        let make_adder = program.function("make_adder").expect("make_adder function");
        let main = program.function("main").expect("main function");

        assert!(make_adder.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::MakeClosure { code, captures, .. }
                if code.capture_count == 1 && code.params == ["value"] && captures.len() == 1
        )));
        assert!(
            main.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::CallClosure { .. }))
        );
    }

    #[test]
    fn compiler_lowers_nested_lambda_transitive_captures() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
fn make_nested(base) {
    return |amount| {
        return |bonus| base + amount + bonus;
    };
}

fn main() {
    let make = make_nested(10);
    let add = make(4);
    return add(3);
}
"#,
        )
        .expect("nested capturing lambda should compile");
        let make_nested = program
            .function("make_nested")
            .expect("make_nested function");

        assert!(make_nested.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::MakeClosure { code, captures, .. }
                if code.capture_count == 1 && code.params == ["amount"] && captures.len() == 1
        )));
    }

    #[test]
    fn compiler_lowers_try_propagation() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
enum Result {
    Ok(value)
    Err(message)
}

fn checked(value) {
    return Result.Ok(value);
}

fn main() {
    let value = checked(10)?;
    return Result.Ok(value + 1);
}
"#,
            "main",
        )
        .expect("try propagation should compile");

        assert!(
            code.instructions.iter().any(|instruction| matches!(
                instruction.kind,
                InstructionKind::TryPropagate { .. }
            ))
        );
    }

    #[test]
    fn compiler_lowers_range_expressions() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let values = 1..=4;
    return values;
}
"#,
            "main",
        )
        .expect("range expression should compile");

        assert!(code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::MakeRange {
                inclusive: true,
                ..
            }
        )));
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

        assert!(
            code.instructions.iter().any(|instruction| matches!(
                &instruction.kind,
                InstructionKind::CallClosure { .. }
            ))
        );
        assert!(!code.instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            InstructionKind::CallFunction { name, .. } if name == "helper"
        )));
    }

    #[test]
    fn compiler_preserves_runtime_diagnostic_spans_for_calls_and_arithmetic() {
        let program = compile_program_source(
            SourceId::new(7),
            r#"
fn helper() {
    return 10 / 0;
}

fn main() {
    return helper();
}
"#,
        )
        .expect("diagnostic source spans should compile");

        let helper = program.function("helper").expect("helper function");
        let div_span = helper
            .instructions
            .iter()
            .find_map(|instruction| match instruction.kind {
                InstructionKind::Div { .. } => instruction.span,
                _ => None,
            })
            .expect("division instruction span");
        assert_eq!(div_span.source, SourceId::new(7));

        let main = program.function("main").expect("main function");
        let call_span = main
            .instructions
            .iter()
            .find_map(|instruction| match instruction.kind {
                InstructionKind::CallFunction { ref name, .. } if name == "helper" => {
                    instruction.span
                }
                _ => None,
            })
            .expect("script call instruction span");
        assert_eq!(call_span.source, SourceId::new(7));
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
pub enum Damage { Physical { amount: int } }
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
pub enum Damage { Physical { amount: int } }
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
    fn compiler_lowers_logical_short_circuit_operators() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    return false && fail() || true;
}
"#,
            "main",
        )
        .expect("logical operators should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::JumpIfFalse { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|instruction| { matches!(instruction.kind, InstructionKind::Jump { .. }) })
        );
        assert!(code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::CallNative { ref name, .. } if name == "fail"
        )));
    }

    #[test]
    fn compiler_lowers_block_and_if_expression_values() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = {
        let base = 2;
        base + 3;
    };
    return if value > 4 {
        value;
    } else {
        0;
    };
}
"#,
            "main",
        )
        .expect("block and if expression values should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::JumpIfFalse { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Move { .. }))
        );
    }

    #[test]
    fn compiler_lowers_if_expression_without_else_to_null() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = if false {
        1;
    };
    return value;
}
"#,
            "main",
        )
        .expect("if expression without else should compile");

        assert!(code.constants.contains(&Constant::Null));
    }

    #[test]
    fn compiler_lowers_returning_block_initializers() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let ignored = {
        return 7;
    };
    return 0;
}
"#,
            "main",
        )
        .expect("returning block initializer should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Return { .. }))
        );
    }

    #[test]
    fn compiler_lowers_returning_expression_operands() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main(kind) {
    log({
        return 7;
    });
    if kind == "if" {
        return if true {
            return 1;
        } else {
            return 2;
        };
    }
    return match kind {
        "match" => { return 3; },
        _ => { return 4; },
    };
}
"#,
            "main",
        )
        .expect("returning expression operands should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Return { .. }))
        );
    }

    #[test]
    fn compiler_lowers_returning_if_and_match_initializers() {
        compile_function_source(
            SourceId::new(1),
            r#"
fn main(flag) {
    let ignored = if flag {
        return 7;
    } else {
        return 8;
    };
    return 0;
}
"#,
            "main",
        )
        .expect("returning if initializer should compile");

        compile_function_source(
            SourceId::new(2),
            r#"
fn main(value) {
    let ignored = match value {
        1 => { return 10; },
        _ => { return 11; },
    };
    return 0;
}
"#,
            "main",
        )
        .expect("returning match initializer should compile");
    }

    #[test]
    fn compiler_lowers_match_expression_values() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let damage = Damage.Physical { amount: 7 };
    let value = match damage {
        Damage.Magical { amount } => amount + 100,
        Damage.Physical { amount } => {
            amount + 1;
        },
        _ => 0,
    };
    return value;
}
"#,
            "main",
        )
        .expect("match expression values should compile");

        assert!(
            code.instructions.iter().any(|instruction| matches!(
                instruction.kind,
                InstructionKind::EnumTagEqual { .. }
            ))
        );
        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Move { .. }))
        );
    }

    #[test]
    fn compiler_lowers_literal_match_patterns() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = 2;
    return match value {
        1 => 10,
        2 => 20,
        _ => 0,
    };
}
"#,
            "main",
        )
        .expect("literal match patterns should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Equal { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .filter(|instruction| matches!(
                    instruction.kind,
                    InstructionKind::JumpIfFalse { .. }
                ))
                .count()
                >= 2
        );
    }

    #[test]
    fn compiler_lowers_binding_match_patterns() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = 7;
    return match value {
        bound => bound + 1,
    };
}
"#,
            "main",
        )
        .expect("binding match patterns should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Move { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Add { .. }))
        );
    }

    #[test]
    fn compiler_lowers_match_guards() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = 7;
    return match value {
        bound if bound < 5 => 10,
        bound if bound == 7 => bound + 1,
        _ => 0,
    };
}
"#,
            "main",
        )
        .expect("match guards should compile");

        assert!(
            code.instructions
                .iter()
                .filter(|instruction| matches!(
                    instruction.kind,
                    InstructionKind::JumpIfFalse { .. }
                ))
                .count()
                >= 2
        );
        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Less { .. }))
        );
    }

    #[test]
    fn compiler_lowers_record_variant_field_patterns() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
enum Reward {
    Grant { kind, amount }
}

fn main() {
    let reward = Reward.Grant { kind: "xp", amount: 7 };
    return match reward {
        Reward.Grant { kind: "gold", amount } => amount,
        Reward.Grant { kind: "xp", amount } => amount + 1,
        _ => 0,
    };
}
"#,
            "main",
        )
        .expect("record variant field patterns should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Equal { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .filter(|instruction| {
                    matches!(instruction.kind, InstructionKind::GetEnumField { .. })
                })
                .count()
                >= 2
        );
    }

    #[test]
    fn compiler_lowers_tuple_variant_constructors_and_patterns() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
enum Damage {
    Physical(amount, bonus),
    Magical(amount),
}

fn main() {
    let damage = Damage.Physical(7, 2);
    return match damage {
        Damage.Physical(amount, bonus) => amount + bonus,
        _ => 0,
    };
}
"#,
            "main",
        )
        .expect("tuple variant constructor and pattern should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::MakeEnum { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .filter(|instruction| {
                    matches!(instruction.kind, InstructionKind::GetEnumField { .. })
                })
                .count()
                >= 2
        );
    }

    #[test]
    fn compiler_lowers_local_assignment_operators() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let value = 1;
    value += 4;
    value *= 3;
    value -= 5;
    value /= 2;
    value %= 5;
    let copy = (value = value + 10);
    return value + copy;
}
"#,
            "main",
        )
        .expect("local assignments should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Add { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Sub { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Mul { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Div { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Rem { .. }))
        );
    }

    #[test]
    fn compiler_lowers_index_reads() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let values = [2, 4, 8];
    let rewards = { "xp": 6 };
    return values[1] + rewards["xp"];
}
"#,
            "main",
        )
        .expect("index reads should compile");

        assert!(
            code.instructions
                .iter()
                .filter(|instruction| matches!(instruction.kind, InstructionKind::GetIndex { .. }))
                .count()
                >= 2
        );
    }

    #[test]
    fn compiler_keeps_call_result_index_reads_off_host_paths() {
        let code = compile_function_source_with_options(
            SourceId::new(1),
            r#"
fn values() {
    return [{ "name": "Damageable" }];
}

fn main() {
    return values()[0].name;
}
"#,
            "main",
            &CompilerOptions::new().with_host_field("count", FieldId::new(1)),
        )
        .expect("call result index read should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::GetIndex { .. }))
        );
        assert!(
            !code
                .instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::GetHostPath { .. }))
        );
    }

    #[test]
    fn compiler_lowers_index_writes() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let values = [2, 4, 8];
    values[1] = 10;
    values[2] += 5;
    return values[1] + values[2];
}
"#,
            "main",
        )
        .expect("index writes should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::SetIndex { .. }))
        );
    }

    #[test]
    fn compiler_lowers_record_field_writes() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let reward = Reward { item_id: "gold", count: 2 };
    reward.count += 3;
    reward.item_id = "xp";
    return reward.count;
}
"#,
            "main",
        )
        .expect("record field writes should compile");

        assert!(code.instructions.iter().any(|instruction| {
            matches!(instruction.kind, InstructionKind::SetRecordField { .. })
        }));
    }

    #[test]
    fn compiler_lowers_nested_record_field_writes() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let player = Player {
        stats: Stats {
            level: 2,
            exp: 5,
        },
    };
    player.stats.level += 3;
    player.stats.exp = player.stats.level + 1;
    return player.stats.level + player.stats.exp;
}
"#,
            "main",
        )
        .expect("nested record field writes should compile");

        assert!(
            code.instructions
                .iter()
                .filter(|instruction| {
                    matches!(instruction.kind, InstructionKind::SetRecordField { .. })
                })
                .count()
                >= 3
        );
    }

    #[test]
    fn compiler_lowers_indexed_record_field_writes() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let players = [
        Player { level: 2, exp: 5 },
        Player { level: 7, exp: 1 },
    ];
    players[0].level += 3;
    players[1].exp = players[0].level + 4;
    return players[0].level + players[1].exp;
}
"#,
            "main",
        )
        .expect("indexed record field writes should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::SetIndex { .. }))
        );
        assert!(code.instructions.iter().any(|instruction| {
            matches!(instruction.kind, InstructionKind::SetRecordField { .. })
        }));
    }

    #[test]
    fn compiler_lowers_immediate_record_field_reads_to_slots() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    return Reward { item_id: "gold", count: 2 }.count;
}
"#,
            "main",
        )
        .expect("immediate record field read should compile");

        assert!(code.instructions.iter().any(|instruction| {
            matches!(
                instruction.kind,
                InstructionKind::GetRecordSlot {
                    ref field,
                    slot: 0,
                    ..
                } if field == "count"
            )
        }));
    }

    #[test]
    fn compiler_lowers_immediate_enum_field_reads_to_slots() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    return Damage.Physical { amount: 7 }.amount;
}
"#,
            "main",
        )
        .expect("immediate enum field read should compile");

        assert!(code.instructions.iter().any(|instruction| {
            matches!(
                instruction.kind,
                InstructionKind::GetEnumSlot {
                    ref field,
                    slot: 0,
                    ..
                } if field == "amount"
            )
        }));
    }

    #[test]
    fn compiler_lowers_typed_enum_variant_field_reads_to_slots() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
enum Damage {
    Physical { amount: int, element: string },
    Magical { amount: int },
}

fn main() {
    let damage = Damage.Physical { amount: 7, element: "slash" };
    return damage.amount;
}
"#,
        )
        .expect("typed enum variant field read should compile to slot bytecode");
        let main = program.function("main").expect("main function");

        assert!(main.instructions.iter().any(|instruction| {
            matches!(
                instruction.kind,
                InstructionKind::GetEnumSlot {
                    ref field,
                    slot: 0,
                    ..
                } if field == "amount"
            )
        }));
        assert!(
            !main.instructions.iter().any(|instruction| matches!(
                instruction.kind,
                InstructionKind::GetEnumField { .. }
            ))
        );
    }

    #[test]
    fn compiler_lowers_typed_record_field_reads_to_slots() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
struct Reward {
    item_id: string,
    count: int,
}

fn make_reward() {
    return Reward { item_id: "gold", count: 2 };
}

fn main() {
    let reward: Reward = make_reward();
    return reward.count;
}
"#,
        )
        .expect("typed record field read should compile to slot bytecode");
        let main = program.function("main").expect("main function");

        assert!(main.instructions.iter().any(|instruction| {
            matches!(
                instruction.kind,
                InstructionKind::GetRecordSlot {
                    ref field,
                    slot: 0,
                    ..
                } if field == "count"
            )
        }));
    }

    #[test]
    fn compiler_lowers_typed_record_field_writes_to_slots() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
struct Reward {
    item_id: string,
    count: int,
}

fn make_reward() {
    return Reward { item_id: "gold", count: 2 };
}

fn main() {
    let reward: Reward = make_reward();
    reward.count += 3;
    reward.item_id = "xp";
    return reward.count;
}
"#,
        )
        .expect("typed record field writes should compile to slot bytecode");
        let main = program.function("main").expect("main function");

        assert!(main.instructions.iter().any(|instruction| {
            matches!(
                instruction.kind,
                InstructionKind::SetRecordSlot {
                    ref field,
                    slot: 0,
                    ..
                } if field == "count"
            )
        }));
        assert!(main.instructions.iter().any(|instruction| {
            matches!(
                instruction.kind,
                InstructionKind::SetRecordSlot {
                    ref field,
                    slot: 1,
                    ..
                } if field == "item_id"
            )
        }));
        assert!(
            !main.instructions.iter().any(|instruction| matches!(
                instruction.kind,
                InstructionKind::SetRecordField { .. }
            ))
        );
    }

    #[test]
    fn compiler_lowers_for_in_loops() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let total = 0;
    for value in [1, 2, 3] {
        total += value;
    }
    return total;
}
"#,
            "main",
        )
        .expect("for-in loop should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::IterInit { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::IterNext { .. }))
        );
    }

    #[test]
    fn compiler_lowers_for_in_patterns() {
        let program = compile_program_source(
            SourceId::new(1),
            r#"
enum Reward {
    Grant { amount },
    Skip { amount },
}

fn main() {
    let total = 0;
    let rewards = [
        Reward.Grant { amount: 2 },
        Reward.Skip { amount: 100 },
        Reward.Grant { amount: 5 },
    ];
    for Reward.Grant { amount } in rewards {
        total += amount;
    }
    return total;
}
"#,
        )
        .expect("for-in pattern should compile");
        let main = program.function("main").expect("main function");

        assert!(
            main.instructions.iter().any(|instruction| matches!(
                instruction.kind,
                InstructionKind::EnumTagEqual { .. }
            ))
        );
        assert!(main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            InstructionKind::GetEnumField { ref field, .. } if field == "amount"
        )));
    }

    #[test]
    fn compiler_lowers_break_and_continue() {
        let code = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    let total = 0;
    for value in [1, 2, 3, 4, 5] {
        if value == 2 {
            continue;
        }
        if value == 5 {
            break;
        }
        total += value;
    }
    return total;
}
"#,
            "main",
        )
        .expect("break and continue should compile");

        assert!(
            code.instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::IterNext { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .filter(|instruction| matches!(instruction.kind, InstructionKind::Jump { .. }))
                .count()
                >= 3
        );
    }

    #[test]
    fn compiler_rejects_break_and_continue_outside_loop() {
        let break_error = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    break;
}
"#,
            "main",
        )
        .expect_err("break outside loop should be rejected");
        assert_eq!(
            break_error.kind,
            CompileErrorKind::UnsupportedSyntax("break outside loop")
        );

        let continue_error = compile_function_source(
            SourceId::new(1),
            r#"
fn main() {
    continue;
}
"#,
            "main",
        )
        .expect_err("continue outside loop should be rejected");
        assert_eq!(
            continue_error.kind,
            CompileErrorKind::UnsupportedSyntax("continue outside loop")
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
