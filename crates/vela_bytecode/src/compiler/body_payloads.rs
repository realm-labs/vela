use vela_common::SourceId;
use vela_common::Span;
use vela_syntax::ast::{
    Argument, AssignOp, AstNode, Block, ElseBranch, ExprKind, IfExpr, MapEntry, MatchArm,
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
    source: Option<SourceId>,
    syntax: Option<SyntaxPattern>,
    fallback: &'ast Pattern,
}

pub(in crate::compiler) struct CompilerRecordPatternFieldPayload<'ast> {
    source: Option<SourceId>,
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
    then_body: Option<CompilerBodyPayload<'ast>>,
    else_body: Option<CompilerBodyPayload<'ast>>,
    else_if: Option<Box<CompilerIfPayload<'ast>>>,
}

pub(super) enum CompilerBlockValue<'payload, 'ast> {
    Empty,
    TailExpression {
        prefix: &'payload [CompilerStatementPayload<'ast>],
        tail: &'payload CompilerStatementPayload<'ast>,
    },
    Statements(&'payload [CompilerStatementPayload<'ast>]),
}

impl<'ast> CompilerBodyPayload<'ast> {
    pub(super) fn syntax(source: SourceId, body: SyntaxBlock, fallback: &'ast Block) -> Self {
        Self {
            syntax: SyntaxBodyPayload { source, body },
            fallback,
        }
    }

    fn nested(source: SourceId, body: SyntaxBlock, fallback: &'ast Block) -> Self {
        Self {
            syntax: SyntaxBodyPayload { source, body },
            fallback,
        }
    }

    #[cfg(test)]
    pub(super) fn fallback(&self) -> &'ast Block {
        self.fallback
    }

    pub(super) fn statement_payloads(&self) -> Vec<CompilerStatementPayload<'ast>> {
        let syntax_statements = syntax_body_statements(&self.syntax.body);

        self.fallback
            .statements
            .iter()
            .enumerate()
            .map(|(index, fallback)| CompilerStatementPayload {
                source: Some(self.syntax.source),
                syntax: syntax_statements.get(index).cloned(),
                fallback,
            })
            .collect()
    }

    pub(super) fn syntax_statements_are_empty(&self) -> bool {
        syntax_body_statements(&self.syntax.body).is_empty()
    }

    pub(super) fn has_unmatched_extra_statement_payloads(&self) -> bool {
        let syntax_statements = syntax_body_statements(&self.syntax.body);
        syntax_statements.len() != self.fallback.statements.len()
    }

    pub(super) fn block_value<'payload>(
        &self,
        statements: &'payload [CompilerStatementPayload<'ast>],
    ) -> CompilerBlockValue<'payload, 'ast> {
        let Some((tail, prefix)) = statements.split_last() else {
            return CompilerBlockValue::Empty;
        };
        if matches!(
            tail.statement_kind(),
            Some(SyntaxStatementKind::Expr | SyntaxStatementKind::If | SyntaxStatementKind::Match)
        ) {
            CompilerBlockValue::TailExpression { prefix, tail }
        } else {
            CompilerBlockValue::Statements(statements)
        }
    }

    #[cfg(test)]
    pub(super) const fn syntax_payload(&self) -> &SyntaxBodyPayload {
        &self.syntax
    }
}

fn syntax_body_statements(body: &SyntaxBlock) -> Vec<SyntaxStatement> {
    body.statements()
        .filter(syntax_statement_counts_for_body_pairing)
        .collect()
}

fn syntax_statement_counts_for_body_pairing(statement: &SyntaxStatement) -> bool {
    if !matches!(statement.statement_kind(), SyntaxStatementKind::Expr) {
        return true;
    }
    let Some(expr_stmt) = statement.as_expr() else {
        return true;
    };
    if expr_stmt.expression().is_none() {
        return false;
    }
    !syntax_statement_starts_with_infix_continuation(statement)
}

fn syntax_statement_starts_with_infix_continuation(statement: &SyntaxStatement) -> bool {
    let Some(token) = statement
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
    else {
        return false;
    };
    matches!(
        token.text(),
        "+" | "*"
            | "/"
            | "%"
            | "&&"
            | "||"
            | "=="
            | "!="
            | "==="
            | "!=="
            | "<"
            | "<="
            | ">"
            | ">="
            | ".."
            | "..="
    )
}

fn match_arm_payloads_for_fallback<'ast>(
    source: Option<SourceId>,
    syntax: SyntaxMatchExpr,
    fallback: &'ast MatchExpr,
) -> Option<Vec<CompilerMatchArmPayload<'ast>>> {
    let syntax_arms = syntax.arms();
    if syntax_arms.len() > fallback.arms.len() {
        return None;
    }
    Some(
        fallback
            .arms
            .iter()
            .enumerate()
            .map(|(index, fallback)| CompilerMatchArmPayload {
                source,
                syntax: syntax_arms.get(index).cloned(),
                fallback,
            })
            .collect(),
    )
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
    let condition = Some(CompilerExpressionPayload {
        source: Some(source),
        syntax: condition_syntax,
        fallback: &fallback.condition,
    });
    let then_body = syntax
        .then_block()
        .map(|body| CompilerBodyPayload::nested(source, body, &fallback.then_branch));
    let else_body = match fallback.else_branch.as_ref() {
        Some(ElseBranch::Block(block)) => syntax
            .else_block()
            .map(|body| CompilerBodyPayload::nested(source, body, block)),
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
        then_body,
        else_body,
        else_if,
    })
}

