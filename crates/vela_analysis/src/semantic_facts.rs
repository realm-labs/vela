use std::collections::{BTreeMap, BTreeSet};

mod callbacks;
mod control_flow;
mod local_flow;
mod logical_records;
mod lookups;
mod patterns;
mod script_types;
mod targets;

#[cfg(test)]
mod local_flow_tests;

use control_flow::{block_flow, fallthrough_flow, if_flow, match_flow, statement_flow};
use logical_records::{
    logical_member_target, logical_record_constructor_fact, logical_record_constructor_target,
};
use lookups::{
    binary_fact, call_return_fact, field_fact, index_fact, literal_fact, registry_method_effect,
    registry_method_fact, resolved_literal_type, schema_knows_owner, source_method,
    try_payload_fact, type_owner,
};
use patterns::{pattern_constructor_target, pattern_local_facts};
pub(crate) use targets::registry_callable_owner;
pub use targets::{
    CallTargetFact, ConstructorTargetFact, HostPathIndexKindFact, HostPathSegmentFact,
    HostPathTargetFact, MemberTargetFact, OperatorTargetFact, ScriptTypeTargetFact,
};
use targets::{direct_lambda_body, registry_field_owner, source_field_fact};

use vela_common::PrimitiveTag;
use vela_hir::binding::{BindingResolution, ConstructorResolution};
use vela_hir::body::{
    HirBody, HirBodyRoot, HirElseBranch, HirExprKind, HirMatchArmBody, HirPathKind, HirPathOwner,
    HirPatternKind, HirStmtKind,
};
use vela_hir::ids::{HirBlockId, HirBodyId, HirExprId, HirLocalId, HirPatternId, HirStmtId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};

