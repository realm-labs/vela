use std::collections::{BTreeMap, btree_map::Entry};
use std::error::Error;
use std::fmt;

use vela_analysis::facts::AnalysisFacts;
use vela_common::{HostMethodId, ShapeId};
use vela_def::{FieldId, FunctionId, GlobalId, MethodId, TypeId, VariantId};
use vela_hir::body::HirBodyOwner;
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirLocalId, HirNodeId, HirPatternId};
use vela_hir::module_graph::ModuleGraph;

use crate::{
    CompileFieldDescriptor, CompileFunctionClass, CompileFunctionDescriptor,
    CompileGlobalDescriptor, CompileMethodClass, CompileMethodDescriptor, CompileTypeDescriptor,
    CompileVariantDescriptor, HostTypeTarget, MirBlockId, MirEffect, MirEvaluatedConstant,
    MirLocalId, MirSourceOrigin, MirTargetTable, MirTempId, MirTypeContract,
};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostFieldTarget {
    pub owner: HostTypeTarget,
    pub semantic: FieldId,
    pub runtime: FieldId,
    pub readable: bool,
    pub writable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostMethodTarget {
    pub owner: HostTypeTarget,
    pub semantic: MethodId,
    pub runtime: HostMethodId,
    pub signature: CompileSignature,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileCallTarget {
    ScriptFunction {
        function: FunctionId,
        debug_name: String,
    },
    ScriptMethod {
        target: MethodExecutableTarget,
        debug_name: String,
    },
    Local(HirLocalId),
    Lambda(HirBodyId),
    NativeFunction {
        function: FunctionId,
        debug_name: String,
    },
    StdlibFunction {
        function: FunctionId,
        debug_name: String,
    },
    ValueMethod {
        owner: TypeId,
        method: MethodId,
        debug_name: String,
    },
    HostMethod(HostMethodTarget),
    Reflection {
        operation: CompileReflectionCall,
        function: FunctionId,
        debug_name: String,
    },
    SetFromArray {
        function: FunctionId,
        debug_name: String,
    },
    HostRemove {
        path: CompileHostPathTarget,
    },
    HostPush {
        path: CompileHostPathTarget,
    },
    DynamicCallable,
    DynamicMethod(DynamicMethodTarget),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileReflectionCall {
    Read,
    Write,
    Call,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileFieldTarget {
    RecordSlot {
        type_id: TypeId,
        shape: ShapeId,
        field: FieldId,
    },
    VariantSlot {
        type_id: TypeId,
        variant: VariantId,
        field: FieldId,
    },
    Dynamic {
        name: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileMemberTarget {
    ScriptField(CompileFieldTarget),
    HostField(HostFieldTarget),
    ScriptMethod {
        target: MethodExecutableTarget,
        debug_name: String,
    },
    ValueMethod {
        owner: TypeId,
        method: MethodId,
        debug_name: String,
    },
    TupleIndex(u32),
    Dynamic {
        name: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileConstructorField {
    pub field: FieldId,
    pub parameter: u32,
    pub parameter_name: String,
    pub value: CompileConstructorValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileConstructorValue {
    Explicit(HirExprId),
    EvaluatedDefault(HirBodyId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileConstructorTarget {
    Record {
        type_id: TypeId,
        shape: ShapeId,
        fields: Vec<CompileConstructorField>,
    },
    Variant {
        type_id: TypeId,
        variant: VariantId,
        fields: Vec<CompileConstructorField>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompilePatternConstructorTarget {
    Record {
        type_id: TypeId,
        shape: ShapeId,
        fields: Vec<FieldId>,
    },
    Variant {
        type_id: TypeId,
        variant: VariantId,
        fields: Vec<FieldId>,
    },
}

pub type CompileGlobalTarget = CompileGlobalDescriptor;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileParameter {
    pub name: String,
    pub contract: Option<MirTypeContract>,
    pub default: CompileParameterDefault,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileParameterDefault {
    Required,
    HirBody(HirBodyId),
    RuntimeProvided,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileSignature {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileHostPathSegment {
    Field(HostFieldTarget),
    ConstantIndex(u32),
    ConstantKey(String),
    DynamicIndex {
        expression: HirExprId,
        capability: CompileHostIndexCapability,
    },
    DynamicKey {
        expression: HirExprId,
        capability: CompileHostIndexCapability,
    },
    VariantField(HostFieldTarget),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileHostIndexCapability {
    pub readable: bool,
    pub writable: bool,
    pub mutable: bool,
    pub removable: bool,
    pub key: Option<MirTypeContract>,
    pub value: Option<MirTypeContract>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileHostPathTarget {
    pub root: HirExprId,
    pub root_type: HostTypeTarget,
    pub segments: Vec<CompileHostPathSegment>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompileGuardKey {
    Expression(HirExprId),
    Parameter {
        function: FunctionId,
        parameter: u32,
    },
    Return(FunctionId),
    Global(HirDeclId),
    Field(FieldId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileGuardTarget {
    pub contract: MirTypeContract,
    pub debug_name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileTargetKind {
    Call,
    Member,
    Constructor,
    HostPath,
}

impl fmt::Display for CompileTargetKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Call => "call",
            Self::Member => "member",
            Self::Constructor => "constructor",
            Self::HostPath => "host path",
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompileTargetSnapshot {
    functions: BTreeMap<FunctionId, CompileFunctionTarget>,
    functions_by_body: BTreeMap<HirBodyId, Vec<FunctionId>>,
    calls: BTreeMap<HirExprId, CompileCallTarget>,
    members: BTreeMap<HirExprId, CompileMemberTarget>,
    constructors: BTreeMap<HirExprId, CompileConstructorTarget>,
    pattern_constructors: BTreeMap<HirPatternId, CompilePatternConstructorTarget>,
    host_paths: BTreeMap<HirExprId, CompileHostPathTarget>,
    globals: BTreeMap<HirDeclId, GlobalId>,
    evaluated_constants: BTreeMap<HirDeclId, MirEvaluatedConstant>,
    evaluated_schema_defaults: BTreeMap<HirBodyId, MirEvaluatedConstant>,
    targets: MirTargetTable,
    guards: BTreeMap<CompileGuardKey, CompileGuardTarget>,
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
    pub fn call(&self, expression: HirExprId) -> Option<&CompileCallTarget> {
        self.calls.get(&expression)
    }

    #[must_use]
    pub fn member(&self, expression: HirExprId) -> Option<&CompileMemberTarget> {
        self.members.get(&expression)
    }

    #[must_use]
    pub fn constructor(&self, expression: HirExprId) -> Option<&CompileConstructorTarget> {
        self.constructors.get(&expression)
    }

    #[must_use]
    pub fn pattern_constructor(
        &self,
        pattern: HirPatternId,
    ) -> Option<&CompilePatternConstructorTarget> {
        self.pattern_constructors.get(&pattern)
    }

    #[must_use]
    pub fn host_path(&self, expression: HirExprId) -> Option<&CompileHostPathTarget> {
        self.host_paths.get(&expression)
    }

    #[must_use]
    pub fn global(&self, declaration: HirDeclId) -> Option<&CompileGlobalTarget> {
        self.globals
            .get(&declaration)
            .and_then(|id| self.targets.global(*id))
    }

    #[must_use]
    pub fn global_by_id(&self, id: GlobalId) -> Option<&CompileGlobalTarget> {
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

    #[must_use]
    pub fn guard(&self, key: CompileGuardKey) -> Option<&CompileGuardTarget> {
        self.guards.get(&key)
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
                Ok(())
            }
            Entry::Occupied(_) => Err(MirBuildError::DuplicateFunctionTarget { function, origin }),
        }
    }

    pub fn insert_call(
        &mut self,
        expression: HirExprId,
        target: CompileCallTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        insert_expression_target(
            &mut self.snapshot.calls,
            expression,
            target,
            CompileTargetKind::Call,
            origin,
        )
    }

    pub fn insert_member(
        &mut self,
        expression: HirExprId,
        target: CompileMemberTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        insert_expression_target(
            &mut self.snapshot.members,
            expression,
            target,
            CompileTargetKind::Member,
            origin,
        )
    }

    pub fn insert_constructor(
        &mut self,
        expression: HirExprId,
        target: CompileConstructorTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        insert_expression_target(
            &mut self.snapshot.constructors,
            expression,
            target,
            CompileTargetKind::Constructor,
            origin,
        )
    }

    pub fn insert_pattern_constructor(
        &mut self,
        pattern: HirPatternId,
        target: CompilePatternConstructorTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        match self.snapshot.pattern_constructors.entry(pattern) {
            Entry::Vacant(entry) => {
                entry.insert(target);
                Ok(())
            }
            Entry::Occupied(_) => {
                Err(MirBuildError::DuplicatePatternConstructor { pattern, origin })
            }
        }
    }

    pub fn insert_host_path(
        &mut self,
        expression: HirExprId,
        target: CompileHostPathTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        insert_expression_target(
            &mut self.snapshot.host_paths,
            expression,
            target,
            CompileTargetKind::HostPath,
            origin,
        )
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
        self.snapshot.globals.insert(declaration, target.id);
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
        if self.snapshot.targets.insert_type(descriptor) {
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
            Ok(())
        } else {
            Err(duplicate_descriptor("field", id.get(), origin))
        }
    }

    pub fn insert_guard(
        &mut self,
        key: CompileGuardKey,
        guard: CompileGuardTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        match self.snapshot.guards.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(guard);
                Ok(())
            }
            Entry::Occupied(_) => Err(MirBuildError::DuplicateGuardTarget { key, origin }),
        }
    }

    #[must_use]
    pub fn build(self) -> CompileTargetSnapshot {
        self.snapshot
    }
}

fn duplicate_descriptor(kind: &str, id: u128, origin: MirSourceOrigin) -> MirBuildError {
    MirBuildError::InconsistentInput {
        origin,
        message: format!("duplicate {kind} descriptor #{id}"),
    }
}

fn insert_expression_target<T>(
    targets: &mut BTreeMap<HirExprId, T>,
    expression: HirExprId,
    target: T,
    kind: CompileTargetKind,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    match targets.entry(expression) {
        Entry::Vacant(entry) => {
            entry.insert(target);
            Ok(())
        }
        Entry::Occupied(_) => Err(MirBuildError::DuplicateCompileTarget {
            kind,
            expression,
            origin,
        }),
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
    function: FunctionId,
    body: HirBodyId,
    analysis: &'a AnalysisFacts,
    targets: &'a CompileTargetSnapshot,
    config: MirLoweringConfig,
}

impl<'a> MirLoweringInput<'a> {
    pub fn new(
        graph: &'a ModuleGraph,
        function: FunctionId,
        body: HirBodyId,
        analysis: &'a AnalysisFacts,
        targets: &'a CompileTargetSnapshot,
        config: MirLoweringConfig,
    ) -> Result<Self, MirBuildError> {
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
        let origin = MirSourceOrigin::body(body, hir_body.origin.span);
        let inconsistent = |message| MirBuildError::InconsistentInput { origin, message };
        let function_descriptor = targets.function_descriptor(function).ok_or_else(|| {
            inconsistent(format!("missing function descriptor #{}", function.get()))
        })?;
        if function_descriptor.class != CompileFunctionClass::Script {
            return Err(inconsistent(format!(
                "HIR-backed function #{} is not classified as a script function",
                function.get()
            )));
        }
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
                CompileMethodClass::Value | CompileMethodClass::Registry => {
                    return Err(inconsistent(format!(
                        "method descriptor #{} is not a script method",
                        method.method.get()
                    )));
                }
            }
        }
        Ok(Self {
            graph,
            function,
            body,
            analysis,
            targets,
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
        self.function
    }

    #[must_use]
    pub const fn analysis(self) -> &'a AnalysisFacts {
        self.analysis
    }

    #[must_use]
    pub const fn targets(self) -> &'a CompileTargetSnapshot {
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
        kind: CompileTargetKind,
        expression: HirExprId,
        origin: MirSourceOrigin,
    },
    DuplicatePatternConstructor {
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
                kind, expression, ..
            } => {
                write!(formatter, "duplicate {kind} target for {expression:?}")
            }
            Self::DuplicatePatternConstructor { pattern, .. } => {
                write!(formatter, "duplicate constructor target for {pattern:?}")
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
            | Self::DuplicateMirFunctionId { origin, .. }
            | Self::DuplicateMirMethodId { origin, .. }
            | Self::MissingMirFunction { origin, .. }
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
