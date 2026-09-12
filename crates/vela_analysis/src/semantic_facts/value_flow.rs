use super::ControlFlowFact;
use vela_hir::body::{
    HirBinaryOp, HirBody, HirBodyRoot, HirElseBranch, HirExprKind, HirIf,
    HirInterpolatedStringPart, HirLiteral, HirMatch, HirMatchArmBody, HirStmtKind,
};
use vela_hir::ids::{HirBlockId, HirExprId};

// None is a reachable unit value. An empty normal set means no fallthrough.
#[derive(Default)]
pub(super) struct ValueFlow {
    normal: Vec<Option<HirExprId>>,
    returned: Vec<Option<HirExprId>>,
    breaks: bool,
    continues: bool,
}

impl ValueFlow {
    fn value(value: Option<HirExprId>) -> Self {
        Self {
            normal: vec![value],
            ..Self::default()
        }
    }

    pub(super) fn control(&self) -> ControlFlowFact {
        ControlFlowFact {
            can_fallthrough: !self.normal.is_empty(),
            may_return: !self.returned.is_empty(),
            may_break: self.breaks,
            may_continue: self.continues,
        }
    }

    fn exits(&mut self, next: &mut Self) {
        self.returned.append(&mut next.returned);
        self.breaks |= next.breaks;
        self.continues |= next.continues;
    }

    fn then(&mut self, mut next: Self) {
        if !self.normal.is_empty() {
            self.exits(&mut next);
            self.normal = next.normal;
        }
    }

    fn branch(&mut self, mut next: Self) {
        self.exits(&mut next);
        self.normal.append(&mut next.normal);
    }

    fn replace_normal(&mut self, value: Option<HirExprId>) {
        if !self.normal.is_empty() {
            self.normal = vec![value];
        }
    }
}

pub(super) fn block_results(body: &HirBody, block: HirBlockId) -> Vec<Option<HirExprId>> {
    block_flow(body, block).normal
}

pub(super) fn if_results(body: &HirBody, value: &HirIf) -> Vec<Option<HirExprId>> {
    if_flow(body, value).normal
}

pub(super) fn match_results(body: &HirBody, value: &HirMatch) -> Vec<Option<HirExprId>> {
    match_flow(body, value).normal
}

pub(super) fn body_results(body: &HirBody) -> Vec<Option<HirExprId>> {
    let mut flow = match body.root {
        HirBodyRoot::Block(block) => block_flow(body, block),
        HirBodyRoot::Expr(id) => expression_flow(body, id),
        HirBodyRoot::Empty => ValueFlow::value(None),
    };
    flow.returned.append(&mut flow.normal);
    flow.returned
}

pub(super) fn block_flow(body: &HirBody, block: HirBlockId) -> ValueFlow {
    let Some(block) = body.blocks.get(&block) else {
        return ValueFlow::value(None);
    };
    let mut flow = ValueFlow::value(None);
    for id in &block.statements {
        if flow.normal.is_empty() {
            break;
        }
        if let Some(statement) = body.statements.get(id) {
            flow.then(statement_flow(body, &statement.kind));
        }
    }
    flow
}

pub(super) fn statement_flow(body: &HirBody, statement: &HirStmtKind) -> ValueFlow {
    match statement {
        HirStmtKind::Return { value } => {
            let mut flow = optional_flow(body, *value);
            flow.returned.append(&mut flow.normal);
            flow
        }
        HirStmtKind::Break => ValueFlow {
            breaks: true,
            ..ValueFlow::default()
        },
        HirStmtKind::Continue => ValueFlow {
            continues: true,
            ..ValueFlow::default()
        },
        HirStmtKind::If(value) => if_flow(body, value),
        HirStmtKind::Match(value) => match_flow(body, value),
        HirStmtKind::Block(block) => {
            let mut flow = block_flow(body, *block);
            flow.replace_normal(None);
            flow
        }
        HirStmtKind::Expr {
            expression,
            terminated,
        } => {
            let mut flow = optional_flow(body, *expression);
            if *terminated {
                flow.replace_normal(None);
            }
            flow
        }
        HirStmtKind::Let { initializer, .. } => {
            let mut flow = optional_flow(body, *initializer);
            flow.replace_normal(None);
            flow
        }
        HirStmtKind::For {
            iterable,
            body: loop_body,
            ..
        } => {
            let mut flow = optional_flow(body, *iterable);
            if !flow.normal.is_empty() {
                if let Some(block) = loop_body {
                    let mut iteration = block_flow(body, *block);
                    flow.returned.append(&mut iteration.returned);
                }
                // A for loop may execute zero iterations. It consumes the
                // body's break/continue exits, not returns from that body.
                flow.replace_normal(None);
            }
            flow
        }
    }
}

