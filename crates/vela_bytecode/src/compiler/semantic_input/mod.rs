//! Production semantic input for MIR construction.
//!
//! Phase 0 builds and validates this immutable generation beside the current
//! direct bytecode emitter. It deliberately contains no MIR body construction
//! and no backend selection: the value is dropped after validation until the
//! atomic backend checkpoint.

mod contracts;
mod external;
mod lambdas;
mod logical_records;
mod placements;
mod schema;
mod try_targets;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use vela_analysis::executable::{
    ExecutableAnalysisError, ExecutableAnalysisGeneration, ExecutableAnalysisInput,
    ExecutableAnalysisView, ExecutableReceiverInput,
};
use vela_analysis::literals::LiteralPrimitiveContext;
use vela_analysis::registry::RegistryFacts;
use vela_analysis::semantic_facts::ScriptTypeTargetFact;
use vela_analysis::type_fact::TypeFact;
use vela_common::Diagnostic;
use vela_def::{FieldId, FunctionId, MethodId, TypeId, VariantId, script_function_id};
use vela_hir::body::HirBody;
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirNodeId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::script_methods::{ScriptMethod, ScriptMethodCatalog};
use vela_mir::{
    CompileTargetSnapshot, CompileTargetSnapshotBuilder, MethodExecutableTarget, MirBuildError,
    MirEvaluatedConstant, MirLoweringConfig, MirLoweringInput, MirSourceOrigin, MirTypeContract,
};
use vela_registry::RegistryCompileView;

use super::error::{CompileError, CompileErrorKind, CompileResult};
use super::options::CompilerOptions;
use super::schema_defaults::EvaluatedSchemaDefaults;

use self::contracts::ContractBoundary;
use self::external::ExternalCatalog;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SemanticRoots {
    Function(HirDeclId),
    Program,
}

#[derive(Clone, Copy)]
pub(super) struct SemanticInputRequest<'graph, 'methods, 'registry> {
    pub(super) graph: &'graph ModuleGraph,
    pub(super) roots: SemanticRoots,
    pub(super) script_function_symbols: &'graph BTreeMap<HirDeclId, String>,
    pub(super) script_methods: &'methods ScriptMethodCatalog,
    pub(super) type_symbols: &'graph BTreeMap<HirDeclId, String>,
    pub(super) state_symbols: &'graph BTreeMap<HirDeclId, String>,
    pub(super) evaluated_constants: &'graph BTreeMap<HirDeclId, MirEvaluatedConstant>,
    pub(super) schema_defaults: &'graph EvaluatedSchemaDefaults,
    pub(super) options: &'graph CompilerOptions,
    pub(super) registry: Option<RegistryCompileView<'registry>>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PreparedSemanticInput {
    analysis: ExecutableAnalysisGeneration,
    targets: CompileTargetSnapshot,
}

impl PreparedSemanticInput {
    pub(super) fn script_method_target(
        &self,
        node: HirNodeId,
        method: MethodId,
        function: FunctionId,
    ) -> Option<MethodExecutableTarget> {
        self.targets
            .methods_for_node(node)
            .iter()
            .copied()
            .find(|target| target.method == method && target.function == function)
    }

    pub(super) fn lowering_inputs<'a>(
        &'a self,
        graph: &'a ModuleGraph,
        config: MirLoweringConfig,
    ) -> CompileResult<Vec<MirLoweringInput<'a>>> {
        self.targets
            .compilation_roots()
            .map(|(function, root)| {
                let analysis = self.analysis.view(function).ok_or_else(|| {
                    input_error(MirBuildError::InconsistentInput {
                        origin: root.origin,
                        message: format!(
                            "missing executable analysis generation for function #{}",
                            function.get()
                        ),
                    })
                })?;
                MirLoweringInput::new(
                    graph,
                    root.identity,
                    root.body,
                    analysis,
                    &self.targets,
                    config,
                )
                .map_err(input_error)
            })
            .collect()
    }

    #[cfg(test)]
    pub(super) const fn analysis(&self) -> &ExecutableAnalysisGeneration {
        &self.analysis
    }

    pub(super) const fn targets(&self) -> &CompileTargetSnapshot {
        &self.targets
    }
}