use crate::facts::AnalysisFacts;
use crate::logical_records::LogicalRecordKind;
use crate::registry::{RegistryEffectFact, RegistryFacts};
use crate::stdlib::{stdlib_function_fact, stdlib_method_fact};
use crate::type_fact::TypeFact;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ControlFlowFact {
    pub can_fallthrough: bool,
    pub may_return: bool,
    pub may_break: bool,
    pub may_continue: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirSemanticFacts {
    types: BTreeMap<HirExprId, TypeFact>,
    local_use_types: BTreeMap<HirExprId, TypeFact>,
    locals: BTreeMap<HirLocalId, TypeFact>,
    script_types: BTreeMap<HirExprId, ScriptTypeTargetFact>,
    local_script_types: BTreeMap<HirLocalId, ScriptTypeTargetFact>,
    patterns: BTreeMap<HirPatternId, TypeFact>,
    logical_record_constructors: BTreeMap<HirExprId, LogicalRecordKind>,
    calls: BTreeMap<HirExprId, CallTargetFact>,
    members: BTreeMap<HirExprId, MemberTargetFact>,
    operators: BTreeMap<HirExprId, OperatorTargetFact>,
    constructors: BTreeMap<HirExprId, ConstructorTargetFact>,
    pattern_constructors: BTreeMap<HirPatternId, ConstructorTargetFact>,
    host_paths: BTreeMap<HirExprId, HostPathTargetFact>,
    effects: BTreeMap<HirExprId, RegistryEffectFact>,
    control_flow: BTreeMap<HirExprId, ControlFlowFact>,
    block_control_flow: BTreeMap<HirBlockId, ControlFlowFact>,
    statement_control_flow: BTreeMap<HirStmtId, ControlFlowFact>,
}

impl HirSemanticFacts {
    pub(crate) fn from_module_graph(
        graph: &ModuleGraph,
        schema: Option<&RegistryFacts>,
        base: &AnalysisFacts,
    ) -> Self {
        Self::from_module_graph_with_body_filter(graph, schema, base, None)
    }

    pub(crate) fn from_module_graph_for_bodies(
        graph: &ModuleGraph,
        schema: Option<&RegistryFacts>,
        base: &AnalysisFacts,
        bodies: &BTreeSet<HirBodyId>,
    ) -> Self {
        Self::from_module_graph_with_body_filter(graph, schema, base, Some(bodies))
    }

    pub(crate) fn totalize_executable_scope(
        &mut self,
        graph: &ModuleGraph,
        bodies: &BTreeSet<HirBodyId>,
    ) {
        for body in bodies.iter().filter_map(|body| graph.body(*body)) {
            for expression in body.expressions.keys() {
                self.types.entry(*expression).or_insert(TypeFact::Unknown);
                self.effects
                    .entry(*expression)
                    .or_insert_with(RegistryEffectFact::pure);
            }
            for local in body
                .locals
                .iter()
                .copied()
                .chain(body.params.iter().map(|param| param.local))
                .chain(body.self_binding)
            {
                self.locals.entry(local).or_insert(TypeFact::Unknown);
            }
            for pattern in body.patterns.keys() {
                self.patterns.entry(*pattern).or_insert(TypeFact::Unknown);
            }
        }
    }

    fn from_module_graph_with_body_filter(
        graph: &ModuleGraph,
        schema: Option<&RegistryFacts>,
        base: &AnalysisFacts,
        selected: Option<&BTreeSet<HirBodyId>>,
    ) -> Self {
        let mut facts = Self::default();
        facts
            .types
            .extend(base.expressions().map(|(id, fact)| (id, fact.clone())));
        facts
            .locals
            .extend(base.locals().map(|(id, fact)| (id, fact.clone())));
        let bodies = graph
            .bodies()
            .filter(|body| selected.is_none_or(|selected| selected.contains(&body.id)))
            .collect::<Vec<_>>();
        let passes = bodies
            .iter()
            .map(|body| body.expressions.len())
            .sum::<usize>()
            .max(1);
        for body in &bodies {
            facts.record_logical_record_constructor_targets(graph, body);
        }
        for _ in 0..passes {
            let before = facts.types.clone();
            let local_uses_before = facts.local_use_types.clone();
            let locals_before = facts.locals.clone();
            let script_types_before = facts.script_types.clone();
            let local_script_types_before = facts.local_script_types.clone();
            for body in &bodies {
                facts.infer_local_facts(graph, body, schema, base);
                facts.record_local_use_facts(graph, body, schema, base);
                for expression in body.expressions.values().rev() {
                    if let Some(target) = facts.infer_script_type(graph, body, expression.id, base)
                    {
                        facts.script_types.insert(expression.id, target);
                    }
                    let fact = facts.infer_expression(graph, body, expression.id, schema, base);
                    if matches!(fact, TypeFact::Unknown) {
                        facts.types.remove(&expression.id);
                    } else {
                        facts.types.insert(expression.id, fact);
                    }
                    facts.record_targets(graph, body, expression.id, schema, base);
                }
                facts.infer_local_script_types(body);
                facts.record_patterns(graph, body, schema, base);
            }
            for body in &bodies {
                facts.infer_callback_params(graph, body, schema, base);
            }
            if facts.types == before
                && facts.local_use_types == local_uses_before
                && facts.locals == locals_before
                && facts.script_types == script_types_before
                && facts.local_script_types == local_script_types_before
            {
                break;
            }
        }
        for body in bodies {
            facts.record_body_control_flow(body);
        }
        facts
    }

    #[must_use]
    pub fn type_fact(&self, expression: HirExprId) -> Option<&TypeFact> {
        self.types.get(&expression)
    }

    #[must_use]
    pub fn local(&self, local: HirLocalId) -> Option<&TypeFact> {
        self.locals.get(&local)
    }

    #[must_use]
    pub fn script_type(&self, expression: HirExprId) -> Option<&ScriptTypeTargetFact> {
        self.script_types.get(&expression)
    }

    #[must_use]
    pub fn local_script_type(&self, local: HirLocalId) -> Option<&ScriptTypeTargetFact> {
        self.local_script_types.get(&local)
    }

    #[must_use]
    pub fn pattern(&self, pattern: HirPatternId) -> Option<&TypeFact> {
        self.patterns.get(&pattern)
    }

    #[must_use]
    pub fn logical_record_constructor(&self, expression: HirExprId) -> Option<LogicalRecordKind> {
        self.logical_record_constructors.get(&expression).copied()
    }

    #[must_use]
    pub fn call_target(&self, expression: HirExprId) -> Option<&CallTargetFact> {
        self.calls.get(&expression)
    }

    #[must_use]
    pub fn member_target(&self, expression: HirExprId) -> Option<&MemberTargetFact> {
        self.members.get(&expression)
    }

    #[must_use]
    pub fn operator_target(&self, expression: HirExprId) -> Option<OperatorTargetFact> {
        self.operators.get(&expression).copied()
    }

    #[must_use]
    pub fn constructor_target(&self, expression: HirExprId) -> Option<&ConstructorTargetFact> {
        self.constructors.get(&expression)
    }

    #[must_use]
    pub fn pattern_constructor_target(
        &self,
        pattern: HirPatternId,
    ) -> Option<&ConstructorTargetFact> {
        self.pattern_constructors.get(&pattern)
    }

    #[must_use]
    pub fn host_path_target(&self, expression: HirExprId) -> Option<&HostPathTargetFact> {
        self.host_paths.get(&expression)
    }

    #[must_use]
    pub fn effect(&self, expression: HirExprId) -> Option<&RegistryEffectFact> {
        self.effects.get(&expression)
    }

    #[must_use]
    pub fn control_flow(&self, expression: HirExprId) -> Option<&ControlFlowFact> {
        self.control_flow.get(&expression)
    }

    #[must_use]
    pub fn block_control_flow(&self, block: HirBlockId) -> Option<&ControlFlowFact> {
        self.block_control_flow.get(&block)
    }

    #[must_use]
    pub fn statement_control_flow(&self, statement: HirStmtId) -> Option<&ControlFlowFact> {
        self.statement_control_flow.get(&statement)
    }

    fn fact(&self, id: HirExprId) -> TypeFact {
        self.types.get(&id).cloned().unwrap_or(TypeFact::Unknown)
    }

    fn infer_expression(
        &self,
        graph: &ModuleGraph,
        body: &HirBody,
        id: HirExprId,
        schema: Option<&RegistryFacts>,
        base: &AnalysisFacts,
    ) -> TypeFact {
        let Some(expression) = body.expressions.get(&id) else {
            return TypeFact::Unknown;
        };
        if let Some(fact) = base.literal(id).map(resolved_literal_type) {
            return fact;
        }
        match &expression.kind {
            HirExprKind::Literal(literal) => literal_fact(literal),
            HirExprKind::Path(_) => match base.resolution(id) {
                Some(BindingResolution::Local(local)) => self
                    .local_use_types
                    .get(&id)
                    .cloned()
                    .or_else(|| self.locals.get(local).cloned())
                    .unwrap_or(TypeFact::Unknown),
                _ => base
                    .base_expression(id)
                    .cloned()
                    .unwrap_or(TypeFact::Unknown),
            },
            HirExprKind::Record { fields, .. } => self
                .logical_record_constructors
                .get(&id)
                .copied()
                .and_then(|kind| {
                    logical_record_constructor_fact(kind, fields, |expression| {
                        self.fact(expression)
                    })
                })
                .or_else(|| base.base_expression(id).cloned())
                .unwrap_or(TypeFact::Unknown),
            HirExprKind::Paren { expression } => {
                expression.map_or(TypeFact::Unknown, |id| self.fact(id))
            }
            HirExprKind::Try { expression } => {
                expression.map_or(TypeFact::Unknown, |id| try_payload_fact(self.fact(id)))
            }
            HirExprKind::Unit => TypeFact::UNIT,
            HirExprKind::Tuple { elements } => {
                TypeFact::tuple(elements.iter().map(|id| self.fact(*id)))
            }
            HirExprKind::Unary { op, operand } => {
                if matches!(op, Some(vela_hir::body::HirUnaryOp::Not)) {
                    TypeFact::BOOL
                } else {
                    operand.map_or(TypeFact::Unknown, |id| self.fact(id))
                }
            }
            HirExprKind::Binary { op, lhs, rhs } => binary_fact(
                *op,
                lhs.map(|id| self.fact(id)),
                rhs.map(|id| self.fact(id)),
            ),
            HirExprKind::Assign { value, .. } => {
                value.map_or(TypeFact::Unknown, |id| self.fact(id))
            }
            HirExprKind::Field(field) => field_fact(
                graph,
                self.script_types.get(&field.receiver),
                &self.fact(field.receiver),
                &field.name,
                schema,
            ),
            HirExprKind::Call(call) => self.call_return(graph, body, call, schema),
            HirExprKind::Index(index) => index_fact(&self.fact(index.receiver), schema),
            HirExprKind::Array { elements } => {
                TypeFact::array(TypeFact::union(elements.iter().map(|id| self.fact(*id))))
            }
            HirExprKind::Map { entries } => TypeFact::map(
                TypeFact::union(
                    entries
                        .iter()
                        .map(|entry| entry.key.map_or(TypeFact::Unknown, |id| self.fact(id))),
                ),
                TypeFact::union(
                    entries
                        .iter()
                        .map(|entry| entry.value.map_or(TypeFact::Unknown, |id| self.fact(id))),
                ),
            ),
            HirExprKind::Lambda { body } => graph.body(*body).map_or(TypeFact::Unknown, |body| {
                let params = body
                    .params
                    .iter()
                    .map(|param| {
                        self.locals
                            .get(&param.local)
                            .or_else(|| base.local(param.local))
                            .cloned()
                            .unwrap_or(TypeFact::Unknown)
                    })
                    .collect();
                TypeFact::function(params, self.body_value(body))
            }),
            HirExprKind::Block { block } => self.block_value(body, *block),
            HirExprKind::If(value) => TypeFact::union([
                value
                    .then_block
                    .map_or(TypeFact::UNIT, |block| self.block_value(body, block)),
                value
                    .else_branch
                    .as_ref()
                    .map_or(TypeFact::UNIT, |branch| match branch {
                        HirElseBranch::Block(block) => self.block_value(body, *block),
                        HirElseBranch::If(value) => {
                            let then_fact = value
                                .then_block
                                .map_or(TypeFact::UNIT, |block| self.block_value(body, block));
                            TypeFact::union([then_fact, TypeFact::UNIT])
                        }
                    }),
            ]),
            HirExprKind::Match(value) => TypeFact::union(value.arms.iter().map(|id| {
                body.match_arms
                    .get(id)
                    .map_or(TypeFact::Unknown, |arm| match arm.body {
                        Some(HirMatchArmBody::Expr(id)) => self.fact(id),
                        Some(HirMatchArmBody::Block(block)) => self.block_value(body, block),
                        None => TypeFact::UNIT,
                    })
            })),
            HirExprKind::Missing => TypeFact::Unknown,
        }
    }

    fn block_value(&self, body: &HirBody, block: vela_hir::ids::HirBlockId) -> TypeFact {
        let Some(statement) = body
            .blocks
            .get(&block)
            .and_then(|block| block.statements.last())
            .and_then(|statement| body.statements.get(statement))
        else {
            return TypeFact::UNIT;
        };
        match &statement.kind {
            HirStmtKind::Expr {
                expression: Some(id),
                terminated: false,
            } => self.fact(*id),
            HirStmtKind::If(value) => value
                .then_block
                .map_or(TypeFact::UNIT, |block| self.block_value(body, block)),
            HirStmtKind::Match(value) => TypeFact::union(value.arms.iter().map(|id| {
                body.match_arms
                    .get(id)
                    .map_or(TypeFact::Unknown, |arm| match arm.body {
                        Some(HirMatchArmBody::Expr(id)) => self.fact(id),
                        Some(HirMatchArmBody::Block(block)) => self.block_value(body, block),
                        None => TypeFact::UNIT,
                    })
            })),
            _ => TypeFact::UNIT,
        }
    }

    fn body_value(&self, body: &HirBody) -> TypeFact {
        match body.root {
            HirBodyRoot::Expr(id) => self.fact(id),
            HirBodyRoot::Block(block) => self.block_value(body, block),
            HirBodyRoot::Empty => TypeFact::UNIT,
        }
    }

    fn record_targets(
        &mut self,
        graph: &ModuleGraph,
        body: &HirBody,
        id: HirExprId,
        schema: Option<&RegistryFacts>,
        base: &AnalysisFacts,
    ) {
        let Some(expression) = body.expressions.get(&id) else {
            return;
        };
        self.effects
            .entry(id)
            .or_insert_with(RegistryEffectFact::pure);
        self.host_paths.remove(&id);
        if let Some(path) = self.host_path_for(body, id, schema) {
            self.host_paths.insert(id, path);
        }
        match &expression.kind {
            HirExprKind::Call(call) => {
                let target = if let Some(lambda) = direct_lambda_body(body, call.callee) {
                    CallTargetFact::Lambda(lambda)
                } else {
                    match base.resolution(call.callee) {
                        Some(BindingResolution::Declaration(declaration)) => {
                            let path = expression_path(body, call.callee, HirPathKind::Callee);
                            if graph
                                .declaration(*declaration)
                                .is_some_and(|decl| decl.kind == DeclarationKind::Enum)
                                && let Some(variant) = path.and_then(|path| path.last())
                            {
                                CallTargetFact::Variant {
                                    enum_declaration: *declaration,
                                    variant: variant.clone(),
                                }
                            } else {
                                CallTargetFact::Declaration(*declaration)
                            }
                        }
                        Some(BindingResolution::Local(id)) => CallTargetFact::Local(*id),
                        Some(BindingResolution::Import(_)) => CallTargetFact::Unresolved,
                        Some(BindingResolution::QualifiedPath(_)) => {
                            self.unbound_call_target(graph, body, id, call, schema)
                        }
                        None => self.unbound_call_target(graph, body, id, call, schema),
                    }
                };
                if let CallTargetFact::Variant {
                    enum_declaration,
                    variant,
                } = &target
                {
                    self.script_types.insert(
                        id,
                        ScriptTypeTargetFact {
                            declaration: *enum_declaration,
                            variant: Some(variant.clone()),
                        },
                    );
                }
                self.calls.insert(id, target);
            }
            HirExprKind::Field(field) => {
                let receiver = self.fact(field.receiver);
                let source_receiver = self.script_types.get(&field.receiver);
                let source_field = source_receiver
                    .and_then(|receiver| source_field_fact(graph, receiver, &field.name));
                let target = if let Some(target) = logical_member_target(&receiver, &field.name) {
                    target
                } else if let Some(field) = source_field {
                    MemberTargetFact::ScriptField {
                        owner: field.owner,
                        variant: field.variant,
                        name: field.name,
                    }
                } else if let Ok(index) = field.name.parse::<usize>() {
                    MemberTargetFact::TupleIndex(index)
                } else if let Some(owner) = registry_field_owner(&receiver) {
                    let host_field = matches!(receiver, TypeFact::Host { .. })
                        .then(|| {
                            schema.and_then(|schema| {
                                schema.host_field_target_fact(&owner, &field.name)
                            })
                        })
                        .flatten();
                    if let Some(target) = host_field {
                        MemberTargetFact::HostField(target.clone())
                    } else if !matches!(receiver, TypeFact::Host { .. })
                        && schema
                            .is_some_and(|schema| schema.field_fact(&owner, &field.name).is_some())
                    {
                        MemberTargetFact::RegistryField {
                            owner: owner.clone(),
                            name: field.name.clone(),
                        }
                    } else if schema.is_some_and(|schema| {
                        registry_method_fact(schema, &receiver, &field.name).is_some()
                    }) {
                        MemberTargetFact::RegistryMethod {
                            owner,
                            name: field.name.clone(),
                        }
                    } else if matches!(receiver, TypeFact::Record { .. })
                        && source_receiver.is_none()
                        && !schema.is_some_and(|schema| schema_knows_owner(schema, &owner))
                    {
                        MemberTargetFact::Dynamic
                    } else {
                        MemberTargetFact::Unresolved
                    }
                } else if stdlib_method_fact(&receiver, &field.name, None).is_some() {
                    MemberTargetFact::StdlibMethod {
                        name: field.name.clone(),
                    }
                } else if matches!(receiver, TypeFact::Any | TypeFact::Unknown) {
                    MemberTargetFact::Dynamic
                } else {
                    MemberTargetFact::Unresolved
                };
                self.members.insert(id, target);
            }
            HirExprKind::Unary { op, operand } => {
                let target = op.map_or(OperatorTargetFact::Unresolved, |op| {
                    if operand.is_some_and(|id| operator_fact_is_dynamic(&self.fact(id))) {
                        OperatorTargetFact::Dynamic
                    } else {
                        OperatorTargetFact::Unary(op)
                    }
                });
                self.operators.insert(id, target);
            }
            HirExprKind::Binary { op, lhs, rhs } => {
                let target = op.map_or(OperatorTargetFact::Unresolved, |op| {
                    if lhs
                        .iter()
                        .chain(rhs.iter())
                        .any(|id| operator_fact_is_dynamic(&self.fact(*id)))
                    {
                        OperatorTargetFact::Dynamic
                    } else {
                        OperatorTargetFact::Binary(op)
                    }
                });
                self.operators.insert(id, target);
            }
            HirExprKind::Assign { op, target, .. } => {
                let operator = op.map_or(OperatorTargetFact::Unresolved, |op| {
                    if target.is_some_and(|id| operator_fact_is_dynamic(&self.fact(id))) {
                        OperatorTargetFact::Dynamic
                    } else {
                        OperatorTargetFact::Assignment(op)
                    }
                });
                self.operators.insert(id, operator);
                if let Some(target) =
                    target.and_then(|target| self.host_path_for(body, target, schema))
                {
                    self.host_paths.insert(id, target);
                }
            }
            HirExprKind::Record { .. } => {
                let resolution = graph
                    .bindings_for_body(body.id)
                    .and_then(|bindings| bindings.constructor_resolution(id));
                let target = expression_path(body, id, HirPathKind::Constructor)
                    .map_or(ConstructorTargetFact::Unresolved, |path| {
                        constructor_target(graph, schema, path, resolution)
                    });
                match &target {
                    ConstructorTargetFact::Declaration(declaration) => {
                        self.script_types
                            .insert(id, ScriptTypeTargetFact::declaration(*declaration));
                    }
                    ConstructorTargetFact::Variant {
                        enum_declaration,
                        variant,
                    } => {
                        self.script_types.insert(
                            id,
                            ScriptTypeTargetFact {
                                declaration: *enum_declaration,
                                variant: Some(variant.clone()),
                            },
                        );
                    }
                    ConstructorTargetFact::RegistryType { .. }
                    | ConstructorTargetFact::RegistryVariant { .. }
                    | ConstructorTargetFact::Dynamic
                    | ConstructorTargetFact::Unresolved => {}
                }
                self.constructors.insert(id, target);
            }
            HirExprKind::Block { block } => {
                self.control_flow.insert(id, block_flow(body, *block));
            }
            HirExprKind::If(_) | HirExprKind::Match(_) | HirExprKind::Try { .. } => {
                self.control_flow.insert(id, fallthrough_flow());
            }
            _ => {}
        }
    }

    fn record_patterns(
        &mut self,
        graph: &ModuleGraph,
        body: &HirBody,
        schema: Option<&RegistryFacts>,
        base: &AnalysisFacts,
    ) {
        for pattern in body.patterns.values() {
            let fact = match &pattern.kind {
                HirPatternKind::Binding { local: Some(local) } => base
                    .local(*local)
                    .or_else(|| self.locals.get(local))
                    .cloned()
                    .unwrap_or(TypeFact::Unknown),
                HirPatternKind::Literal(Some(literal)) => base
                    .pattern_literal(pattern.id)
                    .map(resolved_literal_type)
                    .unwrap_or_else(|| literal_fact(literal)),
                _ => TypeFact::Unknown,
            };
            self.patterns.insert(pattern.id, fact);
            if let Some(target) = pattern_constructor_target(graph, schema, body, pattern.id) {
                self.pattern_constructors.insert(pattern.id, target);
            }
        }
    }

    fn record_logical_record_constructor_targets(&mut self, graph: &ModuleGraph, body: &HirBody) {
        for expression in body.expressions.values() {
            let HirExprKind::Record { constructor, .. } = &expression.kind else {
                continue;
            };
            if let Some(target) =
                logical_record_constructor_target(graph, body, expression.id, *constructor)
            {
                self.logical_record_constructors
                    .insert(expression.id, target);
            }
        }
    }

    fn infer_local_facts(
        &mut self,
        graph: &ModuleGraph,
        body: &HirBody,
        schema: Option<&RegistryFacts>,
        base: &AnalysisFacts,
    ) {
        let mut inferred = Vec::new();
        for statement in body.statements.values() {
            match &statement.kind {
                HirStmtKind::Let {
                    pattern: Some(pattern),
                    initializer: Some(initializer),
                    ..
                } => {
                    let fact = self.fact(*initializer);
                    inferred.extend(pattern_local_facts(
                        graph,
                        schema,
                        body,
                        *pattern,
                        &fact,
                        self.script_types.get(initializer),
                    ));
                }
                HirStmtKind::For {
                    patterns,
                    iterable: Some(iterable),
                    ..
                } => {
                    let item = iterable_item_fact(&self.fact(*iterable));
                    for (index, pattern) in patterns.iter().enumerate() {
                        let fact = if patterns.len() == 2 && index == 0 {
                            TypeFact::I64
                        } else {
                            item.clone()
                        };
                        inferred.extend(pattern_local_facts(
                            graph, schema, body, *pattern, &fact, None,
                        ));
                    }
                }
                HirStmtKind::Match(value) => {
                    infer_match_locals(self, graph, schema, body, value, &mut inferred);
                }
                _ => {}
            }
        }
        for expression in body.expressions.values() {
            if let HirExprKind::Match(value) = &expression.kind {
                infer_match_locals(self, graph, schema, body, value, &mut inferred);
            }
        }
        for inferred in inferred {
            if base.local(inferred.local).is_none() && !matches!(inferred.fact, TypeFact::Unknown) {
                if let Some(script_type) = inferred.script_type {
                    self.local_script_types.insert(inferred.local, script_type);
                }
                self.locals.insert(inferred.local, inferred.fact);
            }
        }
    }

    fn unbound_call_target(
        &mut self,
        graph: &ModuleGraph,
        body: &HirBody,
        call_id: HirExprId,
        call: &vela_hir::body::HirCall,
        schema: Option<&RegistryFacts>,
    ) -> CallTargetFact {
        if let Some(field) = body.field(call.callee) {
            let receiver = self.fact(field.receiver);
            let script_type = self.script_types.get(&field.receiver).cloned();
            if let Some(method) = source_method(graph, &receiver, &field.name) {
                return CallTargetFact::ScriptMethod {
                    method: method.node,
                };
            }
            if stdlib_method_fact(&receiver, &field.name, None).is_some() {
                return CallTargetFact::StdlibMethod {
                    name: field.name.clone(),
                };
            }
            if let Some(owner) = registry_callable_owner(&receiver)
                && let Some(schema) = schema
                && registry_method_fact(schema, &receiver, &field.name).is_some()
            {
                if let Some(effect) = registry_method_effect(schema, &receiver, &field.name) {
                    self.effects.insert(call_id, effect.clone());
                }
                return if matches!(receiver, TypeFact::Host { .. }) {
                    CallTargetFact::HostMethod {
                        owner: owner.to_owned(),
                        name: field.name.clone(),
                    }
                } else {
                    CallTargetFact::RegistryMethod {
                        owner: owner.to_owned(),
                        name: field.name.clone(),
                    }
                };
            }
            let registry_owner_is_closed = schema.is_some_and(|schema| {
                registry_callable_owner(&receiver)
                    .is_some_and(|owner| schema_knows_owner(schema, owner))
            });
            if script_type.is_some()
                || matches!(&receiver, TypeFact::LogicalRecord(_))
                || registry_owner_is_closed
            {
                return CallTargetFact::KnownReceiverMiss {
                    receiver,
                    script_type,
                    method: field.name.clone(),
                };
            }
            return CallTargetFact::Dynamic;
        }

        let Some(path) = expression_path(body, call.callee, HirPathKind::Callee) else {
            return CallTargetFact::Dynamic;
        };
        if let Some((variant, owner_path)) = path.split_last()
            && let Some(declaration) = source_declaration_for_path(graph, owner_path)
            && declaration.kind == DeclarationKind::Enum
        {
            return CallTargetFact::Variant {
                enum_declaration: declaration.id,
                variant: variant.clone(),
            };
        }
        let qualified = path.join("::");
        let args = call
            .arguments
            .iter()
            .map(|argument| argument.value.map_or(TypeFact::Unknown, |id| self.fact(id)))
            .collect::<Vec<_>>();
        if stdlib_function_fact(&qualified, &args).is_some() {
            return CallTargetFact::StdlibFunction { path: qualified };
        }
        if let Some(schema) = schema
            && schema.function_fact(&qualified).is_some()
        {
            if let Some(effect) = schema.function_effect_fact(&qualified) {
                self.effects.insert(call_id, effect.clone());
            }
            return if schema.function_origin(&qualified)
                == Some(vela_reflect::modules::DeclOrigin::Host)
            {
                CallTargetFact::NativeFunction { path: qualified }
            } else {
                CallTargetFact::RegistryFunction { path: qualified }
            };
        }
        CallTargetFact::Unresolved
    }

    fn call_return(
        &self,
        graph: &ModuleGraph,
        body: &HirBody,
        call: &vela_hir::body::HirCall,
        schema: Option<&RegistryFacts>,
    ) -> TypeFact {
        if let Some(field) = body.field(call.callee) {
            let receiver = self.fact(field.receiver);
            if let Some(method) = self.contextual_stdlib_method_fact(graph, body, call) {
                return method.returns;
            }
            let direct = call_return_fact(self.fact(call.callee));
            if !matches!(direct, TypeFact::Unknown) {
                return direct;
            }
            if let Some(method) = source_method(graph, &receiver, &field.name) {
                return method.returns;
            }
            if let Some(method) = stdlib_method_fact(&receiver, &field.name, None) {
                return method.returns;
            }
            if type_owner(&receiver).is_some()
                && let Some(method) =
                    schema.and_then(|schema| registry_method_fact(schema, &receiver, &field.name))
            {
                return call_return_fact(method.clone());
            }
            return TypeFact::Unknown;
        }
        let direct = call_return_fact(self.fact(call.callee));
        if !matches!(direct, TypeFact::Unknown) {
            return direct;
        }
        let Some(path) = expression_path(body, call.callee, HirPathKind::Callee) else {
            return TypeFact::Unknown;
        };
        let qualified = path.join("::");
        let args = call
            .arguments
            .iter()
            .map(|argument| argument.value.map_or(TypeFact::Unknown, |id| self.fact(id)))
            .collect::<Vec<_>>();
        stdlib_function_fact(&qualified, &args)
            .map(|fact| fact.returns)
            .or_else(|| {
                schema
                    .and_then(|schema| schema.function_fact(&qualified))
                    .cloned()
                    .map(call_return_fact)
            })
            .unwrap_or(TypeFact::Unknown)
    }

    fn host_path_for(
        &self,
        body: &HirBody,
        expression: HirExprId,
        schema: Option<&RegistryFacts>,
    ) -> Option<HostPathTargetFact> {
        let expression = body.expressions.get(&expression)?;
        match &expression.kind {
            HirExprKind::Path(_) if matches!(self.fact(expression.id), TypeFact::Host { .. }) => {
                let fact = self.fact(expression.id);
                let owner = type_owner(&fact)?;
                let root_type = schema?.type_target_fact(owner)?.clone();
                Some(HostPathTargetFact {
                    root: expression.id,
                    root_type,
                    segments: Vec::new(),
                })
            }
            HirExprKind::Paren {
                expression: Some(inner),
            } => self.host_path_for(body, *inner, schema),
            HirExprKind::Field(field) => {
                let mut path = self
                    .host_path_for(body, field.receiver, schema)
                    .or_else(|| {
                        let receiver = self.fact(field.receiver);
                        if !matches!(receiver, TypeFact::Host { .. }) {
                            return None;
                        }
                        let owner = type_owner(&receiver)?;
                        Some(HostPathTargetFact {
                            root: field.receiver,
                            root_type: schema?.type_target_fact(owner)?.clone(),
                            segments: Vec::new(),
                        })
                    })?;
                let owner = registry_field_owner(&self.fact(field.receiver))?;
                let target = schema?.host_field_target_fact(&owner, &field.name)?.clone();
                path.segments.push(HostPathSegmentFact::Field(target));
                Some(path)
            }
            HirExprKind::Index(index) => {
                let mut path = self.host_path_for(body, index.receiver, schema)?;
                let receiver = self.fact(index.receiver);
                let owner_name = type_owner(&receiver)?;
                let owner = schema?.type_target_fact(owner_name)?.clone();
                let capability = schema?.index_capability_fact(owner_name)?.clone();
                let kind = if capability.key == TypeFact::I64 {
                    HostPathIndexKindFact::Index
                } else {
                    HostPathIndexKindFact::Key
                };
                path.segments.push(HostPathSegmentFact::Index {
                    expression: index.index,
                    owner,
                    kind,
                    capability,
                });
                Some(path)
            }
            _ => None,
        }
    }

    fn record_body_control_flow(&mut self, body: &HirBody) {
        for statement in body.statements.values() {
            self.statement_control_flow
                .insert(statement.id, statement_flow(body, &statement.kind));
        }
        for block in body.blocks.values() {
            self.block_control_flow
                .insert(block.id, block_flow(body, block.id));
        }
        for expression in body.expressions.values() {
            let flow = match &expression.kind {
                HirExprKind::Block { block } => Some(block_flow(body, *block)),
                HirExprKind::If(value) => Some(if_flow(body, value)),
                HirExprKind::Match(value) => Some(match_flow(body, value)),
                HirExprKind::Try { .. } => Some(fallthrough_flow()),
                _ => None,
            };
            if let Some(flow) = flow {
                self.control_flow.insert(expression.id, flow);
            }
        }
    }
}

fn operator_fact_is_dynamic(fact: &TypeFact) -> bool {
    matches!(fact, TypeFact::Any | TypeFact::Unknown)
}

fn expression_path(body: &HirBody, expression: HirExprId, kind: HirPathKind) -> Option<&[String]> {
    body.paths
        .iter()
        .find(|path| path.kind == kind && path.owner == HirPathOwner::Expression(expression))
        .map(|path| path.path.as_slice())
}

fn infer_match_locals(
    facts: &HirSemanticFacts,
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    body: &HirBody,
    value: &vela_hir::body::HirMatch,
    inferred: &mut Vec<patterns::PatternLocalFact>,
) {
    let fact = value
        .scrutinee
        .map_or(TypeFact::Unknown, |id| facts.fact(id));
    let script_type = value
        .scrutinee
        .and_then(|scrutinee| facts.script_types.get(&scrutinee));
    for arm in &value.arms {
        if let Some(pattern) = body.match_arms.get(arm).and_then(|arm| arm.pattern) {
            inferred.extend(pattern_local_facts(
                graph,
                schema,
                body,
                pattern,
                &fact,
                script_type,
            ));
        }
    }
}

fn iterable_item_fact(fact: &TypeFact) -> TypeFact {
    match fact {
        TypeFact::Array { element }
        | TypeFact::Set { element }
        | TypeFact::Iterator { item: element } => (**element).clone(),
        TypeFact::Range => TypeFact::I64,
        TypeFact::Primitive(PrimitiveTag::String) => TypeFact::CHAR,
        TypeFact::Primitive(PrimitiveTag::Bytes) => TypeFact::U8,
        TypeFact::Any => TypeFact::Any,
        _ => TypeFact::Unknown,
    }
}

fn constructor_target(
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    path: &[String],
    resolution: Option<ConstructorResolution>,
) -> ConstructorTargetFact {
    if path.is_empty() {
        return ConstructorTargetFact::Unresolved;
    }
    if let Some(ConstructorResolution::Declaration(declaration)) = resolution {
        let Some(metadata) = graph.declaration(declaration) else {
            return ConstructorTargetFact::Unresolved;
        };
        return match metadata.kind {
            DeclarationKind::Struct => ConstructorTargetFact::Declaration(declaration),
            DeclarationKind::Enum if path.len() > 1 => ConstructorTargetFact::Variant {
                enum_declaration: declaration,
                variant: path.last().cloned().expect("non-empty constructor path"),
            },
            DeclarationKind::Enum => ConstructorTargetFact::Declaration(declaration),
            DeclarationKind::Const
            | DeclarationKind::Global
            | DeclarationKind::Function
            | DeclarationKind::Trait
            | DeclarationKind::Impl => ConstructorTargetFact::Unresolved,
        };
    }
    let Some(ConstructorResolution::Dynamic(dynamic_path)) = resolution else {
        return ConstructorTargetFact::Unresolved;
    };
    if dynamic_path.len() > 1 {
        let (variant, owner_path) = dynamic_path
            .split_last()
            .expect("non-empty dynamic constructor path");
        let owner = owner_path.join("::");
        if let Some(target) =
            schema.and_then(|schema| schema.variant_for_owner_or_unique_short_name(&owner, variant))
        {
            return ConstructorTargetFact::RegistryVariant {
                owner: target.owner,
                variant: target.name,
            };
        }
    }
    let qualified = dynamic_path.join("::");
    if schema.is_some_and(|schema| {
        schema.type_fact(&qualified).is_some()
            || dynamic_path
                .last()
                .is_some_and(|name| schema.type_fact(name).is_some())
    }) {
        return ConstructorTargetFact::RegistryType { path: qualified };
    }
    ConstructorTargetFact::Dynamic
}

fn source_declaration_for_path<'a>(
    graph: &'a ModuleGraph,
    path: &[String],
) -> Option<&'a vela_hir::module_graph::Declaration> {
    let name = path.last()?;
    let mut matches = graph
        .declarations_by_name(name)
        .into_iter()
        .filter(|declaration| {
            let Some(module) = graph.module_path(declaration.module) else {
                return false;
            };
            let mut qualified = module.segments().to_vec();
            qualified.push(declaration.name.clone());
            qualified == path || (path.len() == 1 && declaration.name == *name)
        });
    let declaration = matches.next()?;
    matches.next().is_none().then_some(declaration)
}

#[cfg(test)]
mod tests;
