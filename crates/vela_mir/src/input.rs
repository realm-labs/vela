use std::collections::{BTreeMap, btree_map::Entry};
use std::error::Error;
use std::fmt;

use vela_analysis::executable::ExecutableAnalysisView;
use vela_common::CallableAsyncness;
use vela_def::{FieldId, FunctionId, MethodId, StateId, TypeId, VariantId};
use vela_hir::body::HirBodyOwner;
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirNodeId, HirPatternId};
use vela_hir::module_graph::ModuleGraph;

use crate::{
    CompileFieldDescriptor, CompileFunctionClass, CompileFunctionDescriptor,
    CompileGlobalDescriptor, CompileMethodClass, CompileMethodDescriptor, CompileTypeDescriptor,
    CompileVariantDescriptor, MirBlockId, MirEffect, MirEvaluatedConstant, MirLocalId,
    MirSourceOrigin, MirTargetTable, MirTempId, MirTypeContract,
};

mod calls;
mod host;
mod identity;
mod lambdas;
mod origins;
mod placements;
mod try_targets;
mod validation;

use origins::CompileTargetOrigins;

pub use calls::{
    CompileCallArguments, CompileCallTarget, CompileCalleeTarget, CompileDynamicCallArgument,
    CompilePlacedCallArgument, CompilePlacedCallValue, CompileReflectionCall,
};
pub use host::{
    CompileHostIndexCapability, CompileHostPathSegment, CompileHostPathTarget, HostFieldTarget,
    HostMethodTarget,
};
pub use lambdas::{CompileLambdaParameterTarget, CompileLambdaTarget};
pub use placements::{
    CompileConstructorField, CompileConstructorTarget, CompileConstructorValue,
    CompileDynamicConstructorField, CompileFieldTarget, CompileFunctionTargets, CompileGuardKey,
    CompileGuardTarget, CompileMemberTarget, CompilePatternConstructorTarget, CompileTargetKind,
};
pub use try_targets::{CompileTryFamily, CompileTryLayoutTarget, CompileTryTarget};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicMethodTarget {
    pub member: String,
    pub positional_arity: u32,
    pub named_arguments: Vec<String>,
}

