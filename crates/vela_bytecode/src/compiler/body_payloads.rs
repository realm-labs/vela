use vela_common::SourceId;
use vela_common::Span;
use vela_syntax::ast::{
    Argument, AstNode, BinaryOp, Block, ElseBranch, ExprKind, IfExpr, MapEntry, MatchArm,
    MatchExpr, Pattern, RecordField, RecordPatternField, Stmt, StmtKind, SyntaxArgument,
    SyntaxBlock, SyntaxExpression, SyntaxExpressionKind, SyntaxIfExpr, SyntaxMapEntry,
    SyntaxMatchArm, SyntaxMatchExpr, SyntaxPattern, SyntaxRecordExprField,
    SyntaxRecordPatternField, SyntaxStatement, SyntaxStatementKind,
};

mod expression_payloads;

#[derive(Clone)]
pub(super) struct SyntaxBodyPayload {
    pub(super) source: SourceId,
    pub(super) body: SyntaxBlock,
}

#[derive(Clone)]
pub(super) struct CompilerBodyPayload<'ast> {
    syntax: SyntaxBodyPayload,
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

#[derive(Clone)]
pub(in crate::compiler) struct CompilerPatternPayload<'ast> {
    syntax: Option<SyntaxPattern>,
    fallback: &'ast Pattern,
}

pub(in crate::compiler) struct CompilerRecordPatternFieldPayload<'ast> {
    syntax: Option<SyntaxRecordPatternField>,
    fallback: &'ast RecordPatternField,
}

pub(in crate::compiler) struct CompilerArgumentPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxArgument>,
    fallback: &'ast Argument,
}

#[derive(Clone)]
pub(in crate::compiler) struct CompilerExpressionPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxExpression>,
    fallback: &'ast vela_syntax::ast::Expr,
}

pub(in crate::compiler) struct CompilerMapEntryPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxMapEntry>,
    fallback: &'ast MapEntry,
}

pub(in crate::compiler) struct CompilerRecordFieldPayload<'ast> {
    source: Option<SourceId>,
    syntax: Option<SyntaxRecordExprField>,
    fallback: &'ast RecordField,
}

pub(super) struct CompilerIfPayload<'ast> {
    condition: Option<CompilerExpressionPayload<'ast>>,
    condition_operator: Option<BinaryOp>,
    then_body: Option<CompilerBodyPayload<'ast>>,
    else_body: Option<CompilerBodyPayload<'ast>>,
    else_if: Option<Box<CompilerIfPayload<'ast>>>,
}

impl<'ast> CompilerBodyPayload<'ast> {
    pub(super) fn syntax(source: SourceId, body: SyntaxBlock, fallback: &'ast Block) -> Self {
        Self {
            syntax: SyntaxBodyPayload { source, body },
            fallback,
        }
    }

    pub(super) fn fallback(&self) -> &'ast Block {
        self.fallback
    }

    pub(super) fn statement_payloads(&self) -> Vec<CompilerStatementPayload<'ast>> {
        let syntax_statements = self.syntax.body.statements().collect::<Vec<_>>();

        self.fallback
            .statements
            .iter()
            .map(|fallback| CompilerStatementPayload {
                source: Some(self.syntax.source),
                syntax: syntax_statement_for_fallback(&syntax_statements, fallback),
                fallback,
            })
            .collect()
    }

    #[cfg(test)]
    pub(super) const fn syntax_payload(&self) -> &SyntaxBodyPayload {
        &self.syntax
    }
}

fn syntax_statement_for_fallback(
    statements: &[SyntaxStatement],
    fallback: &Stmt,
) -> Option<SyntaxStatement> {
    statements
        .iter()
        .find(|statement| syntax_statement_matches_span(statement, fallback.span))
        .cloned()
}

fn syntax_statement_matches_span(statement: &SyntaxStatement, span: Span) -> bool {
    syntax_range_overlaps_span(statement.syntax().text_range(), span)
}

fn syntax_expression_matches_span(expression: &SyntaxExpression, span: Span) -> bool {
    syntax_range_overlaps_span(expression.syntax().text_range(), span)
}

fn syntax_argument_for_fallback(
    arguments: &[SyntaxArgument],
    fallback: &Argument,
) -> Option<SyntaxArgument> {
    arguments
        .iter()
        .find(|argument| {
            argument.expression().is_some_and(|expression| {
                syntax_expression_matches_span(&expression, fallback.value.span)
            })
        })
        .cloned()
}

