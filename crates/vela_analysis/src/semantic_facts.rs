use std::collections::BTreeMap;

mod control_flow;
mod script_types;
mod targets;

use control_flow::{block_flow, fallthrough_flow, if_flow, match_flow, statement_flow};
pub use targets::{
    CallTargetFact, ConstructorTargetFact, HostPathIndexKindFact, HostPathSegmentFact,
    HostPathTargetFact, MemberTargetFact, OperatorTargetFact, ScriptTypeTargetFact,
};
use targets::{direct_lambda_body, registry_field_owner, source_field_fact};

use vela_common::PrimitiveTag;
use vela_hir::binding::BindingResolution;
use vela_hir::body::{
    HirBinaryOp, HirBody, HirBodyRoot, HirElseBranch, HirExprKind, HirLiteral, HirMatchArmBody,
    HirPathKind, HirPathOwner, HirPatternKind, HirStmtKind,
};
use vela_hir::ids::{HirBlockId, HirExprId, HirLocalId, HirNodeId, HirPatternId, HirStmtId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::ImplMetadataKind;

use crate::facts::AnalysisFacts;
use crate::hints::type_fact_from_hint_in_module;
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
    locals: BTreeMap<HirLocalId, TypeFact>,
    script_types: BTreeMap<HirExprId, ScriptTypeTargetFact>,
    local_script_types: BTreeMap<HirLocalId, ScriptTypeTargetFact>,
    patterns: BTreeMap<HirPatternId, TypeFact>,
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
        let mut facts = Self::default();
        facts
            .types
            .extend(base.expressions().map(|(id, fact)| (id, fact.clone())));
        facts
            .locals
            .extend(base.locals().map(|(id, fact)| (id, fact.clone())));
        let passes = graph
            .bodies()
            .map(|body| body.expressions.len())
            .sum::<usize>()
            .max(1);
        for _ in 0..passes {
            let before = facts.types.clone();
            let script_types_before = facts.script_types.clone();
            let local_script_types_before = facts.local_script_types.clone();
            for body in graph.bodies() {
                for expression in body.expressions.values().rev() {
                    if let Some(target) = facts.infer_script_type(graph, body, expression.id, base)
                    {
                        facts.script_types.insert(expression.id, target);
                    }
                    let fact = facts.infer_expression(graph, body, expression.id, schema, base);
                    if !matches!(fact, TypeFact::Unknown) {
                        facts.types.insert(expression.id, fact);
                    }
                    facts.record_targets(graph, body, expression.id, schema, base);
                }
                facts.infer_local_facts(body);
                facts.infer_local_script_types(body);
                facts.record_patterns(graph, body, schema, base);
            }
            if facts.types == before
                && facts.script_types == script_types_before
                && facts.local_script_types == local_script_types_before
            {
                break;
            }
        }
        for body in graph.bodies() {
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
        match &expression.kind {
            HirExprKind::Literal(literal) => literal_fact(literal),
            HirExprKind::Path(_) => match base.resolution(id) {
                Some(BindingResolution::Local(local)) => {
                    self.locals.get(local).cloned().unwrap_or(TypeFact::Unknown)
                }
                _ => base
                    .base_expression(id)
                    .cloned()
                    .unwrap_or(TypeFact::Unknown),
            },
            HirExprKind::Record { .. } => base
                .base_expression(id)
                .cloned()
                .unwrap_or(TypeFact::Unknown),
            HirExprKind::Paren { expression } | HirExprKind::Try { expression } => {
                expression.map_or(TypeFact::Unknown, |id| self.fact(id))
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
                        base.local(param.local)
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
                let source_field = self
                    .script_types
                    .get(&field.receiver)
                    .and_then(|receiver| source_field_fact(graph, receiver, &field.name));
                let target = if let Some(field) = source_field {
                    MemberTargetFact::ScriptField {
                        owner: field.owner,
                        variant: field.variant,
                        name: field.name,
                    }
                } else if let Ok(index) = field.name.parse::<usize>() {
                    MemberTargetFact::TupleIndex(index)
                } else if let Some(owner) = registry_field_owner(&receiver) {
                    if schema.is_some_and(|schema| schema.field_fact(&owner, &field.name).is_some())
                    {
                        if matches!(receiver, TypeFact::Host { .. }) {
                            schema
                                .and_then(|schema| schema.field_target_fact(&owner, &field.name))
                                .cloned()
                                .map_or(MemberTargetFact::Unresolved, MemberTargetFact::HostField)
                        } else {
                            MemberTargetFact::RegistryField {
                                owner: owner.clone(),
                                name: field.name.clone(),
                            }
                        }
                    } else if schema.is_some_and(|schema| {
                        registry_method_fact(schema, &receiver, &field.name).is_some()
                    }) {
                        MemberTargetFact::RegistryMethod {
                            owner,
                            name: field.name.clone(),
                        }
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
                if let Some(path) = self.host_path_for(body, id, schema) {
                    self.host_paths.insert(id, path);
                }
            }
            HirExprKind::Index(_) => {
                if let Some(path) = self.host_path_for(body, id, schema) {
                    self.host_paths.insert(id, path);
                }
            }
            HirExprKind::Unary { op, operand } => {
                let target = op.map_or(OperatorTargetFact::Unresolved, |op| {
                    if operand.is_some_and(|id| matches!(self.fact(id), TypeFact::Any)) {
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
                        .any(|id| matches!(self.fact(*id), TypeFact::Any))
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
                    if target.is_some_and(|id| matches!(self.fact(id), TypeFact::Any)) {
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
                let target = expression_path(body, id, HirPathKind::Constructor)
                    .map_or(ConstructorTargetFact::Unresolved, |path| {
                        constructor_target_for_path(graph, schema, path)
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
                HirPatternKind::Literal(Some(literal)) => literal_fact(literal),
                _ => TypeFact::Unknown,
            };
            self.patterns.insert(pattern.id, fact);
            let path = match &pattern.kind {
                HirPatternKind::TupleVariant { path, .. }
                | HirPatternKind::RecordVariant { path, .. }
                | HirPatternKind::Path { path } => path.and_then(|id| body.paths.get(&id)),
                _ => None,
            };
            if let Some(path) = path {
                self.pattern_constructors.insert(
                    pattern.id,
                    constructor_target_for_path(graph, schema, &path.path),
                );
            }
        }
    }

    fn infer_local_facts(&mut self, body: &HirBody) {
        let mut inferred = Vec::new();
        for statement in body.statements.values() {
            match &statement.kind {
                HirStmtKind::Let {
                    pattern: Some(pattern),
                    initializer: Some(initializer),
                    ..
                } => {
                    let fact = self.fact(*initializer);
                    inferred.extend(pattern_local_facts(body, *pattern, &fact));
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
                        inferred.extend(pattern_local_facts(body, *pattern, &fact));
                    }
                }
                HirStmtKind::Match(value) => {
                    infer_match_locals(self, body, value, &mut inferred);
                }
                _ => {}
            }
        }
        for expression in body.expressions.values() {
            if let HirExprKind::Match(value) = &expression.kind {
                infer_match_locals(self, body, value, &mut inferred);
            }
        }
        for (local, fact) in inferred {
            if !matches!(fact, TypeFact::Unknown) {
                self.locals.entry(local).or_insert(fact);
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
            if let Some(owner) = type_owner(&receiver)
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
            return CallTargetFact::Dynamic;
        }

        let Some(path) = expression_path(body, call.callee, HirPathKind::Callee) else {
            return CallTargetFact::Dynamic;
        };
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
        let direct = call_return_fact(self.fact(call.callee));
        if !matches!(direct, TypeFact::Unknown) {
            return direct;
        }
        if let Some(field) = body.field(call.callee) {
            let receiver = self.fact(field.receiver);
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
                let target = schema?.field_target_fact(&owner, &field.name)?.clone();
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

fn expression_path(body: &HirBody, expression: HirExprId, kind: HirPathKind) -> Option<&[String]> {
    body.paths
        .iter()
        .find(|path| path.kind == kind && path.owner == HirPathOwner::Expression(expression))
        .map(|path| path.path.as_slice())
}

fn pattern_local_facts(
    body: &HirBody,
    pattern: HirPatternId,
    fact: &TypeFact,
) -> Vec<(HirLocalId, TypeFact)> {
    let Some(pattern) = body.patterns.get(&pattern) else {
        return Vec::new();
    };
    match &pattern.kind {
        HirPatternKind::Binding { local } => {
            local.iter().map(|local| (*local, fact.clone())).collect()
        }
        HirPatternKind::TupleVariant { fields, .. } => {
            let elements = match fact {
                TypeFact::Tuple { elements } => Some(elements.as_slice()),
                _ => None,
            };
            fields
                .iter()
                .enumerate()
                .flat_map(|(index, pattern)| {
                    let fact = elements
                        .and_then(|elements| elements.get(index))
                        .unwrap_or(&TypeFact::Unknown);
                    pattern_local_facts(body, *pattern, fact)
                })
                .collect()
        }
        HirPatternKind::RecordVariant { fields, .. } => fields
            .iter()
            .filter_map(|field| field.pattern)
            .flat_map(|pattern| pattern_local_facts(body, pattern, &TypeFact::Unknown))
            .collect(),
        HirPatternKind::Path { .. }
        | HirPatternKind::Wildcard
        | HirPatternKind::Literal(_)
        | HirPatternKind::Missing => Vec::new(),
    }
}

fn infer_match_locals(
    facts: &HirSemanticFacts,
    body: &HirBody,
    value: &vela_hir::body::HirMatch,
    inferred: &mut Vec<(HirLocalId, TypeFact)>,
) {
    let fact = value
        .scrutinee
        .map_or(TypeFact::Unknown, |id| facts.fact(id));
    for arm in &value.arms {
        if let Some(pattern) = body.match_arms.get(arm).and_then(|arm| arm.pattern) {
            inferred.extend(pattern_local_facts(body, pattern, &fact));
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

fn constructor_target_for_path(
    graph: &ModuleGraph,
    schema: Option<&RegistryFacts>,
    path: &[String],
) -> ConstructorTargetFact {
    if path.is_empty() {
        return ConstructorTargetFact::Unresolved;
    }
    if path.len() > 1 {
        let (variant, owner_path) = path.split_last().expect("non-empty constructor path");
        if let Some(declaration) = source_declaration_for_path(graph, owner_path)
            && declaration.kind == DeclarationKind::Enum
        {
            return ConstructorTargetFact::Variant {
                enum_declaration: declaration.id,
                variant: variant.clone(),
            };
        }
        let owner = owner_path.join("::");
        if schema.is_some_and(|schema| schema.variant_fact(&owner, variant).is_some()) {
            return ConstructorTargetFact::RegistryVariant {
                owner,
                variant: variant.clone(),
            };
        }
    }
    if let Some(declaration) = source_declaration_for_path(graph, path) {
        return ConstructorTargetFact::Declaration(declaration.id);
    }
    let qualified = path.join("::");
    if schema.is_some_and(|schema| {
        schema.type_fact(&qualified).is_some()
            || path
                .last()
                .is_some_and(|name| schema.type_fact(name).is_some())
    }) {
        return ConstructorTargetFact::RegistryType { path: qualified };
    }
    ConstructorTargetFact::Unresolved
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

struct SourceMethodFact {
    node: HirNodeId,
    returns: TypeFact,
    return_target: Option<ScriptTypeTargetFact>,
}

fn source_method(graph: &ModuleGraph, receiver: &TypeFact, name: &str) -> Option<SourceMethodFact> {
    let owner = type_owner(receiver)?;
    for declaration in graph.declarations_by_kind(DeclarationKind::Impl) {
        let Some(metadata) = graph.impl_metadata(declaration.id) else {
            continue;
        };
        let target = metadata.target_path.join("::");
        if target != owner && !owner.ends_with(&format!("::{target}")) {
            continue;
        }
        if let Some(method) = metadata.methods.iter().find(|method| method.name == name) {
            let returns = method
                .signature
                .return_type
                .as_ref()
                .map_or(TypeFact::Unknown, |hint| {
                    type_fact_from_hint_in_module(graph, declaration.module, hint)
                });
            let return_target = method.signature.return_type.as_ref().and_then(|hint| {
                crate::hints::schema_declaration_from_hint_in_module(
                    graph,
                    declaration.module,
                    hint,
                )
                .map(ScriptTypeTargetFact::declaration)
            });
            return Some(SourceMethodFact {
                node: method.node,
                returns,
                return_target,
            });
        }
        let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
            continue;
        };
        let Some(trait_declaration) = source_declaration_for_path(graph, trait_path) else {
            continue;
        };
        let Some(shape) = graph.trait_shape(trait_declaration.id) else {
            continue;
        };
        if let Some(method) = shape.methods.iter().find(|method| method.name == name)
            && let Some(node) = method.default_body_node
        {
            let returns = method
                .signature
                .return_type
                .as_ref()
                .map_or(TypeFact::Unknown, |hint| {
                    type_fact_from_hint_in_module(graph, trait_declaration.module, hint)
                });
            let return_target = method.signature.return_type.as_ref().and_then(|hint| {
                crate::hints::schema_declaration_from_hint_in_module(
                    graph,
                    trait_declaration.module,
                    hint,
                )
                .map(ScriptTypeTargetFact::declaration)
            });
            return Some(SourceMethodFact {
                node,
                returns,
                return_target,
            });
        }
    }
    None
}

fn literal_fact(literal: &HirLiteral) -> TypeFact {
    match literal {
        HirLiteral::Bool(_) => TypeFact::BOOL,
        HirLiteral::Integer(value) => {
            TypeFact::primitive(crate::literals::integer_suffix_primitive(value.suffix))
        }
        HirLiteral::Float(value) => {
            TypeFact::primitive(crate::literals::float_suffix_primitive(value.suffix))
        }
        HirLiteral::Char(_) => TypeFact::CHAR,
        HirLiteral::String(_) | HirLiteral::Interpolated { .. } => TypeFact::STRING,
        HirLiteral::Bytes(_) => TypeFact::BYTES,
        HirLiteral::Invalid { .. } => TypeFact::Unknown,
    }
}

fn binary_fact(op: Option<HirBinaryOp>, lhs: Option<TypeFact>, rhs: Option<TypeFact>) -> TypeFact {
    match op {
        Some(
            HirBinaryOp::Equal
            | HirBinaryOp::NotEqual
            | HirBinaryOp::IdentityEqual
            | HirBinaryOp::IdentityNotEqual
            | HirBinaryOp::Less
            | HirBinaryOp::LessEqual
            | HirBinaryOp::Greater
            | HirBinaryOp::GreaterEqual
            | HirBinaryOp::And
            | HirBinaryOp::Or,
        ) => TypeFact::BOOL,
        Some(HirBinaryOp::Range | HirBinaryOp::RangeInclusive) => TypeFact::Range,
        _ => lhs
            .filter(|fact| !matches!(fact, TypeFact::Unknown))
            .or(rhs)
            .unwrap_or(TypeFact::Unknown),
    }
}

fn field_fact(
    graph: &ModuleGraph,
    source: Option<&ScriptTypeTargetFact>,
    receiver: &TypeFact,
    name: &str,
    schema: Option<&RegistryFacts>,
) -> TypeFact {
    if let TypeFact::Tuple { elements } = receiver
        && let Ok(index) = name.parse::<usize>()
    {
        return elements.get(index).cloned().unwrap_or(TypeFact::Unknown);
    }
    if let Some(field) = source.and_then(|source| source_field_fact(graph, source, name)) {
        return field.fact;
    }
    if let Some(method) = stdlib_method_fact(receiver, name, None) {
        return TypeFact::function(method.params, method.returns);
    }
    registry_field_owner(receiver)
        .as_deref()
        .and_then(|owner| {
            let schema = schema?;
            schema
                .field_fact(owner, name)
                .or_else(|| schema.method_fact(owner, name))
        })
        .cloned()
        .unwrap_or({
            if matches!(receiver, TypeFact::Any) {
                TypeFact::Any
            } else {
                TypeFact::Unknown
            }
        })
}

fn call_return_fact(callee: TypeFact) -> TypeFact {
    match callee {
        TypeFact::Function { returns, .. } => *returns,
        TypeFact::Any => TypeFact::Any,
        _ => TypeFact::Unknown,
    }
}

fn index_fact(receiver: &TypeFact, schema: Option<&RegistryFacts>) -> TypeFact {
    match receiver {
        TypeFact::Array { element } | TypeFact::Set { element } => (**element).clone(),
        TypeFact::Map { value, .. } => (**value).clone(),
        TypeFact::Tuple { elements } => TypeFact::union(elements.clone()),
        TypeFact::Primitive(PrimitiveTag::String) => TypeFact::CHAR,
        TypeFact::Primitive(PrimitiveTag::Bytes) => TypeFact::U8,
        TypeFact::Any => TypeFact::Any,
        _ => type_owner(receiver)
            .and_then(|owner| schema?.index_capability_fact(owner))
            .map_or(TypeFact::Unknown, |capability| capability.value.clone()),
    }
}

fn registry_method_fact<'a>(
    schema: &'a RegistryFacts,
    receiver: &TypeFact,
    method: &str,
) -> Option<&'a TypeFact> {
    let owner = type_owner(receiver)?;
    match receiver {
        TypeFact::Trait { .. } => schema
            .trait_method_fact(owner, method)
            .or_else(|| schema.method_fact(owner, method)),
        _ => schema
            .method_fact(owner, method)
            .or_else(|| schema.trait_method_fact(owner, method)),
    }
}

fn registry_method_effect<'a>(
    schema: &'a RegistryFacts,
    receiver: &TypeFact,
    method: &str,
) -> Option<&'a RegistryEffectFact> {
    let owner = type_owner(receiver)?;
    match receiver {
        TypeFact::Trait { .. } => schema
            .trait_method_effect_fact(owner, method)
            .or_else(|| schema.method_effect_fact(owner, method)),
        _ => schema
            .method_effect_fact(owner, method)
            .or_else(|| schema.trait_method_effect_fact(owner, method)),
    }
}

fn type_owner(fact: &TypeFact) -> Option<&str> {
    match fact {
        TypeFact::Record { name }
        | TypeFact::Enum { name, .. }
        | TypeFact::Host { name }
        | TypeFact::Trait { name } => Some(name),
        _ => None,
    }
}
