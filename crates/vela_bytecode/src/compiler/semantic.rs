use std::collections::{BTreeMap, BTreeSet};

use vela_common::SourceId;
use vela_hir::binding::BindingMap;
use vela_hir::body::HirBody;
use vela_hir::ids::{HirDeclId, ModuleId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, ModulePath, ModuleSource};
use vela_hir::type_hint::{FunctionSignature, ParamHint};
use vela_syntax::parse::parse_source_with_id;

use crate::Constant;

use super::const_eval::evaluate_const_body;
use super::error::{CompileError, CompileErrorKind, CompileResult};
use super::field_slots::ScriptFieldSlots;
use super::function_inputs::FunctionCompileInput;
use super::param_defaults::param_default_values;
use super::schema_defaults::{ScriptSchemaDefaults, source_schema_defaults};
use super::script_impls;

pub(super) struct SemanticSource {
    graph: ModuleGraph,
    module: ModuleId,
}

pub(super) struct SemanticModules {
    graph: ModuleGraph,
    modules: Vec<ModuleId>,
}

impl SemanticSource {
    pub(super) const fn script_metadata_graph(&self) -> &ModuleGraph {
        &self.graph
    }

    pub(super) fn function(
        &self,
        name: &str,
    ) -> Option<(
        FunctionCompileInput,
        &FunctionSignature,
        &BindingMap,
        Vec<&HirBody>,
    )> {
        let declaration = self.function_declaration(name)?;
        let signature = self.graph.function_signature(declaration)?;
        let bindings = self.graph.bindings(declaration)?;
        let hir_body = self.graph.function_body(declaration)?;
        let input = function_compile_input(&self.graph, hir_body, signature)?;
        Some((input, signature, bindings, self.graph.bodies().collect()))
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
    ) -> CompileResult<ScriptSchemaDefaults> {
        source_schema_defaults(&self.graph, self.module, type_symbols, const_values)
    }

    pub(super) fn const_values(&self) -> CompileResult<BTreeMap<HirDeclId, Constant>> {
        let mut values_by_declaration = BTreeMap::new();
        for (declaration, _) in module_const_declarations(&self.graph, self.module) {
            let Some(body) = self.graph.const_initializer_body(declaration) else {
                continue;
            };
            let Some(bindings) = self.graph.const_initializer_bindings(declaration) else {
                continue;
            };
            if let Some(value) = evaluate_const_body(body, bindings, &values_by_declaration)? {
                values_by_declaration.insert(declaration, value);
            }
        }
        Ok(values_by_declaration)
    }

    pub(super) fn script_impl_methods(&self) -> Vec<script_impls::ScriptImplMethod<'_>> {
        script_impls::source_methods(&self.graph, self.module)
    }

    fn function_declaration(&self, name: &str) -> Option<HirDeclId> {
        let declaration = self.graph.module(self.module)?.get(name)?;
        let metadata = self.graph.declaration(declaration)?;
        (metadata.kind == DeclarationKind::Function).then_some(declaration)
    }
}

impl SemanticModules {
    pub(super) const fn script_metadata_graph(&self) -> &ModuleGraph {
        &self.graph
    }

    pub(super) fn function(
        &self,
        declaration: HirDeclId,
    ) -> Option<(
        FunctionCompileInput,
        &FunctionSignature,
        &BindingMap,
        Vec<&HirBody>,
    )> {
        let signature = self.graph.function_signature(declaration)?;
        let bindings = self.graph.bindings(declaration)?;
        let hir_body = self.graph.function_body(declaration)?;
        let input = function_compile_input(&self.graph, hir_body, signature)?;
        Some((input, signature, bindings, self.graph.bodies().collect()))
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
    ) -> CompileResult<ScriptSchemaDefaults> {
        let mut defaults = ScriptSchemaDefaults::default();
        for module in &self.modules {
            defaults.merge(source_schema_defaults(
                &self.graph,
                *module,
                type_symbols,
                const_values,
            )?);
        }
        Ok(defaults)
    }

