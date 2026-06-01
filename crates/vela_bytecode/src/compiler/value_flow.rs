use vela_syntax::ast::{Block, Expr, Stmt, StmtKind};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum BlockValue<'ast> {
    Empty,
    TailExpr {
        prefix: &'ast [Stmt],
        expr: &'ast Expr,
    },
    Statements(&'ast [Stmt]),
}

pub(super) fn block_value(block: &Block) -> BlockValue<'_> {
    let Some((last, prefix)) = block.statements.split_last() else {
        return BlockValue::Empty;
    };
    match &last.kind {
        StmtKind::Expr(expr) => BlockValue::TailExpr { prefix, expr },
        _ => BlockValue::Statements(&block.statements),
    }
}
