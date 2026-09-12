use vela_hir::body::{HirBody, HirElseBranch, HirIf, HirMatch, HirMatchArmBody, HirStmtKind};
use vela_hir::ids::{HirBlockId, HirExprId};

// None represents a unit-valued path, not a missing alternative to discard.
pub(super) fn block_results(body: &HirBody, block: HirBlockId) -> Vec<Option<HirExprId>> {
    let statement = body
        .blocks
        .get(&block)
        .and_then(|block| block.statements.last())
        .and_then(|statement| body.statements.get(statement));
    match statement.map(|statement| &statement.kind) {
        Some(HirStmtKind::Expr {
            expression,
            terminated: false,
        }) => vec![*expression],
        Some(HirStmtKind::If(value)) => if_results(body, value),
        Some(HirStmtKind::Match(value)) => match_results(body, value),
        _ => vec![None],
    }
}

pub(super) fn if_results(body: &HirBody, value: &HirIf) -> Vec<Option<HirExprId>> {
    let mut results = value
        .then_block
        .map_or_else(|| vec![None], |block| block_results(body, block));
    results.extend(match &value.else_branch {
        Some(HirElseBranch::Block(block)) => block_results(body, *block),
        Some(HirElseBranch::If(value)) => if_results(body, value),
        None => vec![None],
    });
    results
}

pub(super) fn match_results(body: &HirBody, value: &HirMatch) -> Vec<Option<HirExprId>> {
    value
        .arms
        .iter()
        .flat_map(
            |id| match body.match_arms.get(id).and_then(|arm| arm.body) {
                Some(HirMatchArmBody::Expr(expression)) => vec![Some(expression)],
                Some(HirMatchArmBody::Block(block)) => block_results(body, block),
                None => vec![None],
            },
        )
        .collect()
}
