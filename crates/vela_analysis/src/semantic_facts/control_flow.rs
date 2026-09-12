use super::{ControlFlowFact, value_flow};
use vela_hir::body::{HirBody, HirStmtKind};
use vela_hir::ids::{HirBlockId, HirExprId};

pub(super) fn expression_flow(body: &HirBody, expression: HirExprId) -> ControlFlowFact {
    value_flow::expression_flow(body, expression).control()
}

pub(super) fn statement_flow(body: &HirBody, statement: &HirStmtKind) -> ControlFlowFact {
    value_flow::statement_flow(body, statement).control()
}

pub(super) fn block_flow(body: &HirBody, block: HirBlockId) -> ControlFlowFact {
    value_flow::block_flow(body, block).control()
}
