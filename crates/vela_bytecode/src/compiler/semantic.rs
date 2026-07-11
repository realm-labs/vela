use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_hir::ids::{HirDeclId, ModuleId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, ModulePath, ModuleSource};
use vela_hir::script_methods::{
    ScriptMethodCatalog, ScriptMethodCatalogError, ScriptMethodCatalogMode,
};
use vela_mir::{MirBuildError, MirEvaluatedConstant, MirSourceOrigin};
use vela_syntax::parse::parse_source_with_id;

use super::const_eval::evaluate_const_body;
use super::error::{CompileError, CompileErrorKind, CompileResult};
use super::schema_defaults::{EvaluatedSchemaDefaults, source_schema_defaults};

pub(super) struct SemanticSource {
    graph: ModuleGraph,
    module: ModuleId,
    script_methods: ScriptMethodCatalog,
}

pub(super) struct SemanticModules {
    graph: ModuleGraph,
    modules: Vec<ModuleId>,
    script_methods: ScriptMethodCatalog,
}

impl SemanticSource {
    pub(super) const fn script_metadata_graph(&self) -> &ModuleGraph {
        &self.graph
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

    pub(super) fn schema_defaults(
        &self,
        type_symbols: &BTreeMap<HirDeclId, String>,
        evaluated_constants: &BTreeMap<HirDeclId, MirEvaluatedConstant>,
    ) -> CompileResult<EvaluatedSchemaDefaults> {
        source_schema_defaults(&self.graph, self.module, type_symbols, evaluated_constants)
    }

    pub(super) fn evaluated_constants(
        &self,
    ) -> CompileResult<BTreeMap<HirDeclId, MirEvaluatedConstant>> {
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

    pub(super) const fn script_method_catalog(&self) -> &ScriptMethodCatalog {
        &self.script_methods
    }

    pub(super) fn function_declaration(&self, name: &str) -> Option<HirDeclId> {
        let declaration = self.graph.module(self.module)?.get(name)?;
        let metadata = self.graph.declaration(declaration)?;
        (metadata.kind == DeclarationKind::Function).then_some(declaration)
    }
}

impl SemanticModules {
    pub(super) const fn script_metadata_graph(&self) -> &ModuleGraph {
        &self.graph
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

    pub(super) fn schema_defaults(
        &self,
        type_symbols: &BTreeMap<HirDeclId, String>,
        evaluated_constants: &BTreeMap<HirDeclId, MirEvaluatedConstant>,
    ) -> CompileResult<EvaluatedSchemaDefaults> {
        let mut defaults = EvaluatedSchemaDefaults::default();
        for module in &self.modules {
            defaults.merge(source_schema_defaults(
                &self.graph,
                *module,
                type_symbols,
                evaluated_constants,
            )?);
        }
        Ok(defaults)
    }

    pub(super) fn evaluated_constants(
        &self,
    ) -> CompileResult<BTreeMap<HirDeclId, MirEvaluatedConstant>> {
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

    pub(super) const fn script_method_catalog(&self) -> &ScriptMethodCatalog {
        &self.script_methods
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
        let script_methods = ScriptMethodCatalog::from_graph(
            &graph,
            ScriptMethodCatalogMode::single_source(module, "main"),
        )
        .map_err(script_method_catalog_error)?;
        Ok(SemanticSource {
            graph,
            module,
            script_methods,
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
        let script_methods =
            ScriptMethodCatalog::from_graph(&graph, ScriptMethodCatalogMode::ModuleGraph)
                .map_err(script_method_catalog_error)?;
        Ok(SemanticModules {
            graph,
            modules,
            script_methods,
        })
    } else {
        Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
            graph.diagnostics().to_vec(),
        )))
    }
}

fn script_method_catalog_error(error: ScriptMethodCatalogError) -> CompileError {
    let span = error.origin().span;
    CompileError::new(CompileErrorKind::MirInput(Box::new(
        MirBuildError::InconsistentInput {
            origin: MirSourceOrigin::declaration(error.declaration(), span),
            message: error.to_string(),
        },
    )))
    .with_span(span)
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
