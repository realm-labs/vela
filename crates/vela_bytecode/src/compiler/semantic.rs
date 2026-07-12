use std::collections::BTreeMap;

use vela_hir::ids::{HirDeclId, ModuleId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::script_methods::{
    ScriptMethodCatalog, ScriptMethodCatalogError, ScriptMethodCatalogMode,
};
use vela_hir::source_ingestion::HirSourceSet;
use vela_mir::{MirBuildError, MirEvaluatedConstant, MirSourceOrigin};

use super::ProgramCompilationKind;
use super::const_eval::evaluate_const_body;
use super::error::{CompileError, CompileErrorKind, CompileResult};
use super::schema_defaults::{EvaluatedSchemaDefaults, source_schema_defaults};

pub(super) struct SemanticCompilation<'a> {
    graph: &'a ModuleGraph,
    modules: &'a [ModuleId],
    kind: ProgramCompilationKind,
    script_methods: ScriptMethodCatalog,
}

impl<'a> SemanticCompilation<'a> {
    pub(super) fn new(
        sources: &'a HirSourceSet,
        kind: ProgramCompilationKind,
    ) -> CompileResult<Self> {
        let graph = sources.graph();
        let modules = sources.modules();
        let catalog_mode = match kind {
            ProgramCompilationKind::SingleSource => {
                ScriptMethodCatalogMode::single_source(modules[0], "main")
            }
            ProgramCompilationKind::ModuleGraph => ScriptMethodCatalogMode::ModuleGraph,
        };
        let script_methods = ScriptMethodCatalog::from_graph(graph, catalog_mode)
            .map_err(script_method_catalog_error)?;
        Ok(Self {
            graph,
            modules,
            kind,
            script_methods,
        })
    }

    #[cfg(test)]
    pub(super) const fn graph(&self) -> &ModuleGraph {
        self.graph
    }

    pub(super) fn function_symbols(&self) -> BTreeMap<HirDeclId, String> {
        self.symbols(DeclarationKind::Function, false)
    }

    pub(super) fn global_symbols(&self) -> BTreeMap<HirDeclId, String> {
        match self.kind {
            ProgramCompilationKind::SingleSource => self.symbols(DeclarationKind::Global, true),
            ProgramCompilationKind::ModuleGraph => self.symbols(DeclarationKind::Global, false),
        }
    }

    pub(super) fn type_symbols(&self) -> BTreeMap<HirDeclId, String> {
        self.modules()
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
                    .then(|| {
                        let symbol = match self.kind {
                            ProgramCompilationKind::SingleSource => name.to_owned(),
                            ProgramCompilationKind::ModuleGraph => {
                                format!("{path}::{}", metadata.name)
                            }
                        };
                        (declaration, symbol)
                    })
                })
            })
            .collect()
    }

    pub(super) fn evaluated_constants(
        &self,
    ) -> CompileResult<BTreeMap<HirDeclId, MirEvaluatedConstant>> {
        let mut values_by_declaration = BTreeMap::new();
        if self.kind == ProgramCompilationKind::SingleSource {
            self.evaluate_module_constants(self.modules[0], &mut values_by_declaration)?;
            return Ok(values_by_declaration);
        }
        loop {
            let mut progressed = false;
            for module in self.modules() {
                progressed |=
                    self.evaluate_module_constants(*module, &mut values_by_declaration)?;
            }
            if !progressed {
                break;
            }
        }
        Ok(values_by_declaration)
    }

    fn evaluate_module_constants(
        &self,
        module: ModuleId,
        values_by_declaration: &mut BTreeMap<HirDeclId, MirEvaluatedConstant>,
    ) -> CompileResult<bool> {
        let mut progressed = false;
        for declaration in module_const_declarations(self.graph, module) {
            if values_by_declaration.contains_key(&declaration) {
                continue;
            }
            let Some(body) = self.graph.const_initializer_body(declaration) else {
                continue;
            };
            let Some(bindings) = self.graph.const_initializer_bindings(declaration) else {
                continue;
            };
            if let Some(value) = evaluate_const_body(body, bindings, values_by_declaration)? {
                values_by_declaration.insert(declaration, value);
                progressed = true;
            }
        }
        Ok(progressed)
    }

    pub(super) fn schema_defaults(
        &self,
        type_symbols: &BTreeMap<HirDeclId, String>,
        evaluated_constants: &BTreeMap<HirDeclId, MirEvaluatedConstant>,
    ) -> CompileResult<EvaluatedSchemaDefaults> {
        let mut defaults = EvaluatedSchemaDefaults::default();
        for module in self.modules() {
            defaults.merge(source_schema_defaults(
                self.graph,
                *module,
                type_symbols,
                evaluated_constants,
            )?);
        }
        Ok(defaults)
    }

    pub(super) const fn script_method_catalog(&self) -> &ScriptMethodCatalog {
        &self.script_methods
    }

    fn modules(&self) -> &[ModuleId] {
        self.modules
    }

    fn symbols(
        &self,
        kind: DeclarationKind,
        single_source_main_prefix: bool,
    ) -> BTreeMap<HirDeclId, String> {
        self.modules()
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
                    (metadata.kind == kind).then(|| {
                        let symbol = match self.kind {
                            ProgramCompilationKind::SingleSource if single_source_main_prefix => {
                                format!("main::{}", metadata.name)
                            }
                            ProgramCompilationKind::SingleSource => name.to_owned(),
                            ProgramCompilationKind::ModuleGraph => {
                                format!("{path}::{}", metadata.name)
                            }
                        };
                        (declaration, symbol)
                    })
                })
            })
            .collect()
    }
}

fn module_const_declarations(graph: &ModuleGraph, module: ModuleId) -> Vec<HirDeclId> {
    let Some(declarations) = graph.module(module) else {
        return Vec::new();
    };
    let mut consts = declarations
        .names()
        .filter_map(|name| {
            let declaration = declarations.get(name)?;
            let metadata = graph.declaration(declaration)?;
            (metadata.kind == DeclarationKind::Const).then_some(declaration)
        })
        .collect::<Vec<_>>();
    consts.sort_unstable();
    consts
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
