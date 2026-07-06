use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::{Expr, ExprKind, MatchExpr, SyntaxExpressionKind};

use crate::compiler::body_payloads::{CompilerExpressionPayload, CompilerMatchArmPayload};
use crate::compiler::patterns::PatternBindingFacts;
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};
use crate::{Constant, Register, UnlinkedInstructionKind};

impl Compiler<'_, '_> {
    #[cfg(test)]
    pub(in crate::compiler) fn compile_match(
        &mut self,
        match_expr: &MatchExpr,
    ) -> CompileResult<bool> {
        self.compile_match_with_payloads(match_expr, None, None)
    }

    #[cfg(test)]
    pub(in crate::compiler) fn compile_match_with_payloads(
        &mut self,
        match_expr: &MatchExpr,
        scrutinee_payload: Option<&CompilerExpressionPayload<'_>>,
        arm_payloads: Option<&[CompilerMatchArmPayload]>,
    ) -> CompileResult<bool> {
        reject_missing_match_scrutinee_payload(scrutinee_payload)?;
        reject_missing_match_arm_payloads(match_expr, scrutinee_payload, arm_payloads)?;
        let scrutinee_fact =
            self.script_fact_for_expr_with_payload(&match_expr.scrutinee, scrutinee_payload);
        let scrutinee = self.compile_expr_with_payload(&match_expr.scrutinee, scrutinee_payload)?;
        let mut end_jumps = Vec::new();
        let mut all_arms_return = !match_expr.arms.is_empty();

        for (index, arm) in match_expr.arms.iter().enumerate() {
            let arm_payload = match_arm_payload_at(arm_payloads, index)?;
            let pattern_payload = arm_payload.map(CompilerMatchArmPayload::pattern_payload);
            let mut next_arm_jumps =
                self.compile_match_pattern(scrutinee, &arm.pattern, pattern_payload.as_ref())?;
            let previous_locals = self.locals.clone();
            let previous_hir_locals = self.hir_locals.clone();
            let previous_script_types = self.script_types.clone();
            let previous_value_types = self.value_types.clone();
            let previous_value_shapes = self.value_shapes.clone();
            self.bind_pattern_locals(
                scrutinee,
                &arm.pattern,
                pattern_payload.as_ref(),
                arm.body.span,
                PatternBindingFacts::new(scrutinee_fact.clone()),
                LocalBindingKind::Pattern,
            )?;
            let guard_payload = arm_payload.and_then(CompilerMatchArmPayload::guard_payload);
            if arm.guard.is_some()
                && arm_payload.is_some_and(CompilerMatchArmPayload::has_syntax)
                && guard_payload
                    .as_ref()
                    .is_none_or(|payload| payload.syntax_expression().is_none())
            {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "missing CST match guard payload",
                )));
            }
            if let Some(jump) = self.compile_match_guard(arm.guard.as_ref(), guard_payload)? {
                next_arm_jumps.push(jump);
            }
            let arm_returned = self.compile_match_arm_statement(arm, arm_payload)?;
            self.locals = previous_locals;
            self.hir_locals = previous_hir_locals;
            self.script_types = previous_script_types;
            self.value_types = previous_value_types;
            self.value_shapes = previous_value_shapes;
            all_arms_return &= arm_returned;
            if !arm_returned {
                end_jumps.push(self.emit_jump());
            }
            if next_arm_jumps.is_empty() {
                break;
            }
            for jump in next_arm_jumps {
                self.patch_jump(jump, self.current_offset())?;
            }
        }
        for jump in end_jumps {
            self.patch_jump(jump, self.current_offset())?;
        }
        Ok(all_arms_return)
    }

    #[cfg(test)]
    fn compile_match_arm_statement(
        &mut self,
        arm: &vela_syntax::ast::MatchArm,
        payload: Option<&CompilerMatchArmPayload>,
    ) -> CompileResult<bool> {
        if let Some(payload) = payload
            && let Some(kind) = payload.syntax_body_expression_kind()
        {
            return self.compile_match_arm_statement_with_syntax_kind(arm, payload, kind);
        }
        if payload.is_some_and(CompilerMatchArmPayload::has_syntax) {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST match arm body",
            )));
        }
        self.compile_match_arm_statement_without_payload(arm, payload)
    }

    #[cfg(test)]
    fn compile_match_arm_statement_with_syntax_kind(
        &mut self,
        arm: &vela_syntax::ast::MatchArm,
        payload: &CompilerMatchArmPayload,
        kind: SyntaxExpressionKind,
    ) -> CompileResult<bool> {
        if kind == SyntaxExpressionKind::Block {
            let body_payload = payload.body_block_payload();
            if let Some(body) = body_payload {
                return self.compile_body_payload_statements(&body);
            }
            return Err(missing_cst_match_arm_child_payload(
                "missing CST match arm block body payload",
            ));
        }
        let body_payload = payload.body_expression_payload();
        self.compile_expr_with_payload(&arm.body, Some(&body_payload))?;
        Ok(false)
    }

    #[cfg(test)]
    fn compile_match_arm_statement_without_payload(
        &mut self,
        arm: &vela_syntax::ast::MatchArm,
        payload: Option<&CompilerMatchArmPayload>,
    ) -> CompileResult<bool> {
        match &arm.body.kind {
            ExprKind::Block(_) => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST match arm block body payload",
            ))),
            _ => {
                let body_payload = payload.map(CompilerMatchArmPayload::body_expression_payload);
                self.compile_expr_with_payload(&arm.body, body_payload.as_ref())?;
                Ok(false)
            }
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn compile_match_value_to(
        &mut self,
        match_expr: &MatchExpr,
        dst: Register,
    ) -> CompileResult<bool> {
        self.compile_match_value_with_payloads(match_expr, dst, None, None)
    }

    pub(in crate::compiler) fn compile_match_value_with_payloads(
        &mut self,
        match_expr: &MatchExpr,
        dst: Register,
        scrutinee_payload: Option<&CompilerExpressionPayload<'_>>,
        arm_payloads: Option<&[CompilerMatchArmPayload]>,
    ) -> CompileResult<bool> {
        reject_missing_match_scrutinee_payload(scrutinee_payload)?;
        reject_missing_match_arm_payloads(match_expr, scrutinee_payload, arm_payloads)?;
        let scrutinee_fact =
            self.script_fact_for_expr_with_payload(&match_expr.scrutinee, scrutinee_payload);
        let scrutinee = self.compile_expr_with_payload(&match_expr.scrutinee, scrutinee_payload)?;
        let mut end_jumps = Vec::new();
        let mut all_arms_return = !match_expr.arms.is_empty();
        let mut has_catch_all = false;

        for (index, arm) in match_expr.arms.iter().enumerate() {
            let arm_payload = match_arm_payload_at(arm_payloads, index)?;
            let pattern_payload = arm_payload.map(CompilerMatchArmPayload::pattern_payload);
            let mut next_arm_jumps =
                self.compile_match_pattern(scrutinee, &arm.pattern, pattern_payload.as_ref())?;
            let previous_locals = self.locals.clone();
            let previous_hir_locals = self.hir_locals.clone();
            let previous_script_types = self.script_types.clone();
            let previous_value_types = self.value_types.clone();
            let previous_value_shapes = self.value_shapes.clone();
            self.bind_pattern_locals(
                scrutinee,
                &arm.pattern,
                pattern_payload.as_ref(),
                arm.body.span,
                PatternBindingFacts::new(scrutinee_fact.clone()),
                LocalBindingKind::Pattern,
            )?;
            let guard_payload = arm_payload.and_then(CompilerMatchArmPayload::guard_payload);
            if arm.guard.is_some()
                && arm_payload.is_some_and(CompilerMatchArmPayload::has_syntax)
                && guard_payload
                    .as_ref()
                    .is_none_or(|payload| payload.syntax_expression().is_none())
            {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "missing CST match guard payload",
                )));
            }
            if let Some(jump) = self.compile_match_guard(arm.guard.as_ref(), guard_payload)? {
                next_arm_jumps.push(jump);
            }
            let arm_returned = self.compile_match_arm_value_to(&arm.body, arm_payload, dst)?;
            self.locals = previous_locals;
            self.hir_locals = previous_hir_locals;
            self.script_types = previous_script_types;
            self.value_types = previous_value_types;
            self.value_shapes = previous_value_shapes;
            all_arms_return &= arm_returned;
            if !arm_returned {
                end_jumps.push(self.emit_jump());
            }
            if next_arm_jumps.is_empty() {
                has_catch_all = true;
                break;
            }
            for jump in next_arm_jumps {
                self.patch_jump(jump, self.current_offset())?;
            }
        }
        if !has_catch_all {
            self.emit_constant_to(dst, Constant::Null);
            all_arms_return = false;
        }

        for jump in end_jumps {
            self.patch_jump(jump, self.current_offset())?;
        }

        Ok(all_arms_return)
    }

    fn compile_match_guard(
        &mut self,
        guard: Option<&Expr>,
        payload: Option<CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Option<usize>> {
        let Some(guard) = guard else {
            return Ok(None);
        };
        let condition = self.compile_expr_with_payload(guard, payload.as_ref())?;
        Ok(Some(self.emit_jump_if_false(condition)))
    }

    fn compile_match_arm_value_to(
        &mut self,
        body: &Expr,
        payload: Option<&CompilerMatchArmPayload>,
        dst: Register,
    ) -> CompileResult<bool> {
        if let Some(payload) = payload
            && let Some(kind) = payload.syntax_body_expression_kind()
        {
            return self.compile_match_arm_value_with_syntax_kind(body, payload, kind, dst);
        }
        if payload.is_some_and(CompilerMatchArmPayload::has_syntax) {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST match arm body",
            )));
        }
        self.compile_match_arm_value_without_payload_to(body, payload, dst)
    }

    fn compile_match_arm_value_with_syntax_kind(
        &mut self,
        body: &Expr,
        payload: &CompilerMatchArmPayload,
        kind: SyntaxExpressionKind,
        dst: Register,
    ) -> CompileResult<bool> {
        match kind {
            SyntaxExpressionKind::Block => {
                let body_payload = payload.body_block_payload();
                if let Some(body) = body_payload {
                    self.compile_block_payload_value_to(&body, dst)
                } else {
                    Err(missing_cst_match_arm_child_payload(
                        "missing CST match arm block body payload",
                    ))
                }
            }
            SyntaxExpressionKind::If => {
                let body_payload = payload.body_expression_payload();
                let ExprKind::If(if_expr) = &body.kind else {
                    return Err(missing_cst_match_arm_child_payload(
                        "missing CST match arm if payload",
                    ));
                };
                let Some(if_payload) = body_payload.if_payload() else {
                    return Err(missing_cst_match_arm_child_payload(
                        "missing CST match arm if payload",
                    ));
                };
                self.compile_if_value_with_payloads(if_expr, dst, Some(&if_payload))
            }
            SyntaxExpressionKind::Match => {
                let body_payload = payload.body_expression_payload();
                let ExprKind::Match(match_expr) = &body.kind else {
                    return Err(missing_cst_match_arm_child_payload(
                        "missing CST match arm match payloads",
                    ));
                };
                let Some(scrutinee_payload) = body_payload.match_scrutinee_payload() else {
                    return Err(missing_cst_match_arm_child_payload(
                        "missing CST match arm match payloads",
                    ));
                };
                let Some(arm_payloads) = body_payload.match_arm_payloads() else {
                    return Err(missing_cst_match_arm_child_payload(
                        "missing CST match arm match payloads",
                    ));
                };
                self.compile_match_value_with_payloads(
                    match_expr,
                    dst,
                    Some(&scrutinee_payload),
                    Some(&arm_payloads),
                )
            }
            _ => {
                let body_payload = payload.body_expression_payload();
                let value = self.compile_expr_with_payload(body, Some(&body_payload))?;
                self.emit(UnlinkedInstructionKind::Move { dst, src: value });
                Ok(false)
            }
        }
    }

    fn compile_match_arm_value_without_payload_to(
        &mut self,
        body: &Expr,
        payload: Option<&CompilerMatchArmPayload>,
        dst: Register,
    ) -> CompileResult<bool> {
        match &body.kind {
            ExprKind::Block(_) => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST match arm block body payload",
            ))),
            _ => {
                let body_payload = payload.map(CompilerMatchArmPayload::body_expression_payload);
                let value = self.compile_expr_with_payload(body, body_payload.as_ref())?;
                self.emit(UnlinkedInstructionKind::Move { dst, src: value });
                Ok(false)
            }
        }
    }
}

