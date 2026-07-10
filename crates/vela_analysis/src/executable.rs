//! Executable-qualified semantic facts for production lowering.
//!
//! Editor analysis remains available through [`crate::facts::AnalysisFacts`].
//! This module builds an independent fact generation for each stable function
//! identity, so a shared HIR body can be instantiated for multiple concrete
//! method receivers without any query falling back to uninstantiated facts.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use vela_common::Diagnostic;
use vela_def::FunctionId;
use vela_hir::ids::{
    HirBlockId, HirBodyId, HirDeclId, HirExprId, HirLocalId, HirPatternId, HirStmtId,
};
use vela_hir::module_graph::ModuleGraph;

use crate::facts::{AnalysisFacts, ExecutableReceiverSeed};
use crate::literals::{LiteralPrimitiveContext, LiteralResult};
use crate::registry::{RegistryEffectFact, RegistryFacts};
use crate::semantic_facts::{
    CallTargetFact, ConstructorTargetFact, ControlFlowFact, HostPathTargetFact, MemberTargetFact,
    OperatorTargetFact, ScriptTypeTargetFact,
};
use crate::type_fact::TypeFact;
use crate::validation::{
    ArrayOrderingCapabilityFact, CallArgumentPlacementFact, ExecutableValidationFacts,
    LoopControlFact, OperatorCapabilityFact,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableReceiverInput {
    fact: TypeFact,
    script_type: Option<ScriptTypeTargetFact>,
}

impl ExecutableReceiverInput {
    #[must_use]
    pub fn new(fact: TypeFact) -> Self {
        Self {
            fact,
            script_type: None,
        }
    }

    #[must_use]
    pub fn with_script_type(mut self, script_type: ScriptTypeTargetFact) -> Self {
        self.script_type = Some(script_type);
        self
    }

    #[must_use]
    pub const fn fact(&self) -> &TypeFact {
        &self.fact
    }

    #[must_use]
    pub const fn script_type(&self) -> Option<&ScriptTypeTargetFact> {
        self.script_type.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableAnalysisInput {
    function: FunctionId,
    body: HirBodyId,
    receiver: Option<ExecutableReceiverInput>,
    literal_contexts: BTreeMap<HirExprId, LiteralPrimitiveContext>,
}

impl ExecutableAnalysisInput {
    #[must_use]
    pub fn new(function: FunctionId, body: HirBodyId) -> Self {
        Self {
            function,
            body,
            receiver: None,
            literal_contexts: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_receiver(mut self, receiver: ExecutableReceiverInput) -> Self {
        self.receiver = Some(receiver);
        self
    }

    #[must_use]
    pub fn with_literal_context(
        mut self,
        expression: HirExprId,
        context: LiteralPrimitiveContext,
    ) -> Self {
        self.literal_contexts.insert(expression, context);
        self
    }

    #[must_use]
    pub fn with_literal_contexts(
        mut self,
        contexts: impl IntoIterator<Item = (HirExprId, LiteralPrimitiveContext)>,
    ) -> Self {
        self.literal_contexts.extend(contexts);
        self
    }

    #[must_use]
    pub const fn function(&self) -> FunctionId {
        self.function
    }

    #[must_use]
    pub const fn body(&self) -> HirBodyId {
        self.body
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutableAnalysisError {
    MissingBody {
        function: FunctionId,
        body: HirBodyId,
    },
    DuplicateFunction {
        function: FunctionId,
    },
    MissingFunction {
        function: FunctionId,
    },
    ReceiverWithoutSelf {
        function: FunctionId,
        body: HirBodyId,
    },
    ExpressionOutsideRoot {
        function: FunctionId,
        expression: HirExprId,
    },
}

impl fmt::Display for ExecutableAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBody { function, body } => write!(
                formatter,
                "function #{} references missing HIR body #{}",
                function.get(),
                body.get()
            ),
            Self::DuplicateFunction { function } => write!(
                formatter,
                "executable analysis contains duplicate function #{}",
                function.get()
            ),
            Self::MissingFunction { function } => write!(
                formatter,
                "executable analysis does not contain function #{}",
                function.get()
            ),
            Self::ReceiverWithoutSelf { function, body } => write!(
                formatter,
                "function #{} supplies a receiver for HIR body #{} without a self binding",
                function.get(),
                body.get()
            ),
            Self::ExpressionOutsideRoot {
                function,
                expression,
            } => write!(
                formatter,
                "function #{} supplies a literal context for expression #{} outside its body closure",
                function.get(),
                expression.get()
            ),
        }
    }
}

impl Error for ExecutableAnalysisError {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExecutableRootFacts {
    body: HirBodyId,
    bodies: BTreeSet<HirBodyId>,
    receiver: Option<ExecutableReceiverInput>,
    facts: AnalysisFacts,
    validation: ExecutableValidationFacts,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExecutableAnalysisGeneration {
    schema: Option<RegistryFacts>,
    roots: BTreeMap<FunctionId, ExecutableRootFacts>,
}

impl ExecutableAnalysisGeneration {
    pub fn from_module_graph(
        graph: &ModuleGraph,
        inputs: impl IntoIterator<Item = ExecutableAnalysisInput>,
    ) -> Result<Self, ExecutableAnalysisError> {
        Self::build(graph, None, inputs)
    }

    pub fn from_module_graph_and_schema(
        graph: &ModuleGraph,
        schema: &RegistryFacts,
        inputs: impl IntoIterator<Item = ExecutableAnalysisInput>,
    ) -> Result<Self, ExecutableAnalysisError> {
        Self::build(graph, Some(schema), inputs)
    }

    fn build(
        graph: &ModuleGraph,
        schema: Option<&RegistryFacts>,
        inputs: impl IntoIterator<Item = ExecutableAnalysisInput>,
    ) -> Result<Self, ExecutableAnalysisError> {
        let mut generation = Self {
            schema: schema.cloned(),
            roots: BTreeMap::new(),
        };
        for input in inputs {
            if generation.roots.contains_key(&input.function) {
                return Err(ExecutableAnalysisError::DuplicateFunction {
                    function: input.function,
                });
            }
            let function = input.function;
            let root = analyze_root(graph, generation.schema.as_ref(), &input)?;
            generation.roots.insert(function, root);
        }
        Ok(generation)
    }

    pub fn rebuild_literal_contexts(
        &mut self,
        graph: &ModuleGraph,
        function: FunctionId,
        contexts: impl IntoIterator<Item = (HirExprId, LiteralPrimitiveContext)>,
    ) -> Result<(), ExecutableAnalysisError> {
        let existing = self
            .roots
            .get(&function)
            .ok_or(ExecutableAnalysisError::MissingFunction { function })?;
        let input = ExecutableAnalysisInput {
            function,
            body: existing.body,
            receiver: existing.receiver.clone(),
            literal_contexts: contexts.into_iter().collect(),
        };
        let root = analyze_root(graph, self.schema.as_ref(), &input)?;
        self.roots.insert(function, root);
        Ok(())
    }

    #[must_use]
    pub fn view(&self, function: FunctionId) -> Option<ExecutableAnalysisView<'_>> {
        self.roots
            .get(&function)
            .map(|root| ExecutableAnalysisView { function, root })
    }

    pub fn functions(&self) -> impl Iterator<Item = FunctionId> + '_ {
        self.roots.keys().copied()
    }
}

fn analyze_root(
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    input: &ExecutableAnalysisInput,
) -> Result<ExecutableRootFacts, ExecutableAnalysisError> {
    let root = graph
        .body(input.body)
        .ok_or(ExecutableAnalysisError::MissingBody {
            function: input.function,
            body: input.body,
        })?;
    let bodies = executable_body_closure(graph, input.body);
    for expression in input.literal_contexts.keys() {
        if !expression_belongs_to(graph, &bodies, *expression) {
            return Err(ExecutableAnalysisError::ExpressionOutsideRoot {
                function: input.function,
                expression: *expression,
            });
        }
    }
    let receiver = match input.receiver.as_ref() {
        Some(receiver) => {
            let local = root
                .self_binding
                .ok_or(ExecutableAnalysisError::ReceiverWithoutSelf {
                    function: input.function,
                    body: input.body,
                })?;
            Some(ExecutableReceiverSeed {
                local,
                fact: receiver.fact(),
                script_type: receiver.script_type(),
            })
        }
        None => None,
    };
    let facts = AnalysisFacts::from_executable_scope(
        graph,
        schema,
        &bodies,
        receiver,
        &input.literal_contexts,
    );
    let validation = ExecutableValidationFacts::from_analysis(graph, schema, &facts, &bodies);
    Ok(ExecutableRootFacts {
        body: input.body,
        bodies,
        receiver: input.receiver.clone(),
        facts,
        validation,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct ExecutableAnalysisView<'generation> {
    function: FunctionId,
    root: &'generation ExecutableRootFacts,
}

impl ExecutableAnalysisView<'_> {
    #[must_use]
    pub const fn function(&self) -> FunctionId {
        self.function
    }

    #[must_use]
    pub const fn root_body(&self) -> HirBodyId {
        self.root.body
    }

    #[must_use]
    pub fn contains_body(&self, body: HirBodyId) -> bool {
        self.root.bodies.contains(&body)
    }

    #[must_use]
    pub fn declaration(&self, declaration: HirDeclId) -> Option<&TypeFact> {
        self.root.facts.declaration(declaration)
    }

    #[must_use]
    pub fn local(&self, local: HirLocalId) -> Option<&TypeFact> {
        self.root.facts.local(local)
    }

    #[must_use]
    pub fn expression(&self, expression: HirExprId) -> Option<&TypeFact> {
        self.root.facts.expression(expression)
    }

    #[must_use]
    pub fn script_type(&self, expression: HirExprId) -> Option<&ScriptTypeTargetFact> {
        self.root.facts.script_type(expression)
    }

    #[must_use]
    pub fn local_script_type(&self, local: HirLocalId) -> Option<&ScriptTypeTargetFact> {
        self.root.facts.local_script_type(local)
    }

    #[must_use]
    pub fn literal(&self, expression: HirExprId) -> Option<&LiteralResult> {
        self.root.facts.literal(expression)
    }

    #[must_use]
    pub fn pattern(&self, pattern: HirPatternId) -> Option<&TypeFact> {
        self.root.facts.pattern(pattern)
    }

    #[must_use]
    pub fn call_target(&self, expression: HirExprId) -> Option<&CallTargetFact> {
        self.root.facts.call_target(expression)
    }

    #[must_use]
    pub fn call_argument_placement(
        &self,
        expression: HirExprId,
    ) -> Option<&CallArgumentPlacementFact> {
        self.root.validation.call_argument_placement(expression)
    }

    #[must_use]
    pub fn member_target(&self, expression: HirExprId) -> Option<&MemberTargetFact> {
        self.root.facts.member_target(expression)
    }

    #[must_use]
    pub fn operator_target(&self, expression: HirExprId) -> Option<OperatorTargetFact> {
        self.root.facts.operator_target(expression)
    }

    #[must_use]
    pub fn constructor_target(&self, expression: HirExprId) -> Option<&ConstructorTargetFact> {
        self.root.facts.constructor_target(expression)
    }

    #[must_use]
    pub fn pattern_constructor_target(
        &self,
        pattern: HirPatternId,
    ) -> Option<&ConstructorTargetFact> {
        self.root.facts.pattern_constructor_target(pattern)
    }

    #[must_use]
    pub fn host_path_target(&self, expression: HirExprId) -> Option<&HostPathTargetFact> {
        self.root.facts.host_path_target(expression)
    }

    #[must_use]
    pub fn effect(&self, expression: HirExprId) -> Option<&RegistryEffectFact> {
        self.root.facts.effect(expression)
    }

    #[must_use]
    pub fn control_flow(&self, expression: HirExprId) -> Option<&ControlFlowFact> {
        self.root.facts.control_flow(expression)
    }

    #[must_use]
    pub fn block_control_flow(&self, block: HirBlockId) -> Option<&ControlFlowFact> {
        self.root.facts.block_control_flow(block)
    }

    #[must_use]
    pub fn statement_control_flow(&self, statement: HirStmtId) -> Option<&ControlFlowFact> {
        self.root.facts.statement_control_flow(statement)
    }

    #[must_use]
    pub fn operator_capability(&self, expression: HirExprId) -> Option<&OperatorCapabilityFact> {
        self.root.validation.operator(expression)
    }

    #[must_use]
    pub fn array_ordering_capability(
        &self,
        expression: HirExprId,
    ) -> Option<&ArrayOrderingCapabilityFact> {
        self.root.validation.array_ordering(expression)
    }

    #[must_use]
    pub fn loop_control(&self, statement: HirStmtId) -> Option<LoopControlFact> {
        self.root.validation.loop_control(statement)
    }

    #[must_use]
    pub fn validation_diagnostics(&self) -> &[Diagnostic] {
        self.root.validation.diagnostics()
    }

    #[must_use]
    pub fn literal_diagnostics(&self, graph: &ModuleGraph) -> Vec<Diagnostic> {
        self.root.facts.literal_diagnostics(graph)
    }
}

fn executable_body_closure(graph: &ModuleGraph, root: HirBodyId) -> BTreeSet<HirBodyId> {
    graph
        .bodies()
        .filter(|body| {
            graph
                .body_and_ancestors(body.id)
                .any(|ancestor| ancestor.id == root)
        })
        .map(|body| body.id)
        .collect()
}

fn expression_belongs_to(
    graph: &ModuleGraph,
    bodies: &BTreeSet<HirBodyId>,
    expression: HirExprId,
) -> bool {
    bodies.iter().any(|body| {
        graph
            .body(*body)
            .is_some_and(|body| body.expressions.contains_key(&expression))
    })
}

#[cfg(test)]
mod callback_tests;

#[cfg(test)]
mod tests;