pub(super) fn prepare_semantic_input(
    request: SemanticInputRequest<'_, '_, '_>,
) -> CompileResult<PreparedSemanticInput> {
    let combined = external::combined_registry(request.registry)?;
    let declaration_slots = combined
        .compile_view()
        .declaration_slots()
        .map_err(|error| {
            CompileError::new(CompileErrorKind::RegistrySnapshot(error.to_string()))
        })?;
    let mut catalog = ExternalCatalog::from_view(combined.compile_view(), &declaration_slots)
        .map_err(|error| {
            CompileError::new(CompileErrorKind::RegistrySnapshot(error.to_string()))
        })?;
    if request.registry.is_none() {
        catalog.include_policy_neutral_reflection_manifest();
    }
    let try_layouts = try_targets::TryLayouts::from_catalog(&catalog)?;
    let mut registry_facts =
        RegistryFacts::from_compile_view_with_slots(combined.compile_view(), declaration_slots)
            .map_err(|error| {
                CompileError::new(CompileErrorKind::RegistrySnapshot(error.to_string()))
            })?;
    if request.registry.is_none() {
        external::include_policy_neutral_reflection_signatures(&mut registry_facts);
    }
    external::apply_option_index_capabilities(&mut registry_facts, request.options, &catalog);
    let mut builder = GenerationBuilder::new(request, registry_facts, catalog, try_layouts);
    builder.insert_script_schema()?;
    builder.insert_script_callables()?;
    builder.insert_lambda_targets()?;
    builder.insert_compile_time_values()?;
    builder.rebuild_executable_analysis(&BTreeMap::new())?;
    builder.reject_literal_diagnostics()?;
    builder.reject_analysis_validation_diagnostics()?;
    let mut probe = builder.clone();
    probe.insert_placements()?;
    let literal_contexts = probe.literal_contexts()?;
    builder.boundaries = probe.boundaries;
    builder.rebuild_executable_analysis(&literal_contexts)?;
    builder.reject_literal_diagnostics()?;
    builder.reject_analysis_validation_diagnostics()?;
    builder.insert_placements()?;
    builder.finish()
}

#[derive(Clone)]
pub(super) struct GenerationBuilder<'graph, 'methods> {
    request: SemanticInputRequest<'graph, 'methods, 'static>,
    // The request never reads the registry view after the owned catalog/facts
    // are created. Erasing that borrow keeps the generation self-contained.
    registry_was_provided: bool,
    registry_facts: RegistryFacts,
    catalog: ExternalCatalog,
    executable_analysis: ExecutableAnalysisGeneration,
    targets: CompileTargetSnapshotBuilder,
    function_ids: BTreeMap<HirDeclId, FunctionId>,
    state_initializer_ids: BTreeMap<HirDeclId, FunctionId>,
    type_ids: BTreeMap<HirDeclId, TypeId>,
    type_names: BTreeMap<TypeId, String>,
    type_shapes: BTreeMap<TypeId, vela_common::ShapeId>,
    variant_ids: BTreeMap<(HirDeclId, String), VariantId>,
    field_ids: BTreeMap<(HirDeclId, Option<String>, String), FieldId>,
    method_targets: BTreeMap<(HirNodeId, TypeId), MethodExecutableTarget>,
    function_code_symbols: BTreeMap<FunctionId, String>,
    function_return_contracts: BTreeMap<FunctionId, Option<MirTypeContract>>,
    try_layouts: try_targets::TryLayouts,
    inserted_external_functions: BTreeSet<FunctionId>,
    inserted_external_methods: BTreeSet<(TypeId, MethodId)>,
    inserted_external_types: BTreeSet<TypeId>,
    inserted_external_variants: BTreeSet<VariantId>,
    inserted_external_fields: BTreeSet<FieldId>,
    inserted_logical_records: BTreeSet<vela_analysis::logical_records::LogicalRecordKind>,
    boundaries: Vec<ContractBoundary>,
    contract_edges: Vec<(MirTypeContract, MirSourceOrigin)>,
    diagnostics: Vec<Diagnostic>,
}

