use super::{LocalEnvironment, LocalFlow, LocalValue, join_environments, set_local};
use crate::semantic_facts::refine_local_fact;
use crate::type_fact::TypeFact;
use vela_hir::binding::BindingResolution;
use vela_hir::body::{
    HirBinaryOp, HirBodyRoot, HirElseBranch, HirExprKind, HirIf, HirInterpolatedStringPart,
    HirLiteral, HirMatch, HirMatchArmBody, HirStmtKind,
};
use vela_hir::ids::{HirBlockId, HirExprId};

impl LocalFlow<'_> {
    pub(super) fn visit_root(&mut self, environment: &mut LocalEnvironment) -> bool {
        match self.body.root {
            HirBodyRoot::Block(block) => self.visit_block(block, environment),
            HirBodyRoot::Expr(expression) => self.visit_expression(expression, environment),
            HirBodyRoot::Empty => true,
        }
    }

    pub(super) fn visit_block(
        &mut self,
        block: HirBlockId,
        environment: &mut LocalEnvironment,
    ) -> bool {
        let Some(block) = self.body.blocks.get(&block) else {
            return true;
        };
        for id in &block.statements {
            if let Some(statement) = self.body.statements.get(id)
                && !self.visit_statement(&statement.kind, environment)
            {
                return false;
            }
        }
        true
    }

    fn visit_statement(
        &mut self,
        statement: &HirStmtKind,
        environment: &mut LocalEnvironment,
    ) -> bool {
        match statement {
            HirStmtKind::Let {
                pattern,
                initializer,
                ..
            } => {
                if !self.visit_optional(*initializer, environment) {
                    return false;
                }
                if let Some(pattern) = pattern {
                    let fact = initializer
                        .map(|id| self.fact(id, environment))
                        .unwrap_or(TypeFact::Unknown);
                    let origins = initializer
                        .map(|id| self.origins(id, environment))
                        .unwrap_or_default();
                    self.bind_pattern(*pattern, &fact, &origins, environment);
                }
                true
            }
            HirStmtKind::Return { value } => {
                self.visit_optional(*value, environment);
                false
            }
            HirStmtKind::Break | HirStmtKind::Continue => {
                if let Some(exits) = self.loop_exits.last_mut() {
                    match statement {
                        HirStmtKind::Break => exits.breaks.push(environment.clone()),
                        _ => exits.continues.push(environment.clone()),
                    }
                }
                false
            }
            HirStmtKind::For {
                patterns,
                iterable,
                body,
            } => self.visit_loop(patterns, *iterable, *body, environment),
            HirStmtKind::If(value) => self.visit_if(value, environment),
            HirStmtKind::Match(value) => self.visit_match(value, environment),
            HirStmtKind::Block(block) => self.visit_block(*block, environment),
            HirStmtKind::Expr { expression, .. } => self.visit_optional(*expression, environment),
        }
    }

    fn visit_expression(
        &mut self,
        expression: HirExprId,
        environment: &mut LocalEnvironment,
    ) -> bool {
        let Some(value) = self.body.expressions.get(&expression) else {
            return true;
        };
        match &value.kind {
            HirExprKind::Path(_) => {
                if let Some(BindingResolution::Local(local)) = self.base.resolution(expression) {
                    self.uses.insert(
                        expression,
                        environment.get(local).cloned().unwrap_or_else(|| {
                            LocalValue::new(TypeFact::Unknown, Default::default())
                        }),
                    );
                }
                true
            }
            HirExprKind::Paren { expression }
            | HirExprKind::Try { expression }
            | HirExprKind::Await { expression }
            | HirExprKind::Unary {
                operand: expression,
                ..
            } => self.visit_optional(*expression, environment),
            HirExprKind::Tuple { elements } | HirExprKind::Array { elements } => elements
                .iter()
                .all(|id| self.visit_expression(*id, environment)),
            HirExprKind::Binary { op, lhs, rhs } => {
                if !self.visit_optional(*lhs, environment) {
                    return false;
                }
                if matches!(op, Some(HirBinaryOp::And | HirBinaryOp::Or)) {
                    let skipped = environment.clone();
                    let mut evaluated = skipped.clone();
                    if self.visit_optional(*rhs, &mut evaluated) {
                        *environment = join_environments([&skipped, &evaluated], self.base);
                    }
                    true
                } else {
                    self.visit_optional(*rhs, environment)
                }
            }
            HirExprKind::Assign { target, value, .. } => {
                if !self.visit_optional(*target, environment)
                    || !self.visit_optional(*value, environment)
                {
                    return false;
                }
                if let Some(target) = target
                    && let Some(BindingResolution::Local(local)) = self.base.resolution(*target)
                {
                    let inferred = value
                        .map(|id| self.fact(id, environment))
                        .unwrap_or(TypeFact::Unknown);
                    let fact = self
                        .base
                        .base_local(*local)
                        .map_or(inferred.clone(), |declared| {
                            refine_local_fact(declared, inferred)
                        });
                    let origins = self
                        .base
                        .base_local_script_type(*local)
                        .cloned()
                        .map(super::ScriptTypeOrigins::known)
                        .unwrap_or_else(|| {
                            value
                                .map(|id| self.origins(id, environment))
                                .unwrap_or_default()
                        });
                    set_local(environment, *local, LocalValue::new(fact, origins));
                }
                true
            }
            HirExprKind::Field(field) => self.visit_expression(field.receiver, environment),
            HirExprKind::Call(call) => {
                self.visit_expression(call.callee, environment)
                    && call
                        .arguments
                        .iter()
                        .all(|arg| self.visit_optional(arg.value, environment))
            }
            HirExprKind::Index(index) => {
                self.visit_expression(index.receiver, environment)
                    && self.visit_expression(index.index, environment)
            }
            HirExprKind::Map { entries } => entries.iter().all(|entry| {
                self.visit_optional(entry.key, environment)
                    && self.visit_optional(entry.value, environment)
            }),
            HirExprKind::Record { fields, .. } => fields
                .iter()
                .all(|field| self.visit_optional(field.value, environment)),
            HirExprKind::Block { block } => self.visit_block(*block, environment),
            HirExprKind::If(value) => self.visit_if(value, environment),
            HirExprKind::Match(value) => self.visit_match(value, environment),
            HirExprKind::Literal(HirLiteral::Interpolated { parts }) => {
                parts.iter().all(|part| match part {
                    HirInterpolatedStringPart::Expr(id) => self.visit_expression(*id, environment),
                    HirInterpolatedStringPart::Text(_) => true,
                })
            }
            HirExprKind::Literal(_)
            | HirExprKind::Unit
            | HirExprKind::Lambda { .. }
            | HirExprKind::Missing => true,
        }
    }

    fn visit_if(&mut self, value: &HirIf, environment: &mut LocalEnvironment) -> bool {
        if !self.visit_optional(value.condition, environment) {
            return false;
        }
        let mut then_environment = environment.clone();
        let then_normal = value
            .then_block
            .is_none_or(|id| self.visit_block(id, &mut then_environment));
        let mut else_environment = environment.clone();
        let else_normal = match &value.else_branch {
            Some(HirElseBranch::If(value)) => self.visit_if(value, &mut else_environment),
            Some(HirElseBranch::Block(block)) => self.visit_block(*block, &mut else_environment),
            None => true,
        };
        let branches = [
            (then_normal, &then_environment),
            (else_normal, &else_environment),
        ];
        *environment = join_environments(
            branches
                .iter()
                .filter_map(|(normal, env)| normal.then_some(*env)),
            self.base,
        );
        then_normal || else_normal
    }

    fn visit_match(&mut self, value: &HirMatch, environment: &mut LocalEnvironment) -> bool {
        if !self.visit_optional(value.scrutinee, environment) {
            return false;
        }
        let scrutinee = value
            .scrutinee
            .map(|id| self.fact(id, environment))
            .unwrap_or(TypeFact::Unknown);
        let origins = value
            .scrutinee
            .map(|id| self.origins(id, environment))
            .unwrap_or_default();
        let mut unmatched = Some(environment.clone());
        let mut branches = Vec::new();
        for id in &value.arms {
            let Some(entry) = unmatched.take() else {
                break;
            };
            let Some(arm) = self.body.match_arms.get(id) else {
                unmatched = Some(entry);
                continue;
            };
            let mut branch = entry.clone();
            let irrefutable =
                crate::semantic_facts::value_flow::pattern_is_irrefutable(self.body, arm.pattern);
            if let Some(pattern) = arm.pattern {
                self.bind_pattern(pattern, &scrutinee, &origins, &mut branch);
            }
            let guarded = self.visit_optional(arm.guard, &mut branch);
            let mut next = Vec::new();
            if !irrefutable {
                next.push(entry);
            }
            // A false guard carries its evaluated writes to later arms. A
            // diverging guard contributes only exits, never an arm body.
            if guarded && arm.guard.is_some() {
                next.push(branch.clone());
            }
            if !next.is_empty() {
                unmatched = Some(join_environments(next.iter(), self.base));
            }
            if guarded {
                let normal = match arm.body {
                    Some(HirMatchArmBody::Expr(id)) => self.visit_expression(id, &mut branch),
                    Some(HirMatchArmBody::Block(id)) => self.visit_block(id, &mut branch),
                    None => true,
                };
                if normal {
                    branches.push(branch);
                }
            }
        }
        if let Some(unmatched) = unmatched {
            branches.push(unmatched);
        }
        let normal = !branches.is_empty();
        *environment = join_environments(branches.iter(), self.base);
        normal
    }

    pub(super) fn visit_optional(
        &mut self,
        expression: Option<HirExprId>,
        environment: &mut LocalEnvironment,
    ) -> bool {
        expression.is_none_or(|id| self.visit_expression(id, environment))
    }
}