fn match_arm_payload_at(
    payloads: Option<&[CompilerMatchArmPayload]>,
    index: usize,
) -> CompileResult<Option<&CompilerMatchArmPayload>> {
    let Some(payloads) = payloads else {
        return Ok(None);
    };
    let payload = payloads.get(index).ok_or_else(|| {
        CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "missing CST match arm payload",
        ))
    })?;
    if !payload.has_syntax() {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "missing CST match arm payload",
        )));
    }
    Ok(Some(payload))
}

fn missing_cst_match_arm_child_payload(message: &'static str) -> CompileError {
    CompileError::new(CompileErrorKind::UnsupportedSyntax(message))
}

fn reject_missing_match_scrutinee_payload(
    payload: Option<&CompilerExpressionPayload<'_>>,
) -> CompileResult<()> {
    if payload.is_some_and(|payload| payload.syntax_expression().is_none()) {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "missing CST match scrutinee payload",
        )));
    }
    Ok(())
}

fn reject_missing_match_arm_payloads(
    match_expr: &MatchExpr,
    scrutinee_payload: Option<&CompilerExpressionPayload<'_>>,
    arm_payloads: Option<&[CompilerMatchArmPayload]>,
) -> CompileResult<()> {
    if scrutinee_payload.is_some() && arm_payloads.is_none() {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST match arms",
        )));
    }
    if arm_payloads.is_some_and(|payloads| payloads.len() > match_expr.arms.len()) {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "mismatched CST match arms",
        )));
    }
    Ok(())
}