    pub(super) fn const_values(&self) -> CompileResult<BTreeMap<HirDeclId, Constant>> {
        let mut values_by_declaration = BTreeMap::new();
        loop {
            let mut progressed = false;
            for module in &self.modules {
                for (declaration, _) in module_const_declarations(&self.graph, *module) {
                    if values_by_declaration.contains_key(&declaration) {
                        continue;
                    }
                    let Some(body) = self.graph.const_initializer_body(declaration) else {
                        continue;
                    };
                    let Some(bindings) = self.graph.const_initializer_bindings(declaration) else {
                        continue;
                    };
                    if let Some(value) =
                        evaluate_const_body(body, bindings, &values_by_declaration)?
                    {
                        values_by_declaration.insert(declaration, value);
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
        script_impls::module_methods(&self.graph)
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

fn function_compile_input(
    graph: &ModuleGraph,
    hir_body: &HirBody,
    signature: &FunctionSignature,
) -> Option<FunctionCompileInput> {
    let param_defaults = param_default_values(hir_body, signature);
    Some(FunctionCompileInput {
        name: function_name_for_body(hir_body, graph)?,
        body: hir_body.id,
        param_defaults,
    })
}

fn function_name_for_body(hir_body: &HirBody, graph: &ModuleGraph) -> Option<String> {
    let vela_hir::body::HirBodyOwner::Declaration(declaration) = hir_body.owner else {
        return None;
    };
    graph
        .declaration(declaration)
        .map(|metadata| metadata.name.clone())
}

pub(super) fn parse_semantic_source(source: SourceId, text: &str) -> CompileResult<SemanticSource> {
    let syntax = parse_source_with_id(source, text);
    if !syntax.diagnostics().is_empty() {
        return Err(CompileError::new(CompileErrorKind::SyntaxDiagnostics(
            syntax.diagnostics().to_vec(),
        )));
    }
    let mut graph = ModuleGraph::new();
    let module = graph.add_parsed_source(
        ModuleSource::new(
            source,
            ModulePath::new(Vec::<String>::new()),
            text.to_owned(),
        ),
        &syntax,
    );
    graph.resolve_imports();
    if graph.diagnostics().is_empty() {
        Ok(SemanticSource { graph, module })
    } else {
        Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
            graph.diagnostics().to_vec(),
        )))
    }
}

pub(super) fn parse_semantic_modules(sources: &[ModuleSource]) -> CompileResult<SemanticModules> {
    let syntax_sources = sources
        .iter()
        .map(|source| (source, parse_source_with_id(source.id, &source.text)))
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

    let mut graph = ModuleGraph::new();
    let mut modules = Vec::new();

    for (source, parsed) in syntax_sources {
        let module = graph.add_parsed_source(source.clone(), &parsed);
        modules.push(module);
    }

    graph.resolve_imports();
    if graph.diagnostics().is_empty() {
        Ok(SemanticModules { graph, modules })
    } else {
        Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
            graph.diagnostics().to_vec(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_source_metadata_reuses_the_authoritative_semantic_graph() {
        let semantic = parse_semantic_source(
            SourceId::new(23),
            r#"
struct Counter {
    value: i64,
}

fn increment(counter: Counter) {
    return counter.value + 1;
}
"#,
        )
        .expect("source should produce semantic HIR");

        let metadata = semantic.script_metadata_graph();
        assert_eq!(metadata, &semantic.graph);
        assert_eq!(
            metadata.module_path(semantic.module),
            Some(&ModulePath::new(Vec::<String>::new())),
        );

        let semantic_declaration = semantic
            .graph
            .module(semantic.module)
            .and_then(|declarations| declarations.get("increment"))
            .expect("semantic graph should contain increment");
        let metadata_declaration = metadata
            .module(semantic.module)
            .and_then(|declarations| declarations.get("increment"))
            .expect("metadata graph should contain increment");
        assert_eq!(metadata_declaration, semantic_declaration);
        assert_eq!(
            metadata
                .function_body(metadata_declaration)
                .expect("metadata function body")
                .id,
            semantic
                .graph
                .function_body(semantic_declaration)
                .expect("semantic function body")
                .id,
        );
    }
}