impl<'graph, 'methods> GenerationBuilder<'graph, 'methods> {
    fn new<'registry>(
        request: SemanticInputRequest<'graph, 'methods, 'registry>,
        registry_facts: RegistryFacts,
        catalog: ExternalCatalog,
        try_layouts: try_targets::TryLayouts,
    ) -> Self {
        let registry_was_provided = request.registry.is_some();
        // All registry data used below has already been copied into catalog and
        // RegistryFacts, so retain only the authoritative graph-side inputs.
        let request = SemanticInputRequest {
            graph: request.graph,
            roots: request.roots,
            script_function_symbols: request.script_function_symbols,
            script_methods: request.script_methods,
            type_symbols: request.type_symbols,
            state_symbols: request.state_symbols,
            evaluated_constants: request.evaluated_constants,
            schema_defaults: request.schema_defaults,
            options: request.options,
            registry: None,
        };
        Self {
            request,
            registry_was_provided,
            registry_facts,
            catalog,
            executable_analysis: ExecutableAnalysisGeneration::default(),
            targets: CompileTargetSnapshot::builder(),
            function_ids: BTreeMap::new(),
            state_initializer_ids: BTreeMap::new(),
            type_ids: BTreeMap::new(),
            type_names: BTreeMap::new(),
            type_shapes: BTreeMap::new(),
            variant_ids: BTreeMap::new(),
            field_ids: BTreeMap::new(),
            method_targets: BTreeMap::new(),
            function_code_symbols: BTreeMap::new(),
            function_return_contracts: BTreeMap::new(),
            try_layouts,
            inserted_external_functions: BTreeSet::new(),
            inserted_external_methods: BTreeSet::new(),
            inserted_external_types: BTreeSet::new(),
            inserted_external_variants: BTreeSet::new(),
            inserted_external_fields: BTreeSet::new(),
            inserted_logical_records: BTreeSet::new(),
            boundaries: Vec::new(),
            contract_edges: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn insert_compile_time_values(&mut self) -> CompileResult<()> {
        for (declaration, value) in self.request.evaluated_constants {
            let Some(body) = self.request.graph.const_initializer_body(*declaration) else {
                continue;
            };
            self.targets
                .insert_evaluated_constant(
                    *declaration,
                    value.clone(),
                    MirSourceOrigin::body(body.id, body.origin.span),
                )
                .map_err(input_error)?;
        }
        for (body, value) in self.request.schema_defaults.evaluated_defaults() {
            let Some(value) = value else {
                continue;
            };
            let Some(hir_body) = self.request.graph.body(*body) else {
                continue;
            };
            self.targets
                .insert_evaluated_schema_default(
                    *body,
                    value.clone(),
                    MirSourceOrigin::body(*body, hir_body.origin.span),
                )
                .map_err(input_error)?;
        }
        Ok(())
    }

    fn finish(mut self) -> CompileResult<PreparedSemanticInput> {
        self.finish_contracts()?;
        self.close_contract_edges()?;
        if !self.diagnostics.is_empty() {
            return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                self.diagnostics,
            )));
        }
        let graph = self.request.graph;
        let analysis = self.executable_analysis;
        let targets = self.targets.build().map_err(input_error)?;
        let prepared = PreparedSemanticInput { analysis, targets };
        drop(prepared.lowering_inputs(graph, MirLoweringConfig::default())?);
        Ok(prepared)
    }

    fn body_for_expression(&self, expression: HirExprId) -> Option<&HirBody> {
        self.request
            .graph
            .bodies()
            .find(|body| body.expressions.contains_key(&expression))
    }

    fn selected_executable_roots(&self) -> CompileResult<Vec<(FunctionId, HirBodyId)>> {
        let mut roots = Vec::new();
        match self.request.roots {
            SemanticRoots::Function(declaration) => {
                let function = self
                    .function_ids
                    .get(&declaration)
                    .copied()
                    .ok_or_else(registry_input_error)?;
                let body = self
                    .request
                    .graph
                    .function_body(declaration)
                    .map(|body| body.id)
                    .ok_or_else(registry_input_error)?;
                roots.push((function, body));
            }
            SemanticRoots::Program => {
                for (declaration, function) in &self.function_ids {
                    let body = self
                        .request
                        .graph
                        .function_body(*declaration)
                        .map(|body| body.id)
                        .ok_or_else(registry_input_error)?;
                    roots.push((*function, body));
                }
                roots.extend(self.request.script_methods.methods().map(|method| {
                    (
                        script_function_id(
                            method.owner().package().as_str(),
                            &method.symbol_seed(),
                        ),
                        method.body(),
                    )
                }));
                roots.extend(self.state_initializer_ids.iter().filter_map(
                    |(declaration, function)| {
                        self.request
                            .graph
                            .state_initializer_body(*declaration)
                            .map(|body| (*function, body.id))
                    },
                ));
            }
        }
        roots.sort_unstable();
        roots.dedup();
        Ok(roots)
    }

    fn executable_body_ids(&self, root: HirBodyId) -> Vec<HirBodyId> {
        self.request
            .graph
            .bodies()
            .filter(|body| placements::runtime_semantic_body(body))
            .filter(|body| {
                self.request
                    .graph
                    .body_and_ancestors(body.id)
                    .any(|ancestor| ancestor.id == root)
            })
            .map(|body| body.id)
            .collect()
    }

    fn executable_analysis(
        &self,
        function: FunctionId,
    ) -> CompileResult<ExecutableAnalysisView<'_>> {
        self.executable_analysis.view(function).ok_or_else(|| {
            let body = self.selected_executable_roots().ok().and_then(|roots| {
                roots
                    .into_iter()
                    .find_map(|(candidate, body)| (candidate == function).then_some(body))
            });
            let origin = body
                .and_then(|body| self.request.graph.body(body))
                .map(|body| MirSourceOrigin::body(body.id, body.origin.span));
            origin.map_or_else(registry_input_error, |origin| {
                input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: format!(
                        "missing executable analysis generation for function #{}",
                        function.get()
                    ),
                })
            })
        })
    }

    fn reject_analysis_validation_diagnostics(&self) -> CompileResult<()> {
        let mut diagnostics = Vec::new();
        let mut roots = self.selected_executable_roots()?;
        roots.sort_by_key(|(function, body)| {
            self.request.graph.body(*body).map(|body| {
                (
                    body.origin.span.source,
                    body.origin.span.start,
                    body.origin.span.end,
                    *function,
                )
            })
        });
        for (function, _) in roots {
            for diagnostic in self.executable_analysis(function)?.validation_diagnostics() {
                if !diagnostics.contains(diagnostic) {
                    diagnostics.push(diagnostic.clone());
                }
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                diagnostics,
            )))
        }
    }

    fn rebuild_executable_analysis(
        &mut self,
        literal_contexts: &BTreeMap<FunctionId, BTreeMap<HirExprId, LiteralPrimitiveContext>>,
    ) -> CompileResult<()> {
        let inputs = self
            .selected_executable_roots()?
            .into_iter()
            .map(|(function, body)| {
                let mut input = ExecutableAnalysisInput::new(function, body);
                if let Some(method) = self.request.script_methods.methods().find(|method| {
                    script_function_id(method.owner().package().as_str(), &method.symbol_seed())
                        == function
                }) {
                    input = input.with_receiver(self.executable_receiver(method));
                }
                if let Some(contexts) = literal_contexts.get(&function) {
                    for (expression, context) in contexts {
                        input = input.with_literal_context(*expression, *context);
                    }
                }
                input
            })
            .collect::<Vec<_>>();
        self.executable_analysis = ExecutableAnalysisGeneration::from_module_graph_and_schema(
            self.request.graph,
            &self.registry_facts,
            inputs,
        )
        .map_err(|error| self.executable_analysis_error(error))?;
        Ok(())
    }

    fn executable_receiver(&self, method: &ScriptMethod) -> ExecutableReceiverInput {
        let target_type = method.owner().target_type();
        if let Some((declaration, _)) = self.request.type_symbols.iter().find(|(_, symbol)| {
            *symbol == target_type || symbol.ends_with(&format!("::{target_type}"))
        }) {
            let fact = match self
                .request
                .graph
                .declaration(*declaration)
                .map(|declaration| declaration.kind)
            {
                Some(DeclarationKind::Enum) => TypeFact::enum_type(target_type, None::<String>),
                _ => TypeFact::record(target_type),
            };
            return ExecutableReceiverInput::new(fact)
                .with_script_type(ScriptTypeTargetFact::declaration(*declaration));
        }
        let fact = self
            .registry_facts
            .type_fact(target_type)
            .cloned()
            .unwrap_or_else(|| TypeFact::host(target_type));
        ExecutableReceiverInput::new(fact)
    }

    fn executable_analysis_error(&self, error: ExecutableAnalysisError) -> CompileError {
        let body = match &error {
            ExecutableAnalysisError::MissingBody { body, .. }
            | ExecutableAnalysisError::ReceiverWithoutSelf { body, .. } => Some(*body),
            ExecutableAnalysisError::DuplicateFunction { function }
            | ExecutableAnalysisError::MissingFunction { function } => {
                self.selected_executable_roots().ok().and_then(|roots| {
                    roots
                        .into_iter()
                        .find_map(|(candidate, body)| (candidate == *function).then_some(body))
                })
            }
            ExecutableAnalysisError::ExpressionOutsideRoot {
                function,
                expression,
            } => {
                return self.expression_origin(*expression).map_or_else(registry_input_error, |origin| {
                    input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: format!(
                            "executable analysis for function #{} contains an out-of-root expression {expression:?}",
                            function.get()
                        ),
                    })
                });
            }
        };
        body.and_then(|body| self.request.graph.body(body))
            .map_or_else(registry_input_error, |body| {
                input_error(MirBuildError::InconsistentInput {
                    origin: MirSourceOrigin::body(body.id, body.origin.span),
                    message: error.to_string(),
                })
            })
    }

    fn expression_origin(&self, expression: HirExprId) -> Option<MirSourceOrigin> {
        let body = self.body_for_expression(expression)?;
        let expression_record = body.expressions.get(&expression)?;
        Some(MirSourceOrigin::expression(
            body.id,
            expression,
            expression_record.origin.span,
        ))
    }

    fn type_contract_for_hint(
        &self,
        module: vela_hir::ids::ModuleId,
        hint: &vela_hir::type_hint::HirTypeHint,
    ) -> Option<MirTypeContract> {
        schema::hir_hint_contract(
            self.request.graph,
            module,
            hint,
            &self.registry_facts,
            &self.type_ids,
            &self.type_shapes,
        )
    }

    fn remember_contract(&mut self, contract: &MirTypeContract, origin: MirSourceOrigin) {
        self.contract_edges.push((contract.clone(), origin));
    }

    fn remember_signature_contracts(
        &mut self,
        signature: &vela_mir::CompileSignature,
        origin: MirSourceOrigin,
    ) {
        for contract in signature
            .parameters
            .iter()
            .filter_map(|parameter| parameter.contract.as_ref())
            .chain(signature.return_contract.as_ref())
        {
            self.remember_contract(contract, origin);
        }
    }

    fn close_contract_edges(&mut self) -> CompileResult<()> {
        while !self.contract_edges.is_empty() {
            let edges = std::mem::take(&mut self.contract_edges);
            for (contract, origin) in edges {
                self.close_contract_edge(&contract, origin)?;
            }
        }
        Ok(())
    }

    fn close_contract_edge(
        &mut self,
        contract: &MirTypeContract,
        origin: MirSourceOrigin,
    ) -> CompileResult<()> {
        match contract {
            MirTypeContract::Definition(type_id)
            | MirTypeContract::Shape { type_id, .. }
            | MirTypeContract::Host(vela_mir::HostTypeTarget {
                semantic: type_id, ..
            }) => self.ensure_contract_type(*type_id, origin)?,
            MirTypeContract::Variant {
                type_id, variant, ..
            } => {
                self.ensure_contract_type(*type_id, origin)?;
                if self.catalog.variant(*variant).is_some() {
                    self.ensure_external_variant(*variant, origin)?;
                } else if !self
                    .variant_ids
                    .values()
                    .any(|candidate| candidate == variant)
                {
                    return Err(input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: format!(
                            "type contract references missing variant descriptor #{}",
                            variant.get()
                        ),
                    }));
                }
            }
            MirTypeContract::Array(element)
            | MirTypeContract::Set(element)
            | MirTypeContract::Iterator(element)
            | MirTypeContract::Option(element) => {
                if let Some(element) = element {
                    self.close_contract_edge(element, origin)?;
                }
            }
            MirTypeContract::Map { key, value }
            | MirTypeContract::Result {
                ok: key,
                err: value,
            } => {
                if let Some(key) = key {
                    self.close_contract_edge(key, origin)?;
                }
                if let Some(value) = value {
                    self.close_contract_edge(value, origin)?;
                }
            }
            MirTypeContract::Tuple(elements) => {
                for element in elements.iter().flatten() {
                    self.close_contract_edge(element, origin)?;
                }
            }
            MirTypeContract::Any
            | MirTypeContract::Primitive(_)
            | MirTypeContract::Range
            | MirTypeContract::Callable { .. } => {}
        }
        Ok(())
    }

    fn ensure_contract_type(
        &mut self,
        type_id: TypeId,
        origin: MirSourceOrigin,
    ) -> CompileResult<()> {
        if self.type_names.contains_key(&type_id) {
            return Ok(());
        }
        if self.catalog.ty(type_id).is_some() {
            return self.ensure_external_type(type_id, origin);
        }
        if let Some(kind) = vela_analysis::logical_records::LogicalRecordKind::from_type_id(type_id)
        {
            return self.ensure_logical_record(kind, origin);
        }
        Err(input_error(MirBuildError::InconsistentInput {
            origin,
            message: format!(
                "type contract references missing type descriptor #{}",
                type_id.get()
            ),
        }))
    }
}

fn input_error(error: MirBuildError) -> CompileError {
    let span = error.origin().map(|origin| origin.span);
    let error = CompileError::new(CompileErrorKind::MirInput(Box::new(error)));
    match span {
        Some(span) => error.with_span(span),
        None => error,
    }
}

fn registry_input_error() -> CompileError {
    CompileError::new(CompileErrorKind::RegistrySnapshot(
        "required definition metadata is missing".to_owned(),
    ))
}
