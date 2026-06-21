use std::collections::{BTreeMap, BTreeSet};

use vela_common::SourceId;
use vela_hir::binding::BindingMap;
use vela_hir::ids::{HirDeclId, ModuleId};
use vela_hir::module_graph::{
    DeclarationKind, ImportResolution, ModuleGraph, ModulePath, ModuleSource,
};
use vela_hir::type_hint::{FunctionSignature, ParamHint};
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::{FunctionItem, SourceFile, SyntaxSourceFile};
use vela_syntax::parse::parse_source_with_id as parse_syntax_source;
use vela_syntax::parser::parse_source as parse_legacy_source;

use crate::Constant;

use super::const_eval::evaluate_syntax_const_expr;
use super::error::{CompileError, CompileErrorKind, CompileResult};
use super::field_slots::ScriptFieldSlots;
use super::legacy_payloads::function_body_payloads;
use super::schema_defaults::{ScriptSchemaDefaults, source_schema_defaults};
use super::script_impls;
use super::syntax_payloads::{const_value_payloads, schema_default_payloads};

pub(super) struct SemanticSource {
    source: SourceId,
    text: String,
    syntax: SyntaxParse<SyntaxSourceFile>,
    parsed: SourceFile,
    graph: ModuleGraph,
    module: ModuleId,
}

pub(super) struct SemanticModules {
    syntax: BTreeMap<ModuleId, SyntaxParse<SyntaxSourceFile>>,
    parsed: BTreeMap<ModuleId, SourceFile>,
    source_ids: BTreeMap<ModuleId, SourceId>,
    graph: ModuleGraph,
    modules: Vec<ModuleId>,
}