fn optional_flow(body: &HirBody, expression: Option<HirExprId>) -> ValueFlow {
    expression.map_or_else(|| ValueFlow::value(None), |id| expression_flow(body, id))
}

pub(super) fn if_flow(body: &HirBody, value: &HirIf) -> ValueFlow {
    let mut flow = optional_flow(body, value.condition);
    if flow.normal.is_empty() {
        return flow;
    }
    let mut branches = value
        .then_block
        .map_or_else(|| ValueFlow::value(None), |id| block_flow(body, id));
    branches.branch(match &value.else_branch {
        Some(HirElseBranch::Block(block)) => block_flow(body, *block),
        Some(HirElseBranch::If(value)) => if_flow(body, value),
        None => ValueFlow::value(None),
    });
    flow.then(branches);
    flow
}

pub(super) fn match_flow(body: &HirBody, value: &HirMatch) -> ValueFlow {
    let mut flow = optional_flow(body, value.scrutinee);
    if flow.normal.is_empty() {
        return flow;
    }
    let mut branches = ValueFlow::default();
    for id in &value.arms {
        let Some(arm) = body.match_arms.get(id) else {
            continue;
        };
        let mut branch = optional_flow(body, arm.guard);
        if !branch.normal.is_empty() {
            branch.then(match arm.body {
                Some(HirMatchArmBody::Block(block)) => block_flow(body, block),
                Some(HirMatchArmBody::Expr(id)) => expression_flow(body, id),
                None => ValueFlow::value(None),
            });
        }
        branches.branch(branch);
    }
    if value.arms.is_empty() {
        branches.normal.push(None);
    }
    flow.then(branches);
    flow
}

pub(super) fn expression_flow(body: &HirBody, id: HirExprId) -> ValueFlow {
    let Some(expression) = body.expression(id) else {
        return ValueFlow::value(Some(id));
    };
    match &expression.kind {
        HirExprKind::Block { block } => block_flow(body, *block),
        HirExprKind::If(value) => if_flow(body, value),
        HirExprKind::Match(value) => match_flow(body, value),
        HirExprKind::Binary {
            op: Some(HirBinaryOp::And | HirBinaryOp::Or),
            lhs,
            rhs,
        } => {
            let mut flow = optional_flow(body, *lhs);
            if !flow.normal.is_empty() {
                let mut right = optional_flow(body, *rhs);
                flow.exits(&mut right);
                flow.replace_normal(Some(id));
            }
            flow
        }
        _ => {
            let mut flow = ValueFlow::value(Some(id));
            for child in expression_inputs(&expression.kind) {
                if flow.normal.is_empty() {
                    break;
                }
                flow.then(expression_flow(body, child));
            }
            flow.replace_normal(Some(id));
            flow
        }
    }
}

fn expression_inputs(expression: &HirExprKind) -> Vec<HirExprId> {
    match expression {
        HirExprKind::Paren { expression }
        | HirExprKind::Try { expression }
        | HirExprKind::Await { expression }
        | HirExprKind::Unary {
            operand: expression,
            ..
        } => expression.iter().copied().collect(),
        HirExprKind::Binary { lhs, rhs, .. } => lhs.iter().chain(rhs).copied().collect(),
        HirExprKind::Assign { target, value, .. } => target.iter().chain(value).copied().collect(),
        HirExprKind::Field(field) => vec![field.receiver],
        HirExprKind::Index(index) => vec![index.receiver, index.index],
        HirExprKind::Call(call) => std::iter::once(call.callee)
            .chain(call.arguments.iter().filter_map(|arg| arg.value))
            .collect(),
        HirExprKind::Array { elements } | HirExprKind::Tuple { elements } => elements.clone(),
        HirExprKind::Map { entries } => entries
            .iter()
            .flat_map(|entry| entry.key.iter().chain(&entry.value).copied())
            .collect(),
        HirExprKind::Record { fields, .. } => {
            fields.iter().filter_map(|field| field.value).collect()
        }
        HirExprKind::Literal(HirLiteral::Interpolated { parts }) => parts
            .iter()
            .filter_map(|part| match part {
                HirInterpolatedStringPart::Expr(id) => Some(*id),
                HirInterpolatedStringPart::Text(_) => None,
            })
            .collect(),
        // A lambda's body belongs to its own invocation and return boundary.
        HirExprKind::Lambda { .. }
        | HirExprKind::Literal(_)
        | HirExprKind::Path(_)
        | HirExprKind::Unit
        | HirExprKind::Missing
        | HirExprKind::Block { .. }
        | HirExprKind::If(_)
        | HirExprKind::Match(_) => Vec::new(),
    }
}
