use std::collections::{BTreeMap, btree_map::Entry};
use std::fmt;

use vela_common::ShapeId;
use vela_def::{FieldId, FunctionId, GlobalId, MethodId, TypeId, VariantId};
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirPatternId};

use crate::{
    CompileFieldDescriptor, CompileFunctionDescriptor, CompileGlobalDescriptor,
    CompileMethodDescriptor, CompileTypeDescriptor, CompileVariantDescriptor, MirEvaluatedConstant,
    MirSourceOrigin, MirTargetTable, MirTypeContract,
};

use super::{
    CompileCallTarget, CompileFunctionIdentity, CompileFunctionTarget, CompileHostPathTarget,
    CompileTargetSnapshot, CompileTargetSnapshotBuilder, CompileTryTarget, HostFieldTarget,
    MethodExecutableTarget, MirBuildError,
};

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
pub struct CompileDynamicConstructorField {
    pub name: String,
    pub value: HirExprId,
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
    DynamicRecord {
        type_name: String,
        fields: Vec<CompileDynamicConstructorField>,
    },
    DynamicVariant {
        owner_name: String,
        variant_name: String,
        fields: Vec<CompileDynamicConstructorField>,
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
    DynamicRecord {
        type_name: String,
        fields: Vec<String>,
    },
    DynamicVariant {
        owner_name: String,
        variant_name: String,
        fields: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompileGuardKey {
    Expression {
        function: FunctionId,
        expression: HirExprId,
    },
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
    Try,
}

impl fmt::Display for CompileTargetKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Call => "call",
            Self::Member => "member",
            Self::Constructor => "constructor",
            Self::HostPath => "host path",
            Self::Try => "try",
        })
    }
}

#[derive(Clone, Copy)]
pub struct CompileFunctionTargets<'a> {
    snapshot: &'a CompileTargetSnapshot,
    root: &'a CompileFunctionTarget,
}

impl<'a> CompileFunctionTargets<'a> {
    pub(super) const fn new(
        snapshot: &'a CompileTargetSnapshot,
        root: &'a CompileFunctionTarget,
    ) -> Self {
        Self { snapshot, root }
    }

    #[must_use]
    pub const fn identity(self) -> CompileFunctionIdentity {
        self.root.identity
    }

    #[must_use]
    pub const fn function(self) -> FunctionId {
        self.identity().function()
    }

    #[must_use]
    pub fn function_target(self) -> &'a CompileFunctionTarget {
        self.root
    }

    #[must_use]
    pub fn call(self, expression: HirExprId) -> Option<&'a CompileCallTarget> {
        self.snapshot.call(self.function(), expression)
    }

    #[must_use]
    pub fn member(self, expression: HirExprId) -> Option<&'a CompileMemberTarget> {
        self.snapshot.member(self.function(), expression)
    }

    #[must_use]
    pub fn constructor(self, expression: HirExprId) -> Option<&'a CompileConstructorTarget> {
        self.snapshot.constructor(self.function(), expression)
    }

    #[must_use]
    pub fn pattern_constructor(
        self,
        pattern: HirPatternId,
    ) -> Option<&'a CompilePatternConstructorTarget> {
        self.snapshot.pattern_constructor(self.function(), pattern)
    }

    #[must_use]
    pub fn host_path(self, expression: HirExprId) -> Option<&'a CompileHostPathTarget> {
        self.snapshot.host_path(self.function(), expression)
    }

    #[must_use]
    pub fn try_target(self, expression: HirExprId) -> Option<&'a CompileTryTarget> {
        self.snapshot.try_target(self.function(), expression)
    }

    #[must_use]
    pub fn expression_guard(self, expression: HirExprId) -> Option<&'a CompileGuardTarget> {
        self.snapshot.guard(CompileGuardKey::Expression {
            function: self.function(),
            expression,
        })
    }

    #[must_use]
    pub fn parameter_guard(self, parameter: u32) -> Option<&'a CompileGuardTarget> {
        self.snapshot.guard(CompileGuardKey::Parameter {
            function: self.function(),
            parameter,
        })
    }

    #[must_use]
    pub fn return_guard(self) -> Option<&'a CompileGuardTarget> {
        self.snapshot
            .guard(CompileGuardKey::Return(self.function()))
    }

    #[must_use]
    pub fn global_guard(self, declaration: HirDeclId) -> Option<&'a CompileGuardTarget> {
        self.snapshot.guard(CompileGuardKey::Global(declaration))
    }

    #[must_use]
    pub fn field_guard(self, field: FieldId) -> Option<&'a CompileGuardTarget> {
        self.snapshot.guard(CompileGuardKey::Field(field))
    }

    #[must_use]
    pub fn global(self, declaration: HirDeclId) -> Option<&'a CompileGlobalDescriptor> {
        self.snapshot.global(declaration)
    }

    #[must_use]
    pub fn global_by_id(self, id: GlobalId) -> Option<&'a CompileGlobalDescriptor> {
        self.snapshot.global_by_id(id)
    }

    #[must_use]
    pub fn evaluated_constant(self, declaration: HirDeclId) -> Option<&'a MirEvaluatedConstant> {
        self.snapshot.evaluated_constant(declaration)
    }

    #[must_use]
    pub fn evaluated_schema_default(self, body: HirBodyId) -> Option<&'a MirEvaluatedConstant> {
        self.snapshot.evaluated_schema_default(body)
    }

    #[must_use]
    pub fn function_descriptor(
        self,
        function: FunctionId,
    ) -> Option<&'a CompileFunctionDescriptor> {
        self.snapshot.function_descriptor(function)
    }

    #[must_use]
    pub fn method_descriptor(
        self,
        owner: TypeId,
        method: MethodId,
    ) -> Option<&'a CompileMethodDescriptor> {
        self.snapshot.method_descriptor(owner, method)
    }

    #[must_use]
    pub fn type_descriptor(self, type_id: TypeId) -> Option<&'a CompileTypeDescriptor> {
        self.snapshot.type_descriptor(type_id)
    }

    #[must_use]
    pub fn variant_descriptor(self, variant: VariantId) -> Option<&'a CompileVariantDescriptor> {
        self.snapshot.variant_descriptor(variant)
    }

    #[must_use]
    pub fn field_descriptor(self, field: FieldId) -> Option<&'a CompileFieldDescriptor> {
        self.snapshot.field_descriptor(field)
    }

    #[must_use]
    pub const fn target_table(self) -> &'a MirTargetTable {
        self.snapshot.target_table()
    }
}