impl<'ast> CompilerIfPayload<'ast> {
    #[cfg(test)]
    pub(in crate::compiler) fn truncated_for_test() -> Self {
        Self {
            condition: None,
            then_body: None,
            else_body: None,
            else_if: None,
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn without_then_body_for_test(mut self) -> Self {
        self.then_body = None;
        self
    }

    #[cfg(test)]
    pub(in crate::compiler) fn without_else_body_for_test(mut self) -> Self {
        self.else_body = None;
        self
    }

    #[cfg(test)]
    pub(in crate::compiler) fn without_else_if_for_test(mut self) -> Self {
        self.else_if = None;
        self
    }
}

impl<'ast> CompilerStatementPayload<'ast> {
    #[cfg(test)]
    pub(super) fn syntax(source: SourceId, syntax: SyntaxStatement, fallback: &'ast Stmt) -> Self {
        Self {
            source: Some(source),
            syntax: Some(syntax),
            fallback,
        }
    }

    #[cfg(test)]
    pub(super) fn missing_child_payload_context(
        syntax: SyntaxStatement,
        fallback: &'ast Stmt,
    ) -> Self {
        Self {
            source: None,
            syntax: Some(syntax),
            fallback,
        }
    }

    pub(super) fn fallback(&self) -> &'ast Stmt {
        self.fallback
    }

    pub(super) fn statement_kind(&self) -> Option<SyntaxStatementKind> {
        self.syntax_statement_kind()
    }

    pub(super) fn syntax_statement_kind(&self) -> Option<SyntaxStatementKind> {
        self.syntax.as_ref().map(SyntaxStatement::statement_kind)
    }

    #[cfg(test)]
    pub(super) fn expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax_expression_kind()
    }

