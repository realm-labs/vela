use std::collections::{BTreeMap, BTreeSet};

use vela_hir::binding::BindingResolution;
use vela_hir::body::{
    HirBinaryOp, HirBody, HirBodyRoot, HirElseBranch, HirExprKind, HirIf,
    HirInterpolatedStringPart, HirLiteral, HirMatch, HirMatchArmBody, HirStmtKind,
};
use vela_hir::ids::{HirBlockId, HirExprId, HirLocalId};

use super::{HirSemanticFacts, iterable_item_fact, pattern_local_facts};
use crate::facts::AnalysisFacts;
use crate::registry::RegistryFacts;
use crate::type_fact::TypeFact;

type LocalEnvironment = BTreeMap<HirLocalId, TypeFact>;

impl HirSemanticFacts {
    pub(super) fn record_local_use_facts(
        &mut self,
        graph: &vela_hir::module_graph::ModuleGraph,
        body: &HirBody,
        schema: Option<&RegistryFacts>,
        base: &AnalysisFacts,
    ) {
        let mut flow = LocalFlow {
            graph,
            body,
            schema,
            base,
            expression_types: &self.types,
            uses: BTreeMap::new(),
        };
        let mut environment = self.locals.clone();
        flow.visit_root(&mut environment);
        for expression in body.expressions.keys() {
            self.local_use_types.remove(expression);
        }
        self.local_use_types.extend(flow.uses);
    }
}

struct LocalFlow<'facts> {
    graph: &'facts vela_hir::module_graph::ModuleGraph,
    body: &'facts HirBody,
    schema: Option<&'facts RegistryFacts>,
    base: &'facts AnalysisFacts,
    expression_types: &'facts BTreeMap<HirExprId, TypeFact>,
    uses: BTreeMap<HirExprId, TypeFact>,
}