fn syntax_range_overlaps_span(range: vela_syntax::TextRange, span: Span) -> bool {
    let start = u32::from(range.start());
    let end = u32::from(range.end());
    start < span.end && span.start < end
}

fn syntax_match_arm_for_fallback(
    arms: &[SyntaxMatchArm],
    fallback: &MatchArm,
) -> Option<SyntaxMatchArm> {
    arms.iter()
        .find(|arm| {
            arm.body_as_expression()
                .is_some_and(|body| syntax_expression_matches_span(&body, fallback.body.span))
        })
        .cloned()
}

fn match_arm_payloads_for_fallback<'ast>(
    source: Option<SourceId>,
    syntax: SyntaxMatchExpr,
    fallback: &'ast MatchExpr,
) -> Vec<CompilerMatchArmPayload<'ast>> {
    let syntax_arms = syntax.arms();
    fallback
        .arms
        .iter()
        .map(|fallback| CompilerMatchArmPayload {
            source,
            syntax: syntax_match_arm_for_fallback(&syntax_arms, fallback),
            fallback,
        })
        .collect()
}

fn match_scrutinee_payload_for_fallback<'ast>(
    source: Option<SourceId>,
    syntax: SyntaxMatchExpr,
    fallback: &'ast MatchExpr,
) -> CompilerExpressionPayload<'ast> {
    CompilerExpressionPayload {
        source,
        syntax: syntax.scrutinee(),
        fallback: &fallback.scrutinee,
    }
}

