use vela_common::SourceId;
use vela_common::Span;
use vela_syntax::ast::{
    AstNode, BinaryOp, Block, ElseBranch, ExprKind, MatchArm, Stmt, StmtKind, SyntaxBlock,
    SyntaxExpression, SyntaxExpressionKind, SyntaxMatchArm, SyntaxStatement, SyntaxStatementKind,
};

#[derive(Clone)]
pub(super) struct SyntaxBodyPayload {
    pub(super) source: SourceId,
    pub(super) body: SyntaxBlock,
}

#[derive(Clone)]
pub(super) struct CompilerBodyPayload<'ast> {
    syntax: Option<SyntaxBodyPayload>,
    fallback: &'ast Block,
}

pub(super) struct CompilerStatementPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxStatement>,
    fallback: &'ast Stmt,
}

pub(super) struct CompilerMatchArmPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxMatchArm>,
    fallback: &'ast MatchArm,
}

impl<'ast> CompilerBodyPayload<'ast> {
    pub(super) fn syntax(source: SourceId, body: SyntaxBlock, fallback: &'ast Block) -> Self {
        Self {
            syntax: Some(SyntaxBodyPayload { source, body }),
            fallback,
        }
    }

    pub(super) fn legacy(fallback: &'ast Block) -> Self {
        Self {
            syntax: None,
            fallback,
        }
    }

    pub(super) fn fallback(&self) -> &'ast Block {
        self.fallback
    }

    pub(super) fn statement_payloads(&self) -> Vec<CompilerStatementPayload<'ast>> {
        let syntax_statements = self
            .syntax
            .as_ref()
            .map(|payload| payload.body.statements().collect::<Vec<_>>());

        self.fallback
            .statements
            .iter()
            .enumerate()
            .map(|(index, fallback)| CompilerStatementPayload {
                source: self.syntax.as_ref().map(|payload| payload.source),
                syntax: syntax_statements.as_ref().and_then(|statements| {
                    syntax_statement_for_fallback(statements, index, fallback)
                }),
                fallback,
            })
            .collect()
    }

    #[cfg(test)]
    pub(super) fn syntax_payload(&self) -> Option<&SyntaxBodyPayload> {
        self.syntax.as_ref()
    }
}

fn syntax_statement_for_fallback(
    statements: &[SyntaxStatement],
    fallback_index: usize,
    fallback: &Stmt,
) -> Option<SyntaxStatement> {
    statements
        .iter()
        .find(|statement| syntax_statement_matches_span(statement, fallback.span))
        .cloned()
        .or_else(|| statements.get(fallback_index).cloned())
}

fn syntax_statement_matches_span(statement: &SyntaxStatement, span: Span) -> bool {
    let range = statement.syntax().text_range();
    u32::from(range.start()) == span.start && u32::from(range.end()) == span.end
}

fn syntax_expression_matches_span(expression: &SyntaxExpression, span: Span) -> bool {
    let range = expression.syntax().text_range();
    u32::from(range.start()) == span.start && u32::from(range.end()) == span.end
}

fn syntax_match_arm_for_fallback(
    arms: &[SyntaxMatchArm],
    fallback_index: usize,
    fallback: &MatchArm,
) -> Option<SyntaxMatchArm> {
    arms.iter()
        .find(|arm| {
            arm.body_as_expression()
                .is_some_and(|body| syntax_expression_matches_span(&body, fallback.body.span))
        })
        .cloned()
        .or_else(|| arms.get(fallback_index).cloned())
}