impl LocalFlow<'_> {
    fn visit_root(&mut self, environment: &mut LocalEnvironment) {
        match self.body.root {
            HirBodyRoot::Block(block) => self.visit_block(block, environment),
            HirBodyRoot::Expr(expression) => self.visit_expression(expression, environment),
            HirBodyRoot::Empty => {}
        }
    }

    fn visit_block(&mut self, block: HirBlockId, environment: &mut LocalEnvironment) {
        let Some(block) = self.body.blocks.get(&block) else {
            return;
        };
        for statement in &block.statements {
            let Some(statement) = self.body.statements.get(statement) else {
                continue;
            };
            match &statement.kind {
                HirStmtKind::Let {
                    pattern,
                    initializer,
                    ..
                } => {
                    if let Some(initializer) = initializer {
                        self.visit_expression(*initializer, environment);
                    }
                    if let Some(pattern) = pattern {
                        let fact = initializer
                            .map(|initializer| self.fact(initializer, environment))
                            .unwrap_or(TypeFact::Unknown);
                        self.bind_pattern(*pattern, &fact, environment);
                    }
                }
                HirStmtKind::Return { value } => {
                    if let Some(value) = value {
                        self.visit_expression(*value, environment);
                    }
                }
                HirStmtKind::For {
                    patterns,
                    iterable,
                    body,
                } => {
                    if let Some(iterable) = iterable {
                        self.visit_expression(*iterable, environment);
                    }
                    let entry = environment.clone();
                    let mut iteration = entry.clone();
                    let item = iterable
                        .map(|iterable| iterable_item_fact(&self.fact(iterable, environment)))
                        .unwrap_or(TypeFact::Unknown);
                    for (index, pattern) in patterns.iter().enumerate() {
                        let fact = if patterns.len() == 2 && index == 0 {
                            TypeFact::I64
                        } else {
                            item.clone()
                        };
                        self.bind_pattern(*pattern, &fact, &mut iteration);
                    }
                    if let Some(body) = body {
                        self.visit_block(*body, &mut iteration);
                    }
                    *environment = join_environments([&entry, &iteration]);
                }
                HirStmtKind::If(value) => self.visit_if(value, environment),
                HirStmtKind::Match(value) => self.visit_match(value, environment),
                HirStmtKind::Block(block) => self.visit_block(*block, environment),
                HirStmtKind::Expr { expression, .. } => {
                    if let Some(expression) = expression {
                        self.visit_expression(*expression, environment);
                    }
                }
                HirStmtKind::Break | HirStmtKind::Continue => {}
            }
        }
    }

    fn visit_expression(&mut self, expression: HirExprId, environment: &mut LocalEnvironment) {
        let Some(value) = self.body.expressions.get(&expression) else {
            return;
        };
        match &value.kind {
            HirExprKind::Path(_) => {
                if let Some(BindingResolution::Local(local)) = self.base.resolution(expression) {
                    self.uses.insert(
                        expression,
                        environment.get(local).cloned().unwrap_or(TypeFact::Unknown),
                    );
                }
            }
            HirExprKind::Paren { expression }
            | HirExprKind::Try { expression }
            | HirExprKind::Unary {
                operand: expression,
                ..
            } => self.visit_optional(*expression, environment),
            HirExprKind::Tuple { elements } | HirExprKind::Array { elements } => {
                for element in elements {
                    self.visit_expression(*element, environment);
                }
            }
            HirExprKind::Binary { op, lhs, rhs } => {
                self.visit_optional(*lhs, environment);
                if matches!(op, Some(HirBinaryOp::And | HirBinaryOp::Or)) {
                    let skipped = environment.clone();
                    let mut evaluated = skipped.clone();
                    self.visit_optional(*rhs, &mut evaluated);
                    *environment = join_environments([&skipped, &evaluated]);
                } else {
                    self.visit_optional(*rhs, environment);
                }
            }
            HirExprKind::Assign { target, value, .. } => {
                self.visit_optional(*target, environment);
                self.visit_optional(*value, environment);
                if let Some(target) = target
                    && let Some(BindingResolution::Local(local)) = self.base.resolution(*target)
                {
                    let fact = self
                        .base
                        .local(*local)
                        .cloned()
                        .or_else(|| value.map(|value| self.fact(value, environment)))
                        .unwrap_or(TypeFact::Unknown);
                    set_local(environment, *local, fact);
                }
            }
            HirExprKind::Field(field) => self.visit_expression(field.receiver, environment),
            HirExprKind::Call(call) => {
                self.visit_expression(call.callee, environment);
                for argument in &call.arguments {
                    self.visit_optional(argument.value, environment);
                }
            }
            HirExprKind::Index(index) => {
                self.visit_expression(index.receiver, environment);
                self.visit_expression(index.index, environment);
            }
            HirExprKind::Map { entries } => {
                for entry in entries {
                    self.visit_optional(entry.key, environment);
                    self.visit_optional(entry.value, environment);
                }
            }
            HirExprKind::Record { fields, .. } => {
                for field in fields {
                    self.visit_optional(field.value, environment);
                }
            }
            HirExprKind::Block { block } => self.visit_block(*block, environment),
            HirExprKind::If(value) => self.visit_if(value, environment),
            HirExprKind::Match(value) => self.visit_match(value, environment),
            HirExprKind::Literal(HirLiteral::Interpolated { parts }) => {
                for part in parts {
                    if let HirInterpolatedStringPart::Expr(expression) = part {
                        self.visit_expression(*expression, environment);
                    }
                }
            }
            HirExprKind::Literal(_)
            | HirExprKind::Unit
            | HirExprKind::Lambda { .. }
            | HirExprKind::Missing => {}
        }
    }

    fn visit_if(&mut self, value: &HirIf, environment: &mut LocalEnvironment) {
        self.visit_optional(value.condition, environment);
        let entry = environment.clone();
        let mut then_environment = entry.clone();
        if let Some(block) = value.then_block {
            self.visit_block(block, &mut then_environment);
        }
        let mut else_environment = entry.clone();
        if let Some(branch) = &value.else_branch {
            match branch {
                HirElseBranch::If(value) => self.visit_if(value, &mut else_environment),
                HirElseBranch::Block(block) => self.visit_block(*block, &mut else_environment),
            }
        }
        *environment = join_environments([&then_environment, &else_environment]);
    }

    fn visit_match(&mut self, value: &HirMatch, environment: &mut LocalEnvironment) {
        self.visit_optional(value.scrutinee, environment);
        let entry = environment.clone();
        let scrutinee = value
            .scrutinee
            .map(|expression| self.fact(expression, environment))
            .unwrap_or(TypeFact::Unknown);
        let mut branches = vec![entry.clone()];
        for arm in &value.arms {
            let Some(arm) = self.body.match_arms.get(arm) else {
                continue;
            };
            let mut branch = entry.clone();
            if let Some(pattern) = arm.pattern {
                self.bind_pattern(pattern, &scrutinee, &mut branch);
            }
            self.visit_optional(arm.guard, &mut branch);
            match arm.body {
                Some(HirMatchArmBody::Expr(expression)) => {
                    self.visit_expression(expression, &mut branch);
                }
                Some(HirMatchArmBody::Block(block)) => self.visit_block(block, &mut branch),
                None => {}
            }
            branches.push(branch);
        }
        *environment = join_environments(branches.iter());
    }

    fn visit_optional(
        &mut self,
        expression: Option<HirExprId>,
        environment: &mut LocalEnvironment,
    ) {
        if let Some(expression) = expression {
            self.visit_expression(expression, environment);
        }
    }

    fn bind_pattern(
        &self,
        pattern: vela_hir::ids::HirPatternId,
        fact: &TypeFact,
        environment: &mut LocalEnvironment,
    ) {
        for inferred in pattern_local_facts(self.graph, self.schema, self.body, pattern, fact, None)
        {
            let fact = self
                .base
                .local(inferred.local)
                .cloned()
                .unwrap_or(inferred.fact);
            set_local(environment, inferred.local, fact);
        }
    }

    fn fact(&self, expression: HirExprId, environment: &LocalEnvironment) -> TypeFact {
        if let Some(BindingResolution::Local(local)) = self.base.resolution(expression) {
            return environment.get(local).cloned().unwrap_or(TypeFact::Unknown);
        }
        self.expression_types
            .get(&expression)
            .cloned()
            .unwrap_or(TypeFact::Unknown)
    }
}

fn set_local(environment: &mut LocalEnvironment, local: HirLocalId, fact: TypeFact) {
    if matches!(fact, TypeFact::Unknown) {
        environment.remove(&local);
    } else {
        environment.insert(local, fact);
    }
}

fn join_environments<'a>(
    environments: impl IntoIterator<Item = &'a LocalEnvironment>,
) -> LocalEnvironment {
    let environments = environments.into_iter().collect::<Vec<_>>();
    let locals = environments
        .iter()
        .flat_map(|environment| environment.keys().copied())
        .collect::<BTreeSet<_>>();
    let mut joined = LocalEnvironment::new();
    for local in locals {
        let Some(first) = environments
            .first()
            .and_then(|environment| environment.get(&local))
        else {
            continue;
        };
        if environments
            .iter()
            .all(|environment| environment.get(&local) == Some(first))
        {
            joined.insert(local, first.clone());
        }
    }
    joined
}