impl SemanticSource {
    pub(super) fn script_metadata_graph(&self) -> ModuleGraph {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            self.source,
            ModulePath::new(Vec::<String>::new()),
            self.text.clone(),
        ));
        graph.resolve_imports();
        graph
    }

    pub(super) fn function(
        &self,
        name: &str,
    ) -> Option<(&FunctionItem, &FunctionSignature, &BindingMap)> {
        let declaration = self.function_declaration(name)?;
        let metadata = self.graph.declaration(declaration)?;
        let signature = self.graph.function_signature(declaration)?;
        let bindings = self.graph.bindings(declaration)?;
        let payloads = function_body_payloads(&self.parsed);
        let function = payloads.get(metadata.name.as_str()).copied()?;
        Some((function, signature, bindings))
    }

    pub(super) fn script_function_names(&self) -> BTreeSet<String> {
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

    pub(super) fn script_function_symbols(&self) -> BTreeMap<HirDeclId, String> {
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

    pub(super) fn script_function_signatures(&self) -> BTreeMap<HirDeclId, Vec<ParamHint>> {
        self.script_function_symbols()
            .keys()
            .filter_map(|declaration| {
                self.graph
                    .function_signature(*declaration)
                    .map(|signature| (*declaration, signature.params.clone()))
            })
            .collect()
    }

    pub(super) fn global_symbols(&self) -> BTreeMap<HirDeclId, String> {
        let Some(declarations) = self.graph.module(self.module) else {
            return BTreeMap::new();
        };
        declarations
            .names()
            .filter_map(|name| {
                let declaration = declarations.get(name)?;
                let metadata = self.graph.declaration(declaration)?;
                (metadata.kind == DeclarationKind::Global)
                    .then(|| (declaration, format!("main::{}", metadata.name)))
            })
            .collect()
    }

    pub(super) fn global_type_symbols(&self) -> BTreeMap<HirDeclId, String> {
        self.global_symbols()
            .keys()
            .filter_map(|declaration| {
                self.graph
                    .global_metadata(*declaration)
                    .map(|metadata| (*declaration, metadata.type_hint.display()))
            })
            .collect()
    }

    pub(super) fn type_symbols(&self) -> BTreeMap<HirDeclId, String> {
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

    pub(super) fn script_field_slots(
        &self,
        type_symbols: &BTreeMap<HirDeclId, String>,
    ) -> ScriptFieldSlots {
        ScriptFieldSlots::from_graph(&self.graph, type_symbols)
    }

    pub(super) fn schema_defaults(
        &self,
        type_symbols: &BTreeMap<HirDeclId, String>,
        const_values: &BTreeMap<HirDeclId, Constant>,
    ) -> ScriptSchemaDefaults {
        source_schema_defaults(
            &schema_default_payloads(self.source, &self.syntax, &self.parsed),
            &self.graph,
            self.module,
            type_symbols,
            self.const_values_by_name(const_values),
        )
    }

    pub(super) fn const_values(&self) -> CompileResult<BTreeMap<HirDeclId, Constant>> {
        let mut values_by_declaration = BTreeMap::new();
        let mut values_by_name = BTreeMap::new();
        let payloads = const_value_payloads(&self.syntax);
        for (declaration, name) in module_const_declarations(&self.graph, self.module) {
            let Some(expr) = payloads.get(&name) else {
                continue;
            };
            if let Some(value) = evaluate_syntax_const_expr(self.source, expr, &values_by_name)? {
                values_by_declaration.insert(declaration, value.clone());
                values_by_name.insert(name, value);
            }
        }
        Ok(values_by_declaration)
    }

    pub(super) fn script_impl_methods(&self) -> Vec<script_impls::ScriptImplMethod<'_>> {
        script_impls::source_methods(&self.parsed, &self.graph, self.module)
    }

    fn const_values_by_name(
        &self,
        const_values: &BTreeMap<HirDeclId, Constant>,
    ) -> BTreeMap<String, Constant> {
        let mut values = BTreeMap::new();
        for (declaration, name) in module_const_declarations(&self.graph, self.module) {
            let Some(value) = const_values.get(&declaration).cloned() else {
                continue;
            };
            values.insert(name, value);
        }
        values
    }

    fn function_declaration(&self, name: &str) -> Option<HirDeclId> {
        let declaration = self.graph.module(self.module)?.get(name)?;
        let metadata = self.graph.declaration(declaration)?;
        (metadata.kind == DeclarationKind::Function).then_some(declaration)
    }
}

impl SemanticModules {
    pub(super) fn script_metadata_graph(&self) -> ModuleGraph {
        self.graph.clone()
    }

    pub(super) fn function(
        &self,
        declaration: HirDeclId,
    ) -> Option<(&FunctionItem, &FunctionSignature, &BindingMap)> {
        let metadata = self.graph.declaration(declaration)?;
        let signature = self.graph.function_signature(declaration)?;
        let bindings = self.graph.bindings(declaration)?;
        let parsed = self.parsed.get(&metadata.module)?;
        let payloads = function_body_payloads(parsed);
        let function = payloads.get(metadata.name.as_str()).copied()?;
        Some((function, signature, bindings))
    }

    pub(super) fn script_function_declarations(&self) -> BTreeSet<HirDeclId> {
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

    pub(super) fn script_function_symbols(&self) -> BTreeMap<HirDeclId, String> {
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
                        .then(|| (declaration, format!("{path}::{}", metadata.name)))
                })
            })
            .collect()
    }

    pub(super) fn script_function_signatures(&self) -> BTreeMap<HirDeclId, Vec<ParamHint>> {
        self.script_function_symbols()
            .keys()
            .filter_map(|declaration| {
                self.graph
                    .function_signature(*declaration)
                    .map(|signature| (*declaration, signature.params.clone()))
            })
            .collect()
    }

    pub(super) fn global_symbols(&self) -> BTreeMap<HirDeclId, String> {
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
                    (metadata.kind == DeclarationKind::Global)
                        .then(|| (declaration, format!("{path}::{}", metadata.name)))
                })
            })
            .collect()
    }

    pub(super) fn global_type_symbols(&self) -> BTreeMap<HirDeclId, String> {
        self.global_symbols()
            .keys()
            .filter_map(|declaration| {
                self.graph
                    .global_metadata(*declaration)
                    .map(|metadata| (*declaration, metadata.type_hint.display()))
            })
            .collect()
    }

    pub(super) fn type_symbols(&self) -> BTreeMap<HirDeclId, String> {
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
                    .then(|| (declaration, format!("{path}::{}", metadata.name)))
                })
            })
            .collect()
    }

    pub(super) fn script_field_slots(
        &self,
        type_symbols: &BTreeMap<HirDeclId, String>,
    ) -> ScriptFieldSlots {
        ScriptFieldSlots::from_graph(&self.graph, type_symbols)
    }

    pub(super) fn schema_defaults(
        &self,
        type_symbols: &BTreeMap<HirDeclId, String>,
        const_values: &BTreeMap<HirDeclId, Constant>,
    ) -> ScriptSchemaDefaults {
        let mut defaults = ScriptSchemaDefaults::default();
        for module in &self.modules {
            let Some(syntax) = self.syntax.get(module) else {
                continue;
            };
            let Some(parsed) = self.parsed.get(module) else {
                continue;
            };
            let Some(source) = self.source_ids.get(module).copied() else {
                continue;
            };
            defaults.merge(source_schema_defaults(
                &schema_default_payloads(source, syntax, parsed),
                &self.graph,
                *module,
                type_symbols,
                self.const_values_by_name(*module, const_values),
            ));
        }
        defaults
    }

    pub(super) fn const_values(&self) -> CompileResult<BTreeMap<HirDeclId, Constant>> {
        let mut values_by_declaration = BTreeMap::new();
        loop {
            let mut progressed = false;
            for module in &self.modules {
                let mut previous_values = BTreeMap::new();
                let Some(parsed) = self.syntax.get(module) else {
                    continue;
                };
                let Some(source) = self.source_ids.get(module).copied() else {
                    continue;
                };
                let payloads = const_value_payloads(parsed);
                for (declaration, name) in module_const_declarations(&self.graph, *module) {
                    if let Some(value) = values_by_declaration.get(&declaration).cloned() {
                        previous_values.insert(name.clone(), value);
                        continue;
                    }

                    let Some(expr) = payloads.get(&name) else {
                        continue;
                    };

                    let mut values_by_name =
                        self.imported_const_values(*module, &values_by_declaration);
                    values_by_name.extend(previous_values.clone());
                    if let Some(value) = evaluate_syntax_const_expr(source, expr, &values_by_name)?
                    {
                        values_by_declaration.insert(declaration, value.clone());
                        previous_values.insert(name, value);
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

    pub(super) fn script_impl_methods(&self) -> Vec<script_impls::ScriptImplMethod<'_>> {
        script_impls::module_methods(&self.parsed, &self.graph)
    }

    fn const_values_by_name(
        &self,
        module: ModuleId,
        const_values: &BTreeMap<HirDeclId, Constant>,
    ) -> BTreeMap<String, Constant> {
        let mut values = self.imported_const_values(module, const_values);
        for (declaration, name) in module_const_declarations(&self.graph, module) {
            let Some(value) = const_values.get(&declaration).cloned() else {
                continue;
            };
            values.insert(name, value);
        }
        values
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

fn module_const_declarations(graph: &ModuleGraph, module: ModuleId) -> Vec<(HirDeclId, String)> {
    let Some(declarations) = graph.module(module) else {
        return Vec::new();
    };
    let mut consts = declarations
        .names()
        .filter_map(|name| {
            let declaration = declarations.get(name)?;
            let metadata = graph.declaration(declaration)?;
            (metadata.kind == DeclarationKind::Const).then(|| (declaration, metadata.name.clone()))
        })
        .collect::<Vec<_>>();
    consts.sort_by_key(|(declaration, _)| *declaration);
    consts
}

pub(super) fn parse_semantic_source(source: SourceId, text: &str) -> CompileResult<SemanticSource> {
    let syntax = parse_syntax_source(source, text);
    if !syntax.diagnostics().is_empty() {
        return Err(CompileError::new(CompileErrorKind::SyntaxDiagnostics(
            syntax.diagnostics().to_vec(),
        )));
    }
    let parsed = parse_legacy_source(source, text);
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(ModuleSource::new(
        source,
        ModulePath::from_qualified("main"),
        text.to_owned(),
    ));
    graph.resolve_imports();
    if graph.diagnostics().is_empty() {
        Ok(SemanticSource {
            source,
            text: text.to_owned(),
            syntax,
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

pub(super) fn parse_semantic_modules(sources: &[ModuleSource]) -> CompileResult<SemanticModules> {
    let syntax_sources = sources
        .iter()
        .map(|source| (source, parse_syntax_source(source.id, &source.text)))
        .collect::<Vec<_>>();
    let syntax_diagnostics = syntax_sources
        .iter()
        .flat_map(|(_, parsed)| parsed.diagnostics().iter().cloned())
        .collect::<Vec<_>>();
    if !syntax_diagnostics.is_empty() {
        return Err(CompileError::new(CompileErrorKind::SyntaxDiagnostics(
            syntax_diagnostics,
        )));
    }

    let mut syntax = BTreeMap::new();
    let mut parsed = BTreeMap::new();
    let mut source_ids = BTreeMap::new();
    let mut graph = ModuleGraph::new();
    let mut modules = Vec::new();

    for (source, syntax_file) in syntax_sources {
        let module = graph.add_source(source.clone());
        let source_file = parse_legacy_source(source.id, &source.text);
        syntax.insert(module, syntax_file);
        parsed.insert(module, source_file);
        source_ids.insert(module, source.id);
        modules.push(module);
    }

    graph.resolve_imports();
    if graph.diagnostics().is_empty() {
        Ok(SemanticModules {
            syntax,
            parsed,
            source_ids,
            graph,
            modules,
        })
    } else {
        Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
            graph.diagnostics().to_vec(),
        )))
    }
}