impl<'ast> CompilerStatementPayload<'ast> {
    pub(super) fn fallback(&self) -> &'ast Stmt {
        self.fallback
    }

    pub(super) fn statement_kind(&self) -> Option<SyntaxStatementKind> {
        self.syntax.as_ref().map(SyntaxStatement::statement_kind)
    }

    pub(super) fn expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.expression()
            .map(|expression| expression.expression_kind())
    }

    pub(super) fn let_initializer_kind(&self) -> Option<SyntaxExpressionKind> {
        self.syntax
            .as_ref()?
            .as_let()?
            .initializer()
            .map(|expression| expression.expression_kind())
    }

    pub(super) fn let_initializer_block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let StmtKind::Let {
            value: Some(value), ..
        } = &self.fallback.kind
        else {
            return None;
        };
        let ExprKind::Block(block) = &value.kind else {
            return None;
        };
        Some(CompilerBodyPayload::syntax(
            self.source?,
            self.syntax.as_ref()?.as_let()?.initializer()?.as_block()?,
            block,
        ))
    }

    pub(super) fn return_value_kind(&self) -> Option<SyntaxExpressionKind> {
        self.syntax
            .as_ref()?
            .as_return()?
            .expression()
            .map(|expression| expression.expression_kind())
    }

    pub(super) fn return_value_block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let StmtKind::Return(Some(value)) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Block(block) = &value.kind else {
            return None;
        };
        Some(CompilerBodyPayload::syntax(
            self.source?,
            self.syntax
                .as_ref()?
                .as_return()?
                .expression()?
                .as_block()?,
            block,
        ))
    }

    pub(super) fn for_iterable_binary_operator(&self) -> Option<BinaryOp> {
        self.syntax
            .as_ref()?
            .as_for()?
            .iterable()?
            .as_binary()?
            .operator()
    }

    pub(super) fn if_condition_binary_operator(&self) -> Option<BinaryOp> {
        self.syntax
            .as_ref()?
            .as_if()?
            .condition()?
            .as_binary()?
            .operator()
    }

    pub(super) fn if_then_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::If(if_expr) = &expr.kind else {
            return None;
        };
        Some(CompilerBodyPayload::syntax(
            self.source?,
            self.syntax.as_ref()?.as_if()?.then_block()?,
            &if_expr.then_branch,
        ))
    }

    pub(super) fn if_else_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::If(if_expr) = &expr.kind else {
            return None;
        };
        let ElseBranch::Block(block) = if_expr.else_branch.as_ref()? else {
            return None;
        };
        Some(CompilerBodyPayload::syntax(
            self.source?,
            self.syntax.as_ref()?.as_if()?.else_block()?,
            block,
        ))
    }

    pub(super) fn block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let StmtKind::Block(fallback) = &self.fallback.kind else {
            return None;
        };
        Some(CompilerBodyPayload::syntax(
            self.source?,
            self.syntax.as_ref()?.as_block()?,
            fallback,
        ))
    }

    pub(super) fn for_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let StmtKind::For { body, .. } = &self.fallback.kind else {
            return None;
        };
        Some(CompilerBodyPayload::syntax(
            self.source?,
            self.syntax.as_ref()?.as_for()?.body()?,
            body,
        ))
    }

    pub(super) fn match_arm_payloads(&self) -> Option<Vec<CompilerMatchArmPayload<'ast>>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Match(match_expr) = &expr.kind else {
            return None;
        };
        let syntax_arms = self.syntax.as_ref()?.as_match()?.arms();
        Some(
            match_expr
                .arms
                .iter()
                .enumerate()
                .map(|(index, fallback)| CompilerMatchArmPayload {
                    source: self.source,
                    syntax: syntax_match_arm_for_fallback(&syntax_arms, index, fallback),
                    fallback,
                })
                .collect(),
        )
    }

    fn expression(&self) -> Option<SyntaxExpression> {
        self.syntax.as_ref()?.as_expr()?.expression()
    }

    #[cfg(test)]
    pub(super) fn syntax_statement(&self) -> Option<&SyntaxStatement> {
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerMatchArmPayload<'ast> {
    pub(super) fn body_block_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let ExprKind::Block(block) = &self.fallback.body.kind else {
            return None;
        };
        Some(CompilerBodyPayload::syntax(
            self.source?,
            self.syntax.as_ref()?.body_block()?,
            block,
        ))
    }

    #[cfg(test)]
    pub(super) fn syntax_arm(&self) -> Option<&SyntaxMatchArm> {
        self.syntax.as_ref()
    }
}
