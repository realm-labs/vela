use vela_hir::binding::LocalBindingKind;
use vela_syntax::ast::{Expr, ExprKind, MatchExpr, SyntaxExpressionKind};

use crate::compiler::body_payloads::{CompilerExpressionPayload, CompilerMatchArmPayload};
use crate::compiler::expression_payload_kinds::expression_payload_kind_matches;
use crate::compiler::patterns::PatternBindingFacts;
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler};
use crate::{Constant, Register, UnlinkedInstructionKind};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_match(
        &mut self,
        match_expr: &MatchExpr,
    ) -> CompileResult<bool> {
        self.compile_match_with_payloads(match_expr, None, None)
    }

    pub(in crate::compiler) fn compile_match_with_payloads(
        &mut self,
        match_expr: &MatchExpr,
        scrutinee_payload: Option<&CompilerExpressionPayload<'_>>,
        arm_payloads: Option<&[CompilerMatchArmPayload<'_>]>,
    ) -> CompileResult<bool> {
        let scrutinee_fact =
            self.script_fact_for_expr_with_payload(&match_expr.scrutinee, scrutinee_payload);
        let scrutinee = self.compile_expr_with_payload(&match_expr.scrutinee, scrutinee_payload)?;
        let mut end_jumps = Vec::new();
        let mut all_arms_return = !match_expr.arms.is_empty();

        for (index, arm) in match_expr.arms.iter().enumerate() {
            let arm_payload = arm_payloads.and_then(|payloads| payloads.get(index));
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
            if let Some(jump) = self.compile_match_guard(
                arm.guard.as_ref(),
                arm_payload.and_then(CompilerMatchArmPayload::guard_payload),
            )? {
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

    fn compile_match_arm_statement(
        &mut self,
        arm: &vela_syntax::ast::MatchArm,
        payload: Option<&CompilerMatchArmPayload<'_>>,
    ) -> CompileResult<bool> {
        if let Some(payload) = payload
            && let Some(kind) = payload.body_expression_kind()
        {
            if expression_payload_kind_matches(kind, &arm.body) {
                return self.compile_match_arm_statement_with_syntax_kind(arm, payload, kind);
            }
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST match arm body",
            )));
        }
        self.compile_legacy_match_arm_statement(arm, payload)
    }

    fn compile_match_arm_statement_with_syntax_kind(
        &mut self,
        arm: &vela_syntax::ast::MatchArm,
        payload: &CompilerMatchArmPayload<'_>,
        kind: SyntaxExpressionKind,
    ) -> CompileResult<bool> {
        if kind == SyntaxExpressionKind::Block {
            let ExprKind::Block(block) = &arm.body.kind else {
                unreachable!("validated CST match arm statement block kind");
            };
            if let Some(body) = payload.body_block_payload() {
                let statements = body.statement_payloads();
                return self.compile_statement_payloads(&statements);
            }
            return self.compile_statements(&block.statements);
        }
        let body_payload = payload.body_expression_payload();
        self.compile_expr_with_payload(&arm.body, Some(&body_payload))?;
        Ok(false)
    }

    fn compile_legacy_match_arm_statement(
        &mut self,
        arm: &vela_syntax::ast::MatchArm,
        payload: Option<&CompilerMatchArmPayload<'_>>,
    ) -> CompileResult<bool> {
        match &arm.body.kind {
            ExprKind::Block(block) => self.compile_statements(&block.statements),
            _ => {
                let body_payload = payload.map(CompilerMatchArmPayload::body_expression_payload);
                self.compile_expr_with_payload(&arm.body, body_payload.as_ref())?;
                Ok(false)
            }
        }
    }

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
        arm_payloads: Option<&[CompilerMatchArmPayload<'_>]>,
    ) -> CompileResult<bool> {
        let scrutinee_fact =
            self.script_fact_for_expr_with_payload(&match_expr.scrutinee, scrutinee_payload);
        let scrutinee = self.compile_expr_with_payload(&match_expr.scrutinee, scrutinee_payload)?;
        let mut end_jumps = Vec::new();
        let mut all_arms_return = !match_expr.arms.is_empty();
        let mut has_catch_all = false;

        for (index, arm) in match_expr.arms.iter().enumerate() {
            let arm_payload = arm_payloads.and_then(|payloads| payloads.get(index));
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
            if let Some(jump) = self.compile_match_guard(
                arm.guard.as_ref(),
                arm_payload.and_then(CompilerMatchArmPayload::guard_payload),
            )? {
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
        if let Some(payload) = payload.as_ref()
            && let Some(kind) = payload.kind()
            && !expression_payload_kind_matches(kind, guard)
        {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST match guard",
            )));
        }
        let condition = self.compile_expr_with_payload(guard, payload.as_ref())?;
        Ok(Some(self.emit_jump_if_false(condition)))
    }

    fn compile_match_arm_value_to(
        &mut self,
        body: &Expr,
        payload: Option<&CompilerMatchArmPayload<'_>>,
        dst: Register,
    ) -> CompileResult<bool> {
        if let Some(payload) = payload
            && let Some(kind) = payload.body_expression_kind()
        {
            if expression_payload_kind_matches(kind, body) {
                return self.compile_match_arm_value_with_syntax_kind(body, payload, kind, dst);
            }
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST match arm body",
            )));
        }
        self.compile_legacy_match_arm_value_to(body, payload, dst)
    }

    fn compile_match_arm_value_with_syntax_kind(
        &mut self,
        body: &Expr,
        payload: &CompilerMatchArmPayload<'_>,
        kind: SyntaxExpressionKind,
        dst: Register,
    ) -> CompileResult<bool> {
        match kind {
            SyntaxExpressionKind::Block => {
                let ExprKind::Block(block) = &body.kind else {
                    unreachable!("validated CST match arm block body kind");
                };
                if let Some(body) = payload.body_block_payload() {
                    self.compile_block_payload_value_to(&body, dst)
                } else {
                    self.compile_block_value_to(block, dst)
                }
            }
            SyntaxExpressionKind::If => {
                let ExprKind::If(if_expr) = &body.kind else {
                    unreachable!("validated CST match arm if body kind");
                };
                let body_payload = payload.body_expression_payload();
                let if_payload = body_payload.if_payload();
                self.compile_if_value_with_payloads(if_expr, dst, if_payload.as_ref())
            }
            SyntaxExpressionKind::Match => {
                let ExprKind::Match(match_expr) = &body.kind else {
                    unreachable!("validated CST match arm match body kind");
                };
                let body_payload = payload.body_expression_payload();
                let scrutinee_payload = body_payload.match_scrutinee_payload();
                let arm_payloads = body_payload.match_arm_payloads();
                self.compile_match_value_with_payloads(
                    match_expr,
                    dst,
                    scrutinee_payload.as_ref(),
                    arm_payloads.as_deref(),
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

    fn compile_legacy_match_arm_value_to(
        &mut self,
        body: &Expr,
        payload: Option<&CompilerMatchArmPayload<'_>>,
        dst: Register,
    ) -> CompileResult<bool> {
        match &body.kind {
            ExprKind::Block(block) => self.compile_block_value_to(block, dst),
            _ => {
                let body_payload = payload.map(CompilerMatchArmPayload::body_expression_payload);
                let value = self.compile_expr_with_payload(body, body_payload.as_ref())?;
                self.emit(UnlinkedInstructionKind::Move { dst, src: value });
                Ok(false)
            }
        }
    }
}