    pub(super) fn syntax_expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.expression()
            .map(|expression| expression.expression_kind())
    }

    #[cfg(test)]
    pub(super) fn value_expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax_value_expression_kind()
    }

    pub(super) fn syntax_value_expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.syntax_expression_kind()
            .or_else(|| match self.statement_kind()? {
                SyntaxStatementKind::Block => Some(SyntaxExpressionKind::Block),
                SyntaxStatementKind::If => Some(SyntaxExpressionKind::If),
                SyntaxStatementKind::Match => Some(SyntaxExpressionKind::Match),
                _ => None,
            })
    }

    #[cfg(test)]
    pub(super) fn let_initializer_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax_let_initializer_kind()
    }

    pub(super) fn syntax_let_initializer_kind(&self) -> Option<SyntaxExpressionKind> {
        self.syntax
            .as_ref()?
            .as_let()?
            .initializer()
            .map(|expression| expression.expression_kind())
    }

    pub(super) fn let_initializer_missing_in_syntax(&self) -> bool {
        self.syntax
            .as_ref()
            .and_then(SyntaxStatement::as_let)
            .is_some_and(|statement| statement.initializer().is_none())
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
        Some(CompilerBodyPayload::nested(
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
        match_arm_payloads_for_fallback(
            self.source,
            self.syntax.as_ref()?.as_let()?.initializer()?.as_match()?,
            match_expr,
        )
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

    #[cfg(test)]
    pub(super) fn return_value_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax_return_value_kind()
    }

    pub(super) fn syntax_return_value_kind(&self) -> Option<SyntaxExpressionKind> {
        self.syntax
            .as_ref()?
            .as_return()?
            .expression()
            .map(|expression| expression.expression_kind())
    }

    pub(super) fn syntax_statement_span(&self) -> Option<Span> {
        let source = self.source?;
        let range = self.syntax.as_ref()?.syntax().text_range();
        Some(Span::new(source, range.start().into(), range.end().into()))
    }

    pub(super) fn return_value_missing_in_syntax(&self) -> bool {
        self.syntax
            .as_ref()
            .and_then(SyntaxStatement::as_return)
            .is_some_and(|statement| statement.expression().is_none())
    }

    pub(super) fn return_value_block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let StmtKind::Return(Some(value)) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Block(block) = &value.kind else {
            return None;
        };
        Some(CompilerBodyPayload::nested(
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
        match_arm_payloads_for_fallback(
            self.source,
            self.syntax
                .as_ref()?
                .as_return()?
                .expression()?
                .as_match()?,
            match_expr,
        )
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

    pub(super) fn for_iterable_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::For { iterable, .. } = &self.fallback.kind else {
            return None;
        };
        self.source?;
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
        self.source?;
        Some(CompilerPatternPayload {
            source: self.source,
            syntax: self.syntax.as_ref()?.as_for()?.index_pattern(),
            fallback: index_pattern.as_ref()?,
        })
    }

    pub(super) fn for_value_pattern_payload(&self) -> Option<CompilerPatternPayload<'ast>> {
        let StmtKind::For { pattern, .. } = &self.fallback.kind else {
            return None;
        };
        self.source?;
        Some(CompilerPatternPayload {
            source: self.source,
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
        Some(CompilerBodyPayload::nested(
            self.source?,
            self.syntax.as_ref()?.as_block()?,
            fallback,
        ))
    }

    pub(super) fn for_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let StmtKind::For { body, .. } = &self.fallback.kind else {
            return None;
        };
        Some(CompilerBodyPayload::nested(
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
        match_arm_payloads_for_fallback(self.source, self.syntax.as_ref()?.as_match()?, match_expr)
    }

    pub(super) fn match_scrutinee_payload(&self) -> Option<CompilerExpressionPayload<'ast>> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Match(match_expr) = &expr.kind else {
            return None;
        };
        self.source?;
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

    pub(super) fn syntax_assignment_operator(&self) -> Option<AssignOp> {
        let StmtKind::Expr(expr) = &self.fallback.kind else {
            return None;
        };
        let ExprKind::Assign { .. } = &expr.kind else {
            return None;
        };
        self.expression()?.as_assign()?.operator()
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
        self.source?;
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
        self.source?;
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.assignment_value_expression(),
            fallback: value,
        })
    }

    #[cfg(test)]
    pub(super) fn assignment_value_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax_assignment_value_kind()
    }

    pub(super) fn syntax_assignment_value_kind(&self) -> Option<SyntaxExpressionKind> {
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
        Some(CompilerBodyPayload::nested(
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
        match_arm_payloads_for_fallback(
            self.source,
            self.assignment_value_expression()?.as_match()?,
            match_expr,
        )
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
        if syntax_args.len() > args.len() {
            return None;
        }
        Some(
            args.iter()
                .enumerate()
                .map(|(index, fallback)| CompilerArgumentPayload {
                    source: self.source,
                    syntax: syntax_args.get(index).cloned(),
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
        self.source?;
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
        Some(CompilerBodyPayload::nested(
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
        match_arm_payloads_for_fallback(
            self.source,
            self.expression()
                .and_then(|expression| expression.as_match())
                .or_else(|| self.syntax.as_ref()?.as_match())?,
            match_expr,
        )
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
        self.source?;
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
    #[cfg(test)]
    pub(super) fn syntax(
        source: SourceId,
        syntax: SyntaxArgument,
        fallback: &'ast Argument,
    ) -> Self {
        Self {
            source: Some(source),
            syntax: Some(syntax),
            fallback,
        }
    }

    #[cfg(test)]
    pub(super) fn missing_value_syntax(source: SourceId, fallback: &'ast Argument) -> Self {
        Self {
            source: Some(source),
            syntax: None,
            fallback,
        }
    }

    pub(in crate::compiler) fn has_value_syntax(&self) -> bool {
        self.syntax
            .as_ref()
            .is_some_and(|syntax| syntax.expression().is_some())
    }

    pub(in crate::compiler) fn syntax_name(&self) -> Option<String> {
        self.source?;
        self.syntax.as_ref().and_then(SyntaxArgument::name_text)
    }

    pub(in crate::compiler) fn value_expression_payload(&self) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload {
            source: self.source,
            syntax: self
                .source
                .and_then(|_| self.syntax.as_ref().and_then(SyntaxArgument::expression)),
            fallback: &self.fallback.value,
        }
    }

    #[cfg(test)]
    pub(super) fn syntax_argument(&self) -> Option<&SyntaxArgument> {
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerExpressionPayload<'ast> {
    #[cfg(test)]
    pub(in crate::compiler) fn missing_syntax(
        source: SourceId,
        fallback: &'ast vela_syntax::ast::Expr,
    ) -> Self {
        Self {
            source: Some(source),
            syntax: None,
            fallback,
        }
    }

    pub(in crate::compiler) fn fallback(&self) -> &'ast vela_syntax::ast::Expr {
        self.fallback
    }

    pub(in crate::compiler) fn source(&self) -> Option<SourceId> {
        self.source
    }

    #[cfg(test)]
    pub(in crate::compiler) fn kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax_kind()
    }

    pub(in crate::compiler) fn syntax_kind(&self) -> Option<SyntaxExpressionKind> {
        self.syntax
            .as_ref()
            .map(|expression| expression.expression_kind())
    }

    pub(in crate::compiler) fn syntax_expression(&self) -> Option<&SyntaxExpression> {
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerIfPayload<'ast> {
    pub(super) fn condition_payload(&self) -> Option<&CompilerExpressionPayload<'ast>> {
        self.condition.as_ref()
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