impl DynamicMethodTarget {
    #[must_use]
    pub fn method(
        member: impl Into<String>,
        positional_arity: u32,
        named_arguments: Vec<String>,
    ) -> Self {
        Self {
            member: member.into(),
            positional_arity,
            named_arguments,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MethodExecutableTarget {
    pub method: MethodId,
    pub function: FunctionId,
    pub owner: TypeId,
    pub node: HirNodeId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompileFunctionIdentity {
    Function(FunctionId),
    Method(MethodExecutableTarget),
}

impl CompileFunctionIdentity {
    #[must_use]
    pub const fn function(self) -> FunctionId {
        match self {
            Self::Function(function) => function,
            Self::Method(target) => target.function,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileFunctionTarget {
    pub identity: CompileFunctionIdentity,
    pub body: HirBodyId,
    pub origin: MirSourceOrigin,
}

pub type CompileGlobalTarget = CompileGlobalDescriptor;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileParameter {
    pub name: String,
    pub contract: Option<MirTypeContract>,
    pub default: CompileParameterDefault,
    pub origin: Option<MirSourceOrigin>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileParameterDefault {
    Required,
    HirBody(HirBodyId),
    RuntimeProvided,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileSignature {
    pub asyncness: CallableAsyncness,
    pub parameters: Vec<CompileParameter>,
    pub positional: CompilePositionalPolicy,
    pub return_contract: Option<MirTypeContract>,
    pub effect: MirEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilePositionalPolicy {
    /// Match the declared parameters exactly, except for an omitted trailing
    /// suffix whose defaults are explicitly represented by the signature.
    ExactOrTrailingDefaults,
    /// Preserve the positional vector and let the current runtime callable
    /// perform its arity/default validation.
    RuntimeChecked,
    /// A proven variadic callable with a statically known minimum arity.
    Variadic { minimum: u32 },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompileTargetSnapshot {
    functions: BTreeMap<FunctionId, CompileFunctionTarget>,
    functions_by_body: BTreeMap<HirBodyId, Vec<FunctionId>>,
    functions_by_declaration: BTreeMap<HirDeclId, FunctionId>,
    methods_by_node: BTreeMap<HirNodeId, Vec<MethodExecutableTarget>>,
    lambdas: BTreeMap<(FunctionId, HirBodyId), CompileLambdaTarget>,
    types_by_declaration: BTreeMap<HirDeclId, TypeId>,
    types_by_name: BTreeMap<String, TypeId>,
    calls: BTreeMap<(FunctionId, HirExprId), CompileCallTarget>,
    members: BTreeMap<(FunctionId, HirExprId), CompileMemberTarget>,
    constructors: BTreeMap<(FunctionId, HirExprId), CompileConstructorTarget>,
    pattern_constructors: BTreeMap<(FunctionId, HirPatternId), CompilePatternConstructorTarget>,
    host_paths: BTreeMap<(FunctionId, HirExprId), CompileHostPathTarget>,
    try_targets: BTreeMap<(FunctionId, HirExprId), CompileTryTarget>,
    globals: BTreeMap<HirDeclId, StateId>,
    evaluated_constants: BTreeMap<HirDeclId, MirEvaluatedConstant>,
    evaluated_schema_defaults: BTreeMap<HirBodyId, MirEvaluatedConstant>,
    targets: MirTargetTable,
    guards: BTreeMap<CompileGuardKey, CompileGuardTarget>,
    origins: CompileTargetOrigins,
}

impl CompileTargetSnapshot {
    #[must_use]
    pub fn builder() -> CompileTargetSnapshotBuilder {
        CompileTargetSnapshotBuilder::default()
    }

    #[must_use]
    pub fn function(&self, function: FunctionId) -> Option<&CompileFunctionTarget> {
        self.functions.get(&function)
    }

    pub fn functions_for_body(&self, body: HirBodyId) -> &[FunctionId] {
        self.functions_by_body.get(&body).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn global(&self, declaration: HirDeclId) -> Option<&CompileGlobalTarget> {
        self.globals
            .get(&declaration)
            .and_then(|id| self.targets.global(*id))
    }

    #[must_use]
    pub fn global_by_id(&self, id: StateId) -> Option<&CompileGlobalTarget> {
        self.targets.global(id)
    }

    #[must_use]
    pub fn evaluated_constant(&self, declaration: HirDeclId) -> Option<&MirEvaluatedConstant> {
        self.evaluated_constants.get(&declaration)
    }

    #[must_use]
    pub fn evaluated_schema_default(&self, body: HirBodyId) -> Option<&MirEvaluatedConstant> {
        self.evaluated_schema_defaults.get(&body)
    }

    #[must_use]
    pub fn function_descriptor(&self, function: FunctionId) -> Option<&CompileFunctionDescriptor> {
        self.targets.function(function)
    }

    #[must_use]
    pub fn method_descriptor(
        &self,
        owner: TypeId,
        method: MethodId,
    ) -> Option<&CompileMethodDescriptor> {
        self.targets.method(owner, method)
    }

    #[must_use]
    pub fn type_descriptor(&self, type_id: TypeId) -> Option<&CompileTypeDescriptor> {
        self.targets.type_descriptor(type_id)
    }

    #[must_use]
    pub fn variant_descriptor(&self, variant: VariantId) -> Option<&CompileVariantDescriptor> {
        self.targets.variant(variant)
    }

    #[must_use]
    pub fn field_descriptor(&self, field: FieldId) -> Option<&CompileFieldDescriptor> {
        self.targets.field(field)
    }

    #[must_use]
    pub const fn target_table(&self) -> &MirTargetTable {
        &self.targets
    }
}

#[derive(Clone, Debug, Default)]
pub struct CompileTargetSnapshotBuilder {
    snapshot: CompileTargetSnapshot,
}

impl CompileTargetSnapshotBuilder {
    pub fn insert_function(
        &mut self,
        body: HirBodyId,
        identity: CompileFunctionIdentity,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let function = identity.function();
        match self.snapshot.functions.entry(function) {
            Entry::Vacant(entry) => {
                entry.insert(CompileFunctionTarget {
                    identity,
                    body,
                    origin,
                });
                self.snapshot
                    .functions_by_body
                    .entry(body)
                    .or_default()
                    .push(function);
                self.snapshot.origins.roots.insert(function, origin);
                Ok(())
            }
            Entry::Occupied(_) => Err(MirBuildError::DuplicateFunctionTarget { function, origin }),
        }
    }

    pub fn insert_global(
        &mut self,
        declaration: HirDeclId,
        target: CompileGlobalTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        if self.snapshot.globals.contains_key(&declaration) {
            return Err(MirBuildError::DuplicateGlobalTarget {
                declaration,
                origin,
            });
        }
        if !self.snapshot.targets.insert_global(target.clone()) {
            return Err(duplicate_descriptor("global", target.id.get(), origin));
        }
        self.snapshot
            .origins
            .global_descriptors
            .insert(target.id, origin);
        self.snapshot.globals.insert(declaration, target.id);
        self.snapshot
            .origins
            .global_bindings
            .insert(declaration, origin);
        Ok(())
    }

    pub fn insert_evaluated_constant(
        &mut self,
        declaration: HirDeclId,
        value: MirEvaluatedConstant,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        match self.snapshot.evaluated_constants.entry(declaration) {
            Entry::Vacant(entry) => {
                entry.insert(value);
                self.snapshot
                    .origins
                    .evaluated_constants
                    .insert(declaration, origin);
                Ok(())
            }
            Entry::Occupied(_) => Err(MirBuildError::DuplicateEvaluatedConstant {
                declaration,
                origin,
            }),
        }
    }

    pub fn insert_evaluated_schema_default(
        &mut self,
        body: HirBodyId,
        value: MirEvaluatedConstant,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        match self.snapshot.evaluated_schema_defaults.entry(body) {
            Entry::Vacant(entry) => {
                entry.insert(value);
                self.snapshot
                    .origins
                    .evaluated_schema_defaults
                    .insert(body, origin);
                Ok(())
            }
            Entry::Occupied(_) => {
                Err(MirBuildError::DuplicateEvaluatedSchemaDefault { body, origin })
            }
        }
    }

    pub fn insert_function_descriptor(
        &mut self,
        descriptor: CompileFunctionDescriptor,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let id = descriptor.id;
        if self.snapshot.targets.insert_function(descriptor) {
            self.snapshot
                .origins
                .function_descriptors
                .insert(id, origin);
            Ok(())
        } else {
            Err(duplicate_descriptor("function", id.get(), origin))
        }
    }

    pub fn insert_method_descriptor(
        &mut self,
        descriptor: CompileMethodDescriptor,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let id = descriptor.id;
        let owner = descriptor.owner;
        if self.snapshot.targets.insert_method(descriptor) {
            self.snapshot
                .origins
                .method_descriptors
                .insert((owner, id), origin);
            Ok(())
        } else {
            Err(MirBuildError::InconsistentInput {
                origin,
                message: format!(
                    "duplicate method descriptor #{} for owner #{}",
                    id.get(),
                    owner.get()
                ),
            })
        }
    }

    pub fn insert_type_descriptor(
        &mut self,
        descriptor: CompileTypeDescriptor,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let id = descriptor.id;
        let canonical_name = descriptor.canonical_name.clone();
        if self.snapshot.type_descriptor(id).is_some()
            || self.snapshot.types_by_name.contains_key(&canonical_name)
        {
            return Err(MirBuildError::InconsistentInput {
                origin,
                message: format!(
                    "duplicate type descriptor #{} or canonical name {canonical_name:?}",
                    id.get()
                ),
            });
        }
        if self.snapshot.targets.insert_type(descriptor) {
            self.snapshot.types_by_name.insert(canonical_name, id);
            self.snapshot.origins.type_descriptors.insert(id, origin);
            Ok(())
        } else {
            Err(duplicate_descriptor("type", id.get(), origin))
        }
    }

    pub fn insert_variant_descriptor(
        &mut self,
        descriptor: CompileVariantDescriptor,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let id = descriptor.id;
        if self.snapshot.targets.insert_variant(descriptor) {
            self.snapshot.origins.variant_descriptors.insert(id, origin);
            Ok(())
        } else {
            Err(duplicate_descriptor("variant", id.get(), origin))
        }
    }

    pub fn insert_field_descriptor(
        &mut self,
        descriptor: CompileFieldDescriptor,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let id = descriptor.id;
        if self.snapshot.targets.insert_field(descriptor) {
            self.snapshot.origins.field_descriptors.insert(id, origin);
            Ok(())
        } else {
            Err(duplicate_descriptor("field", id.get(), origin))
        }
    }

    pub fn build(self) -> Result<CompileTargetSnapshot, MirBuildError> {
        self.snapshot.validate()?;
        Ok(self.snapshot)
    }

    #[cfg(test)]
    pub(crate) fn build_unchecked(self) -> CompileTargetSnapshot {
        self.snapshot
    }
}

fn duplicate_descriptor(kind: &str, id: u128, origin: MirSourceOrigin) -> MirBuildError {
    MirBuildError::InconsistentInput {
        origin,
        message: format!("duplicate {kind} descriptor #{id}"),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirLoweringConfig {
    pub emit_debug_locals: bool,
    pub compute_liveness: bool,
}

impl Default for MirLoweringConfig {
    fn default() -> Self {
        Self {
            emit_debug_locals: true,
            compute_liveness: true,
        }
    }
}

#[derive(Clone, Copy)]
pub struct MirLoweringInput<'a> {
    graph: &'a ModuleGraph,
    identity: CompileFunctionIdentity,
    body: HirBodyId,
    analysis: ExecutableAnalysisView<'a>,
    targets: CompileFunctionTargets<'a>,
    config: MirLoweringConfig,
}

impl<'a> MirLoweringInput<'a> {
    pub fn new(
        graph: &'a ModuleGraph,
        identity: CompileFunctionIdentity,
        body: HirBodyId,
        analysis: ExecutableAnalysisView<'a>,
        targets: &'a CompileTargetSnapshot,
        config: MirLoweringConfig,
    ) -> Result<Self, MirBuildError> {
        let function = identity.function();
        let target = targets.function(function);
        let hir_body = graph.body(body).ok_or(match target {
            Some(target) => MirBuildError::MissingHirBody {
                body,
                origin: target.origin,
            },
            None => MirBuildError::MissingCompilationRoot { function, body },
        })?;
        match &hir_body.owner {
            HirBodyOwner::ConstInitializer(_)
            | HirBodyOwner::StateInitializer(_)
            | HirBodyOwner::SchemaFieldDefault(_)
            | HirBodyOwner::ParameterDefault { .. } => {
                return Err(MirBuildError::NonRuntimeBody {
                    body,
                    origin: MirSourceOrigin::body(body, hir_body.origin.span),
                });
            }
            HirBodyOwner::Declaration(_)
            | HirBodyOwner::TraitDefaultMethod(_)
            | HirBodyOwner::ImplMethod(_)
            | HirBodyOwner::Lambda { .. } => {}
        }
        let target = target.ok_or(MirBuildError::MissingFunctionTarget {
            function,
            origin: MirSourceOrigin::body(body, hir_body.origin.span),
        })?;
        if target.body != body {
            return Err(MirBuildError::FunctionBodyMismatch {
                function,
                expected: target.body,
                actual: body,
                origin: MirSourceOrigin::body(body, hir_body.origin.span),
            });
        }
        if target.identity != identity {
            return Err(MirBuildError::FunctionIdentityMismatch {
                function,
                expected: Box::new(target.identity),
                actual: Box::new(identity),
                origin: MirSourceOrigin::body(body, hir_body.origin.span),
            });
        }
        let origin = MirSourceOrigin::body(body, hir_body.origin.span);
        let inconsistent = |message| MirBuildError::InconsistentInput { origin, message };
        if analysis.function() != function {
            return Err(inconsistent(format!(
                "executable analysis for function #{} cannot lower function #{}",
                analysis.function().get(),
                function.get()
            )));
        }
        if analysis.root_body() != body {
            return Err(inconsistent(format!(
                "executable analysis for function #{} targets HIR body {:?}, not {:?}",
                function.get(),
                analysis.root_body(),
                body
            )));
        }
        let function_descriptor = targets.function_descriptor(function).ok_or_else(|| {
            inconsistent(format!("missing function descriptor #{}", function.get()))
        })?;
        if function_descriptor.class != CompileFunctionClass::Script {
            return Err(inconsistent(format!(
                "HIR-backed function #{} is not classified as a script function",
                function.get()
            )));
        }
        lambdas::validate_hir_closure(
            graph,
            targets,
            function,
            body,
            &function_descriptor.canonical_symbol,
        )?;
        if let CompileFunctionIdentity::Method(method) = target.identity {
            let descriptor = targets
                .method_descriptor(method.owner, method.method)
                .ok_or_else(|| {
                    inconsistent(format!(
                        "missing method descriptor #{} for function #{}",
                        method.method.get(),
                        function.get()
                    ))
                })?;
            if descriptor.owner != method.owner {
                return Err(inconsistent(format!(
                    "method descriptor #{} owner #{} does not match compile target owner #{}",
                    method.method.get(),
                    descriptor.owner.get(),
                    method.owner.get()
                )));
            }
            match &descriptor.class {
                CompileMethodClass::Script {
                    executable,
                    code_symbol,
                    ..
                } if *executable == method
                    && code_symbol == &function_descriptor.canonical_symbol => {}
                CompileMethodClass::Script { executable, .. } if *executable != method => {
                    return Err(inconsistent(format!(
                        "method descriptor #{} executable does not match its compile target",
                        method.method.get()
                    )));
                }
                CompileMethodClass::Script { code_symbol, .. } => {
                    return Err(inconsistent(format!(
                        "method descriptor #{} code symbol {code_symbol:?} does not match function symbol {:?}",
                        method.method.get(),
                        function_descriptor.canonical_symbol
                    )));
                }
                CompileMethodClass::Host { .. }
                | CompileMethodClass::Value
                | CompileMethodClass::Registry => {
                    return Err(inconsistent(format!(
                        "method descriptor #{} is not a script method",
                        method.method.get()
                    )));
                }
            }
        }
        Ok(Self {
            graph,
            identity,
            body,
            analysis,
            targets: CompileFunctionTargets::new(targets, target),
            config,
        })
    }

    #[must_use]
    pub const fn graph(self) -> &'a ModuleGraph {
        self.graph
    }

    #[must_use]
    pub const fn body(self) -> HirBodyId {
        self.body
    }

    #[must_use]
    pub const fn function(self) -> FunctionId {
        self.identity.function()
    }

    #[must_use]
    pub const fn identity(self) -> CompileFunctionIdentity {
        self.identity
    }

    #[must_use]
    pub const fn analysis(self) -> ExecutableAnalysisView<'a> {
        self.analysis
    }

    #[must_use]
    pub const fn targets(self) -> CompileFunctionTargets<'a> {
        self.targets
    }

    #[must_use]
    pub const fn config(self) -> MirLoweringConfig {
        self.config
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirBuildError {
    MissingHirBody {
        body: HirBodyId,
        origin: MirSourceOrigin,
    },
    MissingCompilationRoot {
        function: FunctionId,
        body: HirBodyId,
    },
    NonRuntimeBody {
        body: HirBodyId,
        origin: MirSourceOrigin,
    },
    DuplicateFunctionTarget {
        function: FunctionId,
        origin: MirSourceOrigin,
    },
    DuplicateCompileTarget {
        function: FunctionId,
        kind: CompileTargetKind,
        expression: HirExprId,
        origin: MirSourceOrigin,
    },
    DuplicatePatternConstructor {
        function: FunctionId,
        pattern: HirPatternId,
        origin: MirSourceOrigin,
    },
    DuplicateGlobalTarget {
        declaration: HirDeclId,
        origin: MirSourceOrigin,
    },
    DuplicateEvaluatedConstant {
        declaration: HirDeclId,
        origin: MirSourceOrigin,
    },
    DuplicateEvaluatedSchemaDefault {
        body: HirBodyId,
        origin: MirSourceOrigin,
    },
    DuplicateGuardTarget {
        key: CompileGuardKey,
        origin: MirSourceOrigin,
    },
    MissingFunctionTarget {
        function: FunctionId,
        origin: MirSourceOrigin,
    },
    FunctionBodyMismatch {
        function: FunctionId,
        expected: HirBodyId,
        actual: HirBodyId,
        origin: MirSourceOrigin,
    },
    FunctionIdentityMismatch {
        function: FunctionId,
        expected: Box<CompileFunctionIdentity>,
        actual: Box<CompileFunctionIdentity>,
        origin: MirSourceOrigin,
    },
    DuplicateMirFunctionId {
        function_id: FunctionId,
        origin: MirSourceOrigin,
    },
    DuplicateMirMethodId {
        owner: TypeId,
        method_id: MethodId,
        origin: MirSourceOrigin,
    },
    MissingMirFunction {
        function: crate::MirFunctionId,
        origin: MirSourceOrigin,
    },
    MissingMirFunctionReservation {
        function: crate::MirFunctionId,
        origin: MirSourceOrigin,
    },
    MirFunctionAlreadyDefined {
        function: crate::MirFunctionId,
        origin: MirSourceOrigin,
    },
    MirFunctionReservationBodyMismatch {
        function: crate::MirFunctionId,
        expected: HirBodyId,
        actual: HirBodyId,
        origin: MirSourceOrigin,
    },
    MirFunctionReservationOwnerMismatch {
        function: crate::MirFunctionId,
        expected: Box<crate::MirFunctionOwner>,
        actual: Box<crate::MirFunctionOwner>,
        origin: MirSourceOrigin,
    },
    MissingBlock {
        block: MirBlockId,
        origin: MirSourceOrigin,
    },
    BlockAlreadyTerminated {
        block: MirBlockId,
        origin: MirSourceOrigin,
    },
    MissingTemp {
        temp: MirTempId,
        origin: MirSourceOrigin,
    },
    MissingLocal {
        local: MirLocalId,
        origin: MirSourceOrigin,
    },
    MissingGuard {
        guard: crate::MirGuardId,
        origin: MirSourceOrigin,
    },
    TempAlreadyDefined {
        temp: MirTempId,
        origin: MirSourceOrigin,
    },
    MissingStatementDestination {
        origin: MirSourceOrigin,
    },
    UnexpectedStatementDestination {
        origin: MirSourceOrigin,
    },
    InvalidCallArgumentPlacement {
        origin: MirSourceOrigin,
    },
    IncompleteEffect {
        origin: MirSourceOrigin,
        required: MirEffect,
        actual: MirEffect,
    },
    MissingSafepoint {
        origin: MirSourceOrigin,
    },
    InconsistentInput {
        origin: MirSourceOrigin,
        message: String,
    },
}

impl fmt::Display for MirBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHirBody { body, .. } => write!(formatter, "missing HIR body {body:?}"),
            Self::MissingCompilationRoot { function, body } => write!(
                formatter,
                "compilation root has neither function {function:?} nor HIR body {body:?}"
            ),
            Self::NonRuntimeBody { body, .. } => {
                write!(formatter, "HIR body {body:?} is not a runtime MIR function")
            }
            Self::DuplicateFunctionTarget { function, .. } => {
                write!(
                    formatter,
                    "duplicate compile target for function {function:?}"
                )
            }
            Self::DuplicateCompileTarget {
                function,
                kind,
                expression,
                ..
            } => {
                write!(
                    formatter,
                    "duplicate {kind} target for function {function:?} at {expression:?}"
                )
            }
            Self::DuplicatePatternConstructor {
                function, pattern, ..
            } => {
                write!(
                    formatter,
                    "duplicate constructor target for function {function:?} at {pattern:?}"
                )
            }
            Self::DuplicateGlobalTarget { declaration, .. } => {
                write!(formatter, "duplicate global target for {declaration:?}")
            }
            Self::DuplicateEvaluatedConstant { declaration, .. } => {
                write!(
                    formatter,
                    "duplicate evaluated constant for {declaration:?}"
                )
            }
            Self::DuplicateEvaluatedSchemaDefault { body, .. } => {
                write!(formatter, "duplicate evaluated schema default for {body:?}")
            }
            Self::DuplicateGuardTarget { key, .. } => {
                write!(formatter, "duplicate guard target {key:?}")
            }
            Self::MissingFunctionTarget { function, .. } => {
                write!(
                    formatter,
                    "missing compile target for function {function:?}"
                )
            }
            Self::FunctionBodyMismatch {
                function,
                expected,
                actual,
                ..
            } => {
                write!(
                    formatter,
                    "function {function:?} targets HIR body {expected:?}, not {actual:?}"
                )
            }
            Self::FunctionIdentityMismatch {
                function,
                expected,
                actual,
                ..
            } => write!(
                formatter,
                "function {function:?} has compile identity {expected:?}, not {actual:?}"
            ),
            Self::DuplicateMirFunctionId { function_id, .. } => {
                write!(formatter, "duplicate MIR function ID {function_id:?}")
            }
            Self::DuplicateMirMethodId {
                owner, method_id, ..
            } => {
                write!(
                    formatter,
                    "duplicate MIR method ID {method_id:?} for owner {owner:?}"
                )
            }
            Self::MissingMirFunction { function, .. } => {
                write!(formatter, "missing parent MIR function {function}")
            }
            Self::MissingMirFunctionReservation { function, .. } => {
                write!(formatter, "missing MIR function reservation {function}")
            }
            Self::MirFunctionAlreadyDefined { function, .. } => {
                write!(
                    formatter,
                    "MIR function reservation {function} is already defined"
                )
            }
            Self::MirFunctionReservationBodyMismatch {
                function,
                expected,
                actual,
                ..
            } => write!(
                formatter,
                "MIR function reservation {function} targets HIR body {expected:?}, not {actual:?}"
            ),
            Self::MirFunctionReservationOwnerMismatch {
                function,
                expected,
                actual,
                ..
            } => write!(
                formatter,
                "MIR function reservation {function} has owner {expected:?}, not {actual:?}"
            ),
            Self::MissingBlock { block, .. } => write!(formatter, "missing MIR block {block}"),
            Self::BlockAlreadyTerminated { block, .. } => {
                write!(formatter, "MIR block {block} already has a terminator")
            }
            Self::MissingTemp { temp, .. } => write!(formatter, "missing MIR temp {temp}"),
            Self::MissingLocal { local, .. } => write!(formatter, "missing MIR local {local}"),
            Self::MissingGuard { guard, .. } => write!(formatter, "missing MIR guard {guard}"),
            Self::TempAlreadyDefined { temp, .. } => {
                write!(formatter, "MIR temp {temp} already has a definition")
            }
            Self::MissingStatementDestination { .. } => {
                formatter.write_str("MIR operation requires a result destination")
            }
            Self::UnexpectedStatementDestination { .. } => {
                formatter.write_str("effect-only MIR operation cannot have a result destination")
            }
            Self::InvalidCallArgumentPlacement { .. } => formatter.write_str(
                "MIR call arguments do not match the target signature and default-delivery policy",
            ),
            Self::IncompleteEffect {
                required, actual, ..
            } => {
                write!(
                    formatter,
                    "MIR effect {actual:?} does not include required effect {required:?}"
                )
            }
            Self::MissingSafepoint { .. } => {
                formatter.write_str("effectful MIR operation requires a safepoint")
            }
            Self::InconsistentInput { origin, message } => write!(
                formatter,
                "inconsistent MIR input at {}:{}..{}: {message}",
                origin.span.source.get(),
                origin.span.start,
                origin.span.end
            ),
        }
    }
}

impl MirBuildError {
    #[must_use]
    pub const fn origin(&self) -> Option<MirSourceOrigin> {
        match self {
            Self::MissingHirBody { origin, .. }
            | Self::NonRuntimeBody { origin, .. }
            | Self::DuplicateFunctionTarget { origin, .. }
            | Self::DuplicateCompileTarget { origin, .. }
            | Self::DuplicatePatternConstructor { origin, .. }
            | Self::DuplicateGlobalTarget { origin, .. }
            | Self::DuplicateEvaluatedConstant { origin, .. }
            | Self::DuplicateEvaluatedSchemaDefault { origin, .. }
            | Self::DuplicateGuardTarget { origin, .. }
            | Self::MissingFunctionTarget { origin, .. }
            | Self::FunctionBodyMismatch { origin, .. }
            | Self::FunctionIdentityMismatch { origin, .. }
            | Self::DuplicateMirFunctionId { origin, .. }
            | Self::DuplicateMirMethodId { origin, .. }
            | Self::MissingMirFunction { origin, .. }
            | Self::MissingMirFunctionReservation { origin, .. }
            | Self::MirFunctionAlreadyDefined { origin, .. }
            | Self::MirFunctionReservationBodyMismatch { origin, .. }
            | Self::MirFunctionReservationOwnerMismatch { origin, .. }
            | Self::MissingBlock { origin, .. }
            | Self::BlockAlreadyTerminated { origin, .. }
            | Self::MissingTemp { origin, .. }
            | Self::MissingLocal { origin, .. }
            | Self::MissingGuard { origin, .. }
            | Self::TempAlreadyDefined { origin, .. }
            | Self::MissingStatementDestination { origin }
            | Self::UnexpectedStatementDestination { origin }
            | Self::InvalidCallArgumentPlacement { origin }
            | Self::IncompleteEffect { origin, .. }
            | Self::MissingSafepoint { origin }
            | Self::InconsistentInput { origin, .. } => Some(*origin),
            // This is an invocation/front-door selection error: neither side of
            // the requested root exists, so there is no source anchor to report.
            Self::MissingCompilationRoot { .. } => None,
        }
    }
}

impl Error for MirBuildError {}