impl CompileTargetSnapshot {
    #[must_use]
    pub fn function_targets(&self, function: FunctionId) -> Option<CompileFunctionTargets<'_>> {
        self.function(function)
            .map(|root| CompileFunctionTargets::new(self, root))
    }

    #[must_use]
    pub fn call(&self, function: FunctionId, expression: HirExprId) -> Option<&CompileCallTarget> {
        self.calls.get(&(function, expression))
    }

    #[must_use]
    pub fn member(
        &self,
        function: FunctionId,
        expression: HirExprId,
    ) -> Option<&CompileMemberTarget> {
        self.members.get(&(function, expression))
    }

    #[must_use]
    pub fn constructor(
        &self,
        function: FunctionId,
        expression: HirExprId,
    ) -> Option<&CompileConstructorTarget> {
        self.constructors.get(&(function, expression))
    }

    #[must_use]
    pub fn pattern_constructor(
        &self,
        function: FunctionId,
        pattern: HirPatternId,
    ) -> Option<&CompilePatternConstructorTarget> {
        self.pattern_constructors.get(&(function, pattern))
    }

    #[must_use]
    pub fn host_path(
        &self,
        function: FunctionId,
        expression: HirExprId,
    ) -> Option<&CompileHostPathTarget> {
        self.host_paths.get(&(function, expression))
    }

    #[must_use]
    pub fn guard(&self, key: CompileGuardKey) -> Option<&CompileGuardTarget> {
        self.guards.get(&key)
    }
}

impl CompileTargetSnapshotBuilder {
    pub fn insert_call(
        &mut self,
        function: FunctionId,
        expression: HirExprId,
        target: CompileCallTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        insert_expression_target(
            &mut self.snapshot.calls,
            function,
            expression,
            target,
            CompileTargetKind::Call,
            origin,
        )?;
        self.snapshot
            .origins
            .calls
            .insert((function, expression), origin);
        Ok(())
    }

    pub fn insert_member(
        &mut self,
        function: FunctionId,
        expression: HirExprId,
        target: CompileMemberTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        insert_expression_target(
            &mut self.snapshot.members,
            function,
            expression,
            target,
            CompileTargetKind::Member,
            origin,
        )?;
        self.snapshot
            .origins
            .members
            .insert((function, expression), origin);
        Ok(())
    }

    pub fn insert_constructor(
        &mut self,
        function: FunctionId,
        expression: HirExprId,
        target: CompileConstructorTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        insert_expression_target(
            &mut self.snapshot.constructors,
            function,
            expression,
            target,
            CompileTargetKind::Constructor,
            origin,
        )?;
        self.snapshot
            .origins
            .constructors
            .insert((function, expression), origin);
        Ok(())
    }

    pub fn insert_pattern_constructor(
        &mut self,
        function: FunctionId,
        pattern: HirPatternId,
        target: CompilePatternConstructorTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        match self
            .snapshot
            .pattern_constructors
            .entry((function, pattern))
        {
            Entry::Vacant(entry) => {
                entry.insert(target);
                self.snapshot
                    .origins
                    .pattern_constructors
                    .insert((function, pattern), origin);
                Ok(())
            }
            Entry::Occupied(_) => Err(MirBuildError::DuplicatePatternConstructor {
                function,
                pattern,
                origin,
            }),
        }
    }

    pub fn insert_host_path(
        &mut self,
        function: FunctionId,
        expression: HirExprId,
        target: CompileHostPathTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        insert_expression_target(
            &mut self.snapshot.host_paths,
            function,
            expression,
            target,
            CompileTargetKind::HostPath,
            origin,
        )?;
        self.snapshot
            .origins
            .host_paths
            .insert((function, expression), origin);
        Ok(())
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
                self.snapshot.origins.guards.insert(key, origin);
                Ok(())
            }
            Entry::Occupied(_) => Err(MirBuildError::DuplicateGuardTarget { key, origin }),
        }
    }
}

fn insert_expression_target<T>(
    targets: &mut BTreeMap<(FunctionId, HirExprId), T>,
    function: FunctionId,
    expression: HirExprId,
    target: T,
    kind: CompileTargetKind,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    match targets.entry((function, expression)) {
        Entry::Vacant(entry) => {
            entry.insert(target);
            Ok(())
        }
        Entry::Occupied(_) => Err(MirBuildError::DuplicateCompileTarget {
            function,
            kind,
            expression,
            origin,
        }),
    }
}
