use vela_hir::body::{HirBody, HirElseBranch, HirMatchArmBody, HirStmtKind};
use vela_hir::ids::HirBlockId;

use super::ControlFlowFact;

pub(super) fn fallthrough_flow() -> ControlFlowFact {
    ControlFlowFact {
        can_fallthrough: true,
        ..ControlFlowFact::default()
    }
}

pub(super) fn statement_flow(body: &HirBody, statement: &HirStmtKind) -> ControlFlowFact {
    match statement {
        HirStmtKind::Return { .. } => ControlFlowFact {
            can_fallthrough: false,
            may_return: true,
            ..ControlFlowFact::default()
        },
        HirStmtKind::Break => ControlFlowFact {
            can_fallthrough: false,
            may_break: true,
            ..ControlFlowFact::default()
        },
        HirStmtKind::Continue => ControlFlowFact {
            can_fallthrough: false,
            may_continue: true,
            ..ControlFlowFact::default()
        },
        HirStmtKind::Block(block) => block_flow(body, *block),
        HirStmtKind::If(value) => if_flow(body, value),
        HirStmtKind::Match(value) => match_flow(body, value),
        HirStmtKind::For {
            body: loop_body, ..
        } => {
            let mut flow = loop_body.map_or_else(fallthrough_flow, |block| block_flow(body, block));
            flow.can_fallthrough = true;
            flow.may_break = false;
            flow.may_continue = false;
            flow
        }
        HirStmtKind::Let { .. } | HirStmtKind::Expr { .. } => fallthrough_flow(),
    }
}

pub(super) fn block_flow(body: &HirBody, block: HirBlockId) -> ControlFlowFact {
    let Some(block) = body.blocks.get(&block) else {
        return fallthrough_flow();
    };
    let mut flow = fallthrough_flow();
    for statement in &block.statements {
        if !flow.can_fallthrough {
            break;
        }
        let Some(statement) = body.statements.get(statement) else {
            continue;
        };
        let next = statement_flow(body, &statement.kind);
        flow.may_return |= next.may_return;
        flow.may_break |= next.may_break;
        flow.may_continue |= next.may_continue;
        flow.can_fallthrough = next.can_fallthrough;
    }
    flow
}

pub(super) fn if_flow(body: &HirBody, value: &vela_hir::body::HirIf) -> ControlFlowFact {
    let then_flow = value
        .then_block
        .map_or_else(fallthrough_flow, |block| block_flow(body, block));
    let else_flow =
        value
            .else_branch
            .as_ref()
            .map_or_else(fallthrough_flow, |branch| match branch {
                HirElseBranch::Block(block) => block_flow(body, *block),
                HirElseBranch::If(value) => if_flow(body, value),
            });
    branch_flow([then_flow, else_flow])
}

pub(super) fn match_flow(body: &HirBody, value: &vela_hir::body::HirMatch) -> ControlFlowFact {
    let arms = value.arms.iter().filter_map(|arm| {
        let arm = body.match_arms.get(arm)?;
        Some(match arm.body {
            Some(HirMatchArmBody::Block(block)) => block_flow(body, block),
            Some(HirMatchArmBody::Expr(_)) | None => fallthrough_flow(),
        })
    });
    branch_flow(arms)
}

fn branch_flow(flows: impl IntoIterator<Item = ControlFlowFact>) -> ControlFlowFact {
    let mut result = ControlFlowFact::default();
    let mut saw_branch = false;
    for flow in flows {
        saw_branch = true;
        result.can_fallthrough |= flow.can_fallthrough;
        result.may_return |= flow.may_return;
        result.may_break |= flow.may_break;
        result.may_continue |= flow.may_continue;
    }
    if !saw_branch {
        result.can_fallthrough = true;
    }
    result
}