fn if_payload_for_fallback<'ast>(
    source: Option<SourceId>,
    syntax: SyntaxIfExpr,
    fallback: &'ast IfExpr,
) -> Option<CompilerIfPayload<'ast>> {
    let source = source?;
    let condition_syntax = syntax.condition();
    let condition_operator = condition_syntax
        .as_ref()
        .and_then(|condition| condition.as_binary())
        .and_then(|condition| condition.operator());
    let condition = Some(CompilerExpressionPayload {
        source: Some(source),
        syntax: condition_syntax,
        fallback: &fallback.condition,
    });
    let then_body = syntax
        .then_block()
        .map(|body| CompilerBodyPayload::syntax(source, body, &fallback.then_branch));
    let else_body = match fallback.else_branch.as_ref() {
        Some(ElseBranch::Block(block)) => syntax
            .else_block()
            .map(|body| CompilerBodyPayload::syntax(source, body, block)),
        Some(ElseBranch::If(_)) | None => None,
    };
    let else_if = match fallback.else_branch.as_ref() {
        Some(ElseBranch::If(if_expr)) => {
            let syntax_if = syntax.else_if()?;
            if_payload_for_fallback(Some(source), syntax_if, if_expr).map(Box::new)
        }
        Some(ElseBranch::Block(_)) | None => None,
    };
    Some(CompilerIfPayload {
        condition,
        condition_operator,
        then_body,
        else_body,
        else_if,
    })
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

    pub(super) fn value_expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.expression_kind()
            .or_else(|| match self.statement_kind()? {
                SyntaxStatementKind::Block => Some(SyntaxExpressionKind::Block),
                SyntaxStatementKind::If => Some(SyntaxExpressionKind::If),
                SyntaxStatementKind::Match => Some(SyntaxExpressionKind::Match),
                _ => None,
            })
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

    pub(super) fn let_initializer_if_payload(&self) -> Option<CompilerIfPayload<'ast>> {
        let StmtKind::Let {
            value: Some(value), ..
        } = &self.fallback.kind
        else {
            return None;
        };
        let ExprKind::If(if_expr) = &value.kind else {
            return None;
        };
        if_payload_for_fallback(
            self.source,
            self.syntax.as_ref()?.as_let()?.initializer()?.as_if()?,
            if_expr,
        )
    }

    pub(super) fn let_initializer_match_arm_payloads(
        &self,
    ) -> Option<Vec<CompilerMatchArmPayload<'ast>>> {
        let StmtKind::Let {
            value: Some(value), ..
        } = &self.fallback.kind
        else {
            return None;
        };
        let ExprKind::Match(match_expr) = &value.kind else {
            return None;
        };
        Some(match_arm_payloads_for_fallback(
            self.source,
            self.syntax.as_ref()?.as_let()?.initializer()?.as_match()?,
            match_expr,
        ))
    }

    pub(in crate::compiler) fn let_initializer_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Let {
            value: Some(value), ..
        } = &self.fallback.kind
        else {
            return None;
        };
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.syntax.as_ref()?.as_let()?.initializer(),
            fallback: value,
        })
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

    pub(super) fn return_value_if_payload(&self) -> Option<CompilerIfPayload<'ast>> {
        let StmtKind::Return(Some(value)) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::If(if_expr) = &value.kind else {
            return None;
        };
        if_payload_for_fallback(
            self.source,
            self.syntax.as_ref()?.as_return()?.expression()?.as_if()?,
            if_expr,
        )
    }

    pub(super) fn return_value_match_arm_payloads(
        &self,
    ) -> Option<Vec<CompilerMatchArmPayload<'ast>>> {
        let StmtKind::Return(Some(value)) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Match(match_expr) = &value.kind else {
            return None;
        };
        Some(match_arm_payloads_for_fallback(
            self.source,
            self.syntax
                .as_ref()?
                .as_return()?
                .expression()?
                .as_match()?,
            match_expr,
        ))
    }

    pub(in crate::compiler) fn return_value_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Return(Some(value)) = &self.fallback.kind else {
            return None;
        };
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.syntax.as_ref()?.as_return()?.expression(),
            fallback: value,
        })
    }

    pub(super) fn for_iterable_binary_operator(&self) -> Option<BinaryOp> {
        self.syntax
            .as_ref()?
            .as_for()?
            .iterable()?
            .as_binary()?
            .operator()
    }

    pub(super) fn for_iterable_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::For { iterable, .. } = &self.fallback.kind else {
            return None;
        };
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.syntax.as_ref()?.as_for()?.iterable(),
            fallback: iterable,
        })
    }

    pub(super) fn for_index_pattern_payload(&self) -> Option<CompilerPatternPayload<'ast>> {
        let StmtKind::For { index_pattern, .. } = &self.fallback.kind else {
            return None;
        };
        Some(CompilerPatternPayload {
            syntax: self.syntax.as_ref()?.as_for()?.index_pattern(),
            fallback: index_pattern.as_ref()?,
        })
    }

    pub(super) fn for_value_pattern_payload(&self) -> Option<CompilerPatternPayload<'ast>> {
        let StmtKind::For { pattern, .. } = &self.fallback.kind else {
            return None;
        };
        Some(CompilerPatternPayload {
            syntax: self.syntax.as_ref()?.as_for()?.value_pattern(),
            fallback: pattern,
        })
    }

    pub(super) fn if_payload(&self) -> Option<CompilerIfPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::If(if_expr) = &expr.kind else {
            return None;
        };
        if_payload_for_fallback(self.source, self.syntax.as_ref()?.as_if()?, if_expr)
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
        Some(match_arm_payloads_for_fallback(
            self.source,
            self.syntax.as_ref()?.as_match()?,
            match_expr,
        ))
    }

    pub(super) fn match_scrutinee_payload(&self) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Match(match_expr) = &expr.kind else {
            return None;
        };
        Some(match_scrutinee_payload_for_fallback(
            self.source,
            self.syntax.as_ref()?.as_match()?,
            match_expr,
        ))
    }

    fn expression(&self) -> Option<SyntaxExpression> {
        self.syntax.as_ref()?.as_expr()?.expression()
    }

    pub(in crate::compiler) fn expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.expression(),
            fallback: expr,
        })
    }

    fn assignment_value_expression(&self) -> Option<SyntaxExpression> {
        self.expression()?.as_assign()?.value()
    }

    fn assignment_target_expression(&self) -> Option<SyntaxExpression> {
        self.expression()?.as_assign()?.target()
    }

    pub(in crate::compiler) fn assignment_target_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Assign { target, .. } = &expr.kind else {
            return None;
        };
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.assignment_target_expression(),
            fallback: target,
        })
    }

    pub(in crate::compiler) fn assignment_value_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Assign { value, .. } = &expr.kind else {
            return None;
        };
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.assignment_value_expression(),
            fallback: value,
        })
    }

    pub(super) fn assignment_value_kind(&self) -> Option<SyntaxExpressionKind> {
        self.assignment_value_expression()
            .map(|expression| expression.expression_kind())
    }

    pub(super) fn assignment_value_block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Assign { value, .. } = &expr.kind else {
            return None;
        };
        let ExprKind::Block(block) = &value.kind else {
            return None;
        };
        Some(CompilerBodyPayload::syntax(
            self.source?,
            self.assignment_value_expression()?.as_block()?,
            block,
        ))
    }

    pub(super) fn assignment_value_if_payload(&self) -> Option<CompilerIfPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Assign { value, .. } = &expr.kind else {
            return None;
        };
        let ExprKind::If(if_expr) = &value.kind else {
            return None;
        };
        if_payload_for_fallback(
            self.source,
            self.assignment_value_expression()?.as_if()?,
            if_expr,
        )
    }

    pub(super) fn assignment_value_match_arm_payloads(
        &self,
    ) -> Option<Vec<CompilerMatchArmPayload<'ast>>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Assign { value, .. } = &expr.kind else {
            return None;
        };
        let ExprKind::Match(match_expr) = &value.kind else {
            return None;
        };
        Some(match_arm_payloads_for_fallback(
            self.source,
            self.assignment_value_expression()?.as_match()?,
            match_expr,
        ))
    }

    pub(in crate::compiler) fn call_argument_payloads(
        &self,
    ) -> Option<Vec<CompilerArgumentPayload<'ast>>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Call { args, .. } = &expr.kind else {
            return None;
        };
        let syntax_args = self.expression()?.as_call()?.arguments();
        Some(
            args.iter()
                .map(|fallback| CompilerArgumentPayload {
                    source: self.source,
                    syntax: syntax_argument_for_fallback(&syntax_args, fallback),
                    fallback,
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn call_callee_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Call { callee, .. } = &expr.kind else {
            return None;
        };
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.expression()?.as_call()?.callee(),
            fallback: callee,
        })
    }

    pub(super) fn expression_block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Block(block) = &expr.kind else {
            return None;
        };
        Some(CompilerBodyPayload::syntax(
            self.source?,
            self.expression()
                .and_then(|expression| expression.as_block())
                .or_else(|| self.syntax.as_ref()?.as_block())?,
            block,
        ))
    }

    pub(super) fn expression_if_payload(&self) -> Option<CompilerIfPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::If(if_expr) = &expr.kind else {
            return None;
        };
        if_payload_for_fallback(
            self.source,
            self.expression()
                .and_then(|expression| expression.as_if())
                .or_else(|| self.syntax.as_ref()?.as_if())?,
            if_expr,
        )
    }

    pub(super) fn expression_match_arm_payloads(
        &self,
    ) -> Option<Vec<CompilerMatchArmPayload<'ast>>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Match(match_expr) = &expr.kind else {
            return None;
        };
        Some(match_arm_payloads_for_fallback(
            self.source,
            self.expression()
                .and_then(|expression| expression.as_match())
                .or_else(|| self.syntax.as_ref()?.as_match())?,
            match_expr,
        ))
    }

    pub(super) fn expression_match_scrutinee_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Match(match_expr) = &expr.kind else {
            return None;
        };
        Some(match_scrutinee_payload_for_fallback(
            self.source,
            self.expression()
                .and_then(|expression| expression.as_match())
                .or_else(|| self.syntax.as_ref()?.as_match())?,
            match_expr,
        ))
    }

    #[cfg(test)]
    pub(super) fn syntax_statement(&self) -> Option<&SyntaxStatement> {
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerArgumentPayload<'ast> {
    pub(in crate::compiler) fn syntax_name(&self) -> Option<String> {
        self.syntax.as_ref().and_then(SyntaxArgument::name_text)
    }

    pub(in crate::compiler) fn value_expression_payload(&self) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload {
            source: self.source,
            syntax: self.syntax.as_ref().and_then(SyntaxArgument::expression),
            fallback: &self.fallback.value,
        }
    }

    #[cfg(test)]
    pub(super) fn syntax_argument(&self) -> Option<&SyntaxArgument> {
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerExpressionPayload<'ast> {
    pub(in crate::compiler) fn fallback(&self) -> &'ast vela_syntax::ast::Expr {
        self.fallback
    }

    pub(in crate::compiler) fn kind(&self) -> Option<SyntaxExpressionKind> {
        self.syntax
            .as_ref()
            .map(|expression| expression.expression_kind())
    }

    #[cfg(test)]
    pub(super) fn syntax_expression(&self) -> Option<&SyntaxExpression> {
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerIfPayload<'ast> {
    pub(super) fn condition_payload(&self) -> Option<&CompilerExpressionPayload<'ast>> {
        self.condition.as_ref()
    }

    pub(super) fn condition_operator(&self) -> Option<BinaryOp> {
        self.condition_operator
    }

    pub(super) fn then_body(&self) -> Option<&CompilerBodyPayload<'ast>> {
        self.then_body.as_ref()
    }

    pub(super) fn else_body(&self) -> Option<&CompilerBodyPayload<'ast>> {
        self.else_body.as_ref()
    }

    pub(super) fn else_if(&self) -> Option<&CompilerIfPayload<'ast>> {
        self.else_if.as_deref()
    }
}
