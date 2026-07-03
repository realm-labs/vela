use vela_common::{SourceId, Span};
use vela_syntax::ast::{
    Argument, AssignOp, AstNode, BinaryOp, Expr, ExprKind, IfExpr, InterpolatedStringPart, Literal,
    MapEntry, MatchExpr, Pattern, RecordField, SyntaxExpression, SyntaxExpressionKind,
    SyntaxLambdaBody, SyntaxMapEntry, SyntaxMatchArm, SyntaxPattern, SyntaxPatternKind,
    SyntaxRecordExprField, SyntaxRecordPatternField,
};

#[cfg(test)]
use super::CompilerBodyFallback;
#[cfg(test)]
use super::expression_fallback_block;
use super::{
    CompilerArgumentPayload, CompilerBodyPayload, CompilerExpressionFallbackKind,
    CompilerExpressionPayload, CompilerIfPayload, CompilerMapEntryPayload, CompilerMatchArmPayload,
    CompilerPatternPayload, CompilerRecordFieldPayload, CompilerRecordPatternFieldPayload,
    if_payload_for_expr, match_arm_payloads_for_expr, match_scrutinee_payload_for_expr,
};

impl<'ast> CompilerExpressionPayload<'ast> {
    #[cfg(test)]
    pub(in crate::compiler) fn syntax(
        source: SourceId,
        syntax: SyntaxExpression,
        fallback: &'ast vela_syntax::ast::Expr,
    ) -> Self {
        Self::from_fallback(Some(source), Some(syntax), fallback)
    }

    #[cfg(test)]
    pub(in crate::compiler) fn missing_child_payload_context(
        syntax: SyntaxExpression,
        fallback: &'ast vela_syntax::ast::Expr,
    ) -> Self {
        Self::from_fallback(None, Some(syntax), fallback)
    }

    pub(in crate::compiler) fn block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        #[cfg(test)]
        let fallback = match self.fallback_kind {
            CompilerExpressionFallbackKind::Block(block) => {
                Some(CompilerBodyFallback::block(block))
            }
            _ => return None,
        };
        #[cfg(not(test))]
        let fallback = None;
        CompilerBodyPayload::nested_syntax_optional(
            self.source?,
            self.syntax.as_ref()?.as_block()?,
            fallback,
        )
    }

    pub(in crate::compiler) fn if_payload(&self) -> Option<CompilerIfPayload<'ast>> {
        let if_expr = self.fallback_if_expr()?;
        if_payload_for_expr(self.source, self.syntax.as_ref()?.as_if()?, if_expr)
    }

    pub(in crate::compiler) fn if_payload_with_fallback(
        &self,
    ) -> Option<(&'ast IfExpr, CompilerIfPayload<'ast>)> {
        let if_expr = self.fallback_if_expr()?;
        let payload = if_payload_for_expr(self.source, self.syntax.as_ref()?.as_if()?, if_expr)?;
        Some((if_expr, payload))
    }

    fn fallback_if_expr(&self) -> Option<&'ast IfExpr> {
        #[cfg(test)]
        {
            let CompilerExpressionFallbackKind::If(if_expr) = self.fallback_kind else {
                return None;
            };
            Some(if_expr)
        }
        #[cfg(not(test))]
        {
            let ExprKind::If(if_expr) = &self.fallback.kind else {
                return None;
            };
            Some(if_expr)
        }
    }

    pub(in crate::compiler) fn match_arm_payloads(
        &self,
    ) -> Option<Vec<CompilerMatchArmPayload<'ast>>> {
        let CompilerExpressionFallbackKind::Match(match_expr) = self.fallback_kind else {
            return None;
        };
        match_arm_payloads_for_expr(self.source, self.syntax.as_ref()?.as_match()?, match_expr)
    }

    pub(in crate::compiler) fn match_scrutinee_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let CompilerExpressionFallbackKind::Match(match_expr) = self.fallback_kind else {
            return None;
        };
        self.source?;
        Some(match_scrutinee_payload_for_expr(
            self.source,
            self.syntax.as_ref()?.as_match()?,
            match_expr,
        ))
    }

    pub(in crate::compiler) fn match_payloads_with_fallback(
        &self,
    ) -> Option<(
        &'ast MatchExpr,
        CompilerExpressionPayload<'ast>,
        Vec<CompilerMatchArmPayload<'ast>>,
    )> {
        let CompilerExpressionFallbackKind::Match(match_expr) = self.fallback_kind else {
            return None;
        };
        self.source?;
        let syntax = self.syntax.as_ref()?.as_match()?;
        let scrutinee_payload =
            match_scrutinee_payload_for_expr(self.source, syntax.clone(), match_expr);
        let arm_payloads = match_arm_payloads_for_expr(self.source, syntax, match_expr)?;
        Some((match_expr, scrutinee_payload, arm_payloads))
    }

    pub(in crate::compiler) fn syntax_span(&self) -> Option<Span> {
        Some(syntax_expression_span(self.source?, self.syntax.as_ref()?))
    }

    pub(in crate::compiler) fn syntax_path_segments(&self) -> Option<Vec<String>> {
        self.source?;
        let segments = self.syntax.as_ref()?.as_path()?.path_segments();
        (!segments.is_empty()).then_some(segments)
    }

    pub(in crate::compiler) fn syntax_is_self(&self) -> bool {
        if self.source.is_none() {
            return false;
        }
        self.syntax
            .as_ref()
            .and_then(SyntaxExpression::as_path)
            .is_some_and(|path| path.is_self())
    }

    pub(in crate::compiler) fn syntax_record_path_segments(&self) -> Option<Vec<String>> {
        self.source?;
        let segments = self.syntax.as_ref()?.as_record()?.path_segments();
        (!segments.is_empty()).then_some(segments)
    }

    pub(in crate::compiler) fn syntax_call_callee_path_segments(&self) -> Option<Vec<String>> {
        self.source?;
        let callee = self.syntax.as_ref()?.as_call()?.callee()?;
        let segments = callee.as_path()?.path_segments();
        (!segments.is_empty()).then_some(segments)
    }

    pub(in crate::compiler) fn syntax_call_callee_span(&self) -> Option<Span> {
        Some(syntax_expression_span(
            self.source?,
            &self.syntax.as_ref()?.as_call()?.callee()?,
        ))
    }

    pub(in crate::compiler) fn syntax_literal(&self) -> Option<Literal> {
        self.source?;
        self.syntax.as_ref()?.as_literal()?.literal()
    }

    pub(in crate::compiler) fn assignment_target_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let target = self.fallback_assignment_target()?;
        self.source?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_assign()?.target(),
            target,
        ))
    }

    pub(in crate::compiler) fn assignment_value_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let value = self.fallback_assignment_value()?;
        self.source?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_assign()?.value(),
            value,
        ))
    }

    pub(in crate::compiler) fn syntax_assignment_operator(&self) -> Option<AssignOp> {
        if !self.fallback_is_assignment() {
            return None;
        }
        self.source?;
        self.syntax.as_ref()?.as_assign()?.operator()
    }

    fn fallback_assignment_target(&self) -> Option<&'ast Expr> {
        #[cfg(test)]
        {
            let CompilerExpressionFallbackKind::Assign { target, .. } = self.fallback_kind else {
                return None;
            };
            Some(target)
        }
        #[cfg(not(test))]
        {
            let ExprKind::Assign { target, .. } = &self.fallback.kind else {
                return None;
            };
            Some(target)
        }
    }

    fn fallback_assignment_value(&self) -> Option<&'ast Expr> {
        #[cfg(test)]
        {
            let CompilerExpressionFallbackKind::Assign { value, .. } = self.fallback_kind else {
                return None;
            };
            Some(value)
        }
        #[cfg(not(test))]
        {
            let ExprKind::Assign { value, .. } = &self.fallback.kind else {
                return None;
            };
            Some(value)
        }
    }

    fn fallback_is_assignment(&self) -> bool {
        #[cfg(test)]
        {
            matches!(
                self.fallback_kind,
                CompilerExpressionFallbackKind::Assign { .. }
            )
        }
        #[cfg(not(test))]
        {
            matches!(self.fallback_kind, CompilerExpressionFallbackKind::Assign)
        }
    }

    pub(in crate::compiler) fn paren_inner_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        self.source?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_paren()?.expression(),
            self.fallback,
        ))
    }

    pub(in crate::compiler) fn unary_operand_payload(
        &self,
        expr: &'ast Expr,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        if !self.fallback_is_unary() {
            return None;
        }
        self.source?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_unary()?.expression(),
            expr,
        ))
    }

    fn fallback_is_unary(&self) -> bool {
        #[cfg(test)]
        {
            matches!(
                self.fallback_kind,
                CompilerExpressionFallbackKind::Unary { .. }
            )
        }
        #[cfg(not(test))]
        {
            matches!(self.fallback_kind, CompilerExpressionFallbackKind::Unary)
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn fallback_unary_operand(&self) -> Option<&'ast Expr> {
        let CompilerExpressionFallbackKind::Unary { expr } = self.fallback_kind else {
            return None;
        };
        Some(expr)
    }

    pub(in crate::compiler) fn syntax_unary_operator(&self) -> Option<vela_syntax::ast::UnaryOp> {
        self.source?;
        self.syntax.as_ref()?.as_unary()?.operator()
    }

    pub(in crate::compiler) fn try_operand_payload(
        &self,
        expr: &'ast Expr,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        if !self.fallback_is_try() {
            return None;
        }
        self.source?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_try()?.expression(),
            expr,
        ))
    }

    fn fallback_is_try(&self) -> bool {
        #[cfg(test)]
        {
            matches!(self.fallback_kind, CompilerExpressionFallbackKind::Try(_))
        }
        #[cfg(not(test))]
        {
            matches!(self.fallback_kind, CompilerExpressionFallbackKind::Try)
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn fallback_try_operand(&self) -> Option<&'ast Expr> {
        let CompilerExpressionFallbackKind::Try(expr) = self.fallback_kind else {
            return None;
        };
        Some(expr)
    }

    pub(in crate::compiler) fn binary_operand_payloads(
        &self,
        left: &'ast Expr,
        right: &'ast Expr,
    ) -> Option<(
        CompilerExpressionPayload<'ast>,
        CompilerExpressionPayload<'ast>,
    )> {
        if !self.fallback_is_binary() {
            return None;
        }
        self.source?;
        let syntax = self.syntax.as_ref()?.as_binary()?;
        Some((
            CompilerExpressionPayload::from_fallback(self.source, syntax.lhs(), left),
            CompilerExpressionPayload::from_fallback(self.source, syntax.rhs(), right),
        ))
    }

    fn fallback_is_binary(&self) -> bool {
        #[cfg(test)]
        {
            matches!(
                self.fallback_kind,
                CompilerExpressionFallbackKind::Binary { .. }
            )
        }
        #[cfg(not(test))]
        {
            matches!(self.fallback_kind, CompilerExpressionFallbackKind::Binary)
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn fallback_binary_operands(
        &self,
    ) -> Option<(BinaryOp, &'ast Expr, &'ast Expr)> {
        let CompilerExpressionFallbackKind::Binary { op, left, right } = self.fallback_kind else {
            return None;
        };
        Some((op, left, right))
    }

    pub(in crate::compiler) fn syntax_binary_operator(&self) -> Option<BinaryOp> {
        self.source?;
        let binary = self.syntax.as_ref()?.as_binary()?;
        let mut expressions = binary.expressions();
        expressions.next()?;
        expressions.next()?;
        if expressions.next().is_some() {
            return None;
        }
        binary.operator()
    }

    pub(in crate::compiler) fn logical_chain_operand_payloads(
        &self,
        op: BinaryOp,
        expr: &'ast Expr,
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
        fn collect_expr<'ast>(
            expr: &'ast vela_syntax::ast::Expr,
            op: BinaryOp,
            operands: &mut Vec<&'ast vela_syntax::ast::Expr>,
        ) {
            if let ExprKind::Binary {
                op: expr_op,
                left,
                right,
            } = &expr.kind
                && *expr_op == op
            {
                collect_expr(left, op, operands);
                collect_expr(right, op, operands);
            } else {
                operands.push(expr);
            }
        }

        fn collect_syntax(
            syntax: SyntaxExpression,
            op: BinaryOp,
            operands: &mut Vec<SyntaxExpression>,
        ) -> Option<()> {
            if let Some(binary) = syntax.as_binary()
                && binary.operator() == Some(op)
            {
                let expressions = binary.expressions().collect::<Vec<_>>();
                if expressions.len() < 2 {
                    return None;
                }
                for expression in expressions {
                    collect_syntax(expression, op, operands)?;
                }
                return Some(());
            }

            operands.push(syntax);
            Some(())
        }

        if !self.fallback_is_binary() {
            return None;
        };
        let ExprKind::Binary { op: expr_op, .. } = &expr.kind else {
            return None;
        };
        if *expr_op != op {
            return None;
        }

        let mut expr_operands = Vec::new();
        collect_expr(expr, op, &mut expr_operands);

        self.source?;
        let mut syntax_operands = Vec::new();
        collect_syntax(self.syntax.clone()?, op, &mut syntax_operands)?;
        if syntax_operands.len() != expr_operands.len() {
            return None;
        }

        Some(
            expr_operands
                .into_iter()
                .zip(syntax_operands.into_iter().map(Some))
                .map(|(fallback, syntax)| {
                    CompilerExpressionPayload::from_fallback(self.source, syntax, fallback)
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn call_argument_payloads(
        &self,
    ) -> Option<Vec<CompilerArgumentPayload<'ast>>> {
        let args = self.fallback_call_args()?;
        let syntax_args = self.syntax.as_ref()?.as_call()?.arguments();
        if syntax_args.len() > args.len() {
            return None;
        }
        Some(
            args.iter()
                .enumerate()
                .map(|(index, fallback)| CompilerArgumentPayload {
                    source: self.source,
                    syntax: syntax_args.get(index).cloned(),
                    value_fallback: &fallback.value,
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn call_callee_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let callee = self.fallback_call_callee()?;
        self.source?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_call()?.callee(),
            callee,
        ))
    }

    fn fallback_call_args(&self) -> Option<&'ast [Argument]> {
        #[cfg(test)]
        {
            let CompilerExpressionFallbackKind::Call { args, .. } = self.fallback_kind else {
                return None;
            };
            Some(args)
        }
        #[cfg(not(test))]
        {
            let ExprKind::Call { args, .. } = &self.fallback.kind else {
                return None;
            };
            Some(args)
        }
    }

    fn fallback_call_callee(&self) -> Option<&'ast Expr> {
        #[cfg(test)]
        {
            let CompilerExpressionFallbackKind::Call { callee, .. } = self.fallback_kind else {
                return None;
            };
            Some(callee)
        }
        #[cfg(not(test))]
        {
            let ExprKind::Call { callee, .. } = &self.fallback.kind else {
                return None;
            };
            Some(callee)
        }
    }

    pub(in crate::compiler) fn field_base_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let base = self.fallback_field_base()?;
        self.source?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.as_field()?.receiver(),
            base,
        ))
    }

    fn fallback_field_base(&self) -> Option<&'ast Expr> {
        #[cfg(test)]
        {
            let CompilerExpressionFallbackKind::Field { base } = self.fallback_kind else {
                return None;
            };
            Some(base)
        }
        #[cfg(not(test))]
        {
            let ExprKind::Field { base, .. } = &self.fallback.kind else {
                return None;
            };
            Some(base)
        }
    }

    pub(in crate::compiler) fn syntax_field_name(&self) -> Option<String> {
        self.source?;
        self.syntax.as_ref()?.as_field()?.name_text()
    }

    pub(in crate::compiler) fn index_operand_payloads(
        &self,
    ) -> Option<(
        CompilerExpressionPayload<'ast>,
        CompilerExpressionPayload<'ast>,
    )> {
        self.source?;
        let (base, index) = self.fallback_index_operands()?;
        let syntax = self.syntax.as_ref()?.as_index()?;
        Some((
            CompilerExpressionPayload::from_fallback(self.source, syntax.receiver(), base),
            CompilerExpressionPayload::from_fallback(self.source, syntax.index(), index),
        ))
    }

    fn fallback_index_operands(&self) -> Option<(&'ast Expr, &'ast Expr)> {
        #[cfg(test)]
        {
            let CompilerExpressionFallbackKind::Index { base, index } = self.fallback_kind else {
                return None;
            };
            Some((base, index))
        }
        #[cfg(not(test))]
        {
            let ExprKind::Index { base, index } = &self.fallback.kind else {
                return None;
            };
            Some((base, index))
        }
    }

    pub(in crate::compiler) fn lambda_body_payload(
        &self,
        body: &'ast Expr,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        if !self.fallback_is_lambda() {
            return None;
        }
        self.source?;
        let syntax = match self.syntax.as_ref()?.as_lambda()?.body()? {
            SyntaxLambdaBody::Expression(expression) => Some(expression),
            SyntaxLambdaBody::Block(block) => SyntaxExpression::cast(block.syntax().clone()),
        };
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            syntax,
            body,
        ))
    }

    fn fallback_is_lambda(&self) -> bool {
        #[cfg(test)]
        {
            matches!(
                self.fallback_kind,
                CompilerExpressionFallbackKind::Lambda { .. }
            )
        }
        #[cfg(not(test))]
        {
            matches!(self.fallback_kind, CompilerExpressionFallbackKind::Lambda)
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn fallback_lambda_body(&self) -> Option<&'ast Expr> {
        let CompilerExpressionFallbackKind::Lambda { body } = self.fallback_kind else {
            return None;
        };
        Some(body)
    }

    pub(in crate::compiler) fn array_element_payloads(
        &self,
        items: &'ast [Expr],
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
        if !self.fallback_is_array() {
            return None;
        }
        let syntax_items = self
            .syntax
            .as_ref()?
            .as_array()?
            .expressions()
            .collect::<Vec<_>>();
        Some(
            items
                .iter()
                .enumerate()
                .map(|(index, fallback)| {
                    CompilerExpressionPayload::from_fallback(
                        self.source,
                        syntax_items.get(index).cloned(),
                        fallback,
                    )
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn has_extra_array_elements(&self, items: &'ast [Expr]) -> bool {
        if self.source.is_none() {
            return false;
        }
        if !self.fallback_is_array() {
            return false;
        }
        let Some(syntax) = self.syntax.as_ref().and_then(SyntaxExpression::as_array) else {
            return false;
        };
        syntax.expressions().count() > items.len()
    }

    fn fallback_is_array(&self) -> bool {
        #[cfg(test)]
        {
            matches!(self.fallback_kind, CompilerExpressionFallbackKind::Array(_))
        }
        #[cfg(not(test))]
        {
            matches!(self.fallback_kind, CompilerExpressionFallbackKind::Array)
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn fallback_array_items(&self) -> Option<&'ast [Expr]> {
        let CompilerExpressionFallbackKind::Array(items) = self.fallback_kind else {
            return None;
        };
        Some(items)
    }

    pub(in crate::compiler) fn map_entry_payloads(
        &self,
        entries: &'ast [MapEntry],
    ) -> Option<Vec<CompilerMapEntryPayload<'ast>>> {
        if !self.fallback_is_map() {
            return None;
        }
        let syntax_entries = self
            .syntax
            .as_ref()?
            .as_map()?
            .entries()
            .collect::<Vec<_>>();
        Some(
            entries
                .iter()
                .enumerate()
                .map(|(index, fallback)| CompilerMapEntryPayload {
                    source: self.source,
                    syntax: syntax_entries.get(index).cloned(),
                    value_fallback: &fallback.value,
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn has_mismatched_map_entries(
        &self,
        entries: &'ast [MapEntry],
    ) -> bool {
        if self.source.is_none() {
            return false;
        }
        if !self.fallback_is_map() {
            return false;
        }
        let Some(syntax) = self.syntax.as_ref().and_then(SyntaxExpression::as_map) else {
            return false;
        };
        syntax.entries().count() != entries.len()
    }

    fn fallback_is_map(&self) -> bool {
        #[cfg(test)]
        {
            matches!(self.fallback_kind, CompilerExpressionFallbackKind::Map(_))
        }
        #[cfg(not(test))]
        {
            matches!(self.fallback_kind, CompilerExpressionFallbackKind::Map)
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn fallback_map_entries(&self) -> Option<&'ast [MapEntry]> {
        let CompilerExpressionFallbackKind::Map(entries) = self.fallback_kind else {
            return None;
        };
        Some(entries)
    }

    pub(in crate::compiler) fn record_field_payloads(
        &self,
        fields: &'ast [RecordField],
    ) -> Option<Vec<CompilerRecordFieldPayload<'ast>>> {
        if !self.fallback_is_record() {
            return None;
        }
        let syntax_fields = self.syntax.as_ref()?.as_record()?.fields();
        Some(
            fields
                .iter()
                .enumerate()
                .map(|(index, fallback)| CompilerRecordFieldPayload {
                    source: self.source,
                    syntax: syntax_fields.get(index).cloned(),
                    value_fallback: fallback.value.as_ref(),
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn has_extra_record_fields(&self, fields: &'ast [RecordField]) -> bool {
        if self.source.is_none() {
            return false;
        }
        if !self.fallback_is_record() {
            return false;
        }
        let Some(syntax) = self.syntax.as_ref().and_then(SyntaxExpression::as_record) else {
            return false;
        };
        syntax.fields().len() > fields.len()
    }

    fn fallback_is_record(&self) -> bool {
        #[cfg(test)]
        {
            matches!(
                self.fallback_kind,
                CompilerExpressionFallbackKind::Record { .. }
            )
        }
        #[cfg(not(test))]
        {
            matches!(self.fallback_kind, CompilerExpressionFallbackKind::Record)
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn fallback_record_fields(&self) -> Option<&'ast [RecordField]> {
        let CompilerExpressionFallbackKind::Record { fields } = self.fallback_kind else {
            return None;
        };
        Some(fields)
    }

    pub(in crate::compiler) fn interpolated_expression_payloads(
        &self,
        parts: &'ast [InterpolatedStringPart],
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
        if !self.fallback_is_interpolated_string() {
            return None;
        }
        let syntax_expressions = self
            .syntax
            .as_ref()?
            .as_literal()?
            .interpolation_expressions()
            .collect::<Vec<_>>();
        Some(
            parts
                .iter()
                .filter_map(|part| match part {
                    InterpolatedStringPart::Text(_) => None,
                    InterpolatedStringPart::Expr(expr) => Some(expr),
                })
                .enumerate()
                .map(|(index, fallback)| {
                    CompilerExpressionPayload::from_fallback(
                        self.source,
                        syntax_expressions.get(index).cloned(),
                        fallback,
                    )
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn has_extra_interpolation_expressions(
        &self,
        parts: &'ast [InterpolatedStringPart],
    ) -> bool {
        if self.source.is_none() {
            return false;
        }
        if !self.fallback_is_interpolated_string() {
            return false;
        }
        let Some(syntax) = self.syntax.as_ref().and_then(SyntaxExpression::as_literal) else {
            return false;
        };
        let expression_count = parts
            .iter()
            .filter(|part| matches!(part, InterpolatedStringPart::Expr(_)))
            .count();
        syntax.interpolation_expressions().count() > expression_count
    }

    fn fallback_is_interpolated_string(&self) -> bool {
        #[cfg(test)]
        {
            matches!(
                self.fallback_kind,
                CompilerExpressionFallbackKind::InterpolatedString(_)
            )
        }
        #[cfg(not(test))]
        {
            matches!(
                self.fallback_kind,
                CompilerExpressionFallbackKind::InterpolatedString
            )
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn fallback_interpolated_string_parts(
        &self,
    ) -> Option<&'ast [InterpolatedStringPart]> {
        let CompilerExpressionFallbackKind::InterpolatedString(parts) = self.fallback_kind else {
            return None;
        };
        Some(parts)
    }
}

fn syntax_expression_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}

impl<'ast> CompilerMapEntryPayload<'ast> {
    #[cfg(test)]
    pub(in crate::compiler) fn syntax(
        source: SourceId,
        syntax: SyntaxMapEntry,
        fallback: &'ast vela_syntax::ast::MapEntry,
    ) -> Self {
        Self {
            source: Some(source),
            syntax: Some(syntax),
            value_fallback: &fallback.value,
        }
    }

    pub(in crate::compiler) fn syntax_key_name(&self) -> Option<String> {
        self.source?;
        let key = self.syntax.as_ref()?.key()?;
        if let Some(literal) = key.as_literal().and_then(|literal| literal.literal()) {
            return match literal {
                Literal::String(value) => Some(value),
                Literal::Char(value) => Some(value.to_string()),
                Literal::Integer(value) => Some(value.source_text_with_suffix()),
                Literal::Float(value) => Some(value.source_text_with_suffix()),
                _ => None,
            };
        }
        key.as_path().and_then(|path| path.path_text())
    }

    pub(in crate::compiler) fn has_key_syntax(&self) -> bool {
        self.source.is_some()
            && self
                .syntax
                .as_ref()
                .is_some_and(|entry| entry.key().is_some())
    }

    pub(in crate::compiler) fn has_value_syntax(&self) -> bool {
        self.source.is_some()
            && self
                .syntax
                .as_ref()
                .is_some_and(|entry| entry.value().is_some())
    }

    pub(in crate::compiler) fn value_expression_payload(&self) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload::from_fallback(
            self.source,
            self.source
                .and_then(|_| self.syntax.as_ref().and_then(SyntaxMapEntry::value)),
            self.value_fallback,
        )
    }
}

impl<'ast> CompilerRecordFieldPayload<'ast> {
    #[cfg(test)]
    pub(in crate::compiler) fn syntax(
        source: SourceId,
        syntax: SyntaxRecordExprField,
        fallback: &'ast vela_syntax::ast::RecordField,
    ) -> Self {
        Self {
            source: Some(source),
            syntax: Some(syntax),
            value_fallback: fallback.value.as_ref(),
        }
    }

    pub(in crate::compiler) fn syntax_label_name(&self) -> Option<String> {
        self.source?;
        self.syntax
            .as_ref()
            .and_then(SyntaxRecordExprField::label_text)
    }

    pub(in crate::compiler) fn has_syntax(&self) -> bool {
        self.source.is_some() && self.syntax.is_some()
    }

    pub(in crate::compiler) fn has_value_syntax(&self) -> bool {
        self.source.is_some()
            && self
                .syntax
                .as_ref()
                .is_some_and(|field| field.expression().is_some())
    }

    pub(in crate::compiler) fn value_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.source.and_then(|_| {
                self.syntax
                    .as_ref()
                    .and_then(SyntaxRecordExprField::expression)
            }),
            self.value_fallback?,
        ))
    }
}

impl<'ast> CompilerMatchArmPayload<'ast> {
    #[cfg(test)]
    pub(in crate::compiler) fn syntax(
        source: SourceId,
        syntax: SyntaxMatchArm,
        fallback: &'ast vela_syntax::ast::MatchArm,
    ) -> Self {
        Self {
            source: Some(source),
            syntax: Some(syntax),
            pattern_fallback: &fallback.pattern,
            guard_fallback: fallback.guard.as_ref(),
            body_fallback: &fallback.body,
            #[cfg(test)]
            body_block_fallback: expression_fallback_block(&fallback.body),
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn missing_child_payload_context(
        syntax: SyntaxMatchArm,
        fallback: &'ast vela_syntax::ast::MatchArm,
    ) -> Self {
        Self {
            source: None,
            syntax: Some(syntax),
            pattern_fallback: &fallback.pattern,
            guard_fallback: fallback.guard.as_ref(),
            body_fallback: &fallback.body,
            #[cfg(test)]
            body_block_fallback: expression_fallback_block(&fallback.body),
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn missing_syntax(fallback: &'ast vela_syntax::ast::MatchArm) -> Self {
        Self {
            source: None,
            syntax: None,
            pattern_fallback: &fallback.pattern,
            guard_fallback: fallback.guard.as_ref(),
            body_fallback: &fallback.body,
            #[cfg(test)]
            body_block_fallback: expression_fallback_block(&fallback.body),
        }
    }

    pub(in crate::compiler) fn pattern_payload(&self) -> CompilerPatternPayload<'ast> {
        CompilerPatternPayload::from_fallback(
            self.source,
            self.source
                .and_then(|_| self.syntax.as_ref().and_then(SyntaxMatchArm::pattern)),
            self.pattern_fallback,
        )
    }

    pub(in crate::compiler) fn has_syntax(&self) -> bool {
        self.source.is_some() && self.syntax.is_some()
    }

    #[cfg(test)]
    pub(in crate::compiler) fn body_expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax_body_expression_kind()
    }

    pub(in crate::compiler) fn syntax_body_expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax
            .as_ref()?
            .body_as_expression()
            .map(|body| body.expression_kind())
    }

    pub(in crate::compiler) fn guard_payload(&self) -> Option<CompilerExpressionPayload<'ast>> {
        self.source?;
        Some(CompilerExpressionPayload::from_fallback(
            self.source,
            self.syntax.as_ref()?.guard(),
            self.guard_fallback?,
        ))
    }

    pub(in crate::compiler) fn body_block_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        #[cfg(test)]
        let fallback = self.body_block_fallback.map(CompilerBodyFallback::block);
        #[cfg(not(test))]
        let fallback = None;
        CompilerBodyPayload::nested_syntax_optional(
            self.source?,
            self.syntax.as_ref()?.body_block()?,
            fallback,
        )
    }

    pub(in crate::compiler) fn body_expression_payload(&self) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload::from_fallback(
            self.source,
            self.source.and_then(|_| {
                self.syntax
                    .as_ref()
                    .and_then(SyntaxMatchArm::body_as_expression)
            }),
            self.body_fallback,
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn syntax_arm(&self) -> Option<&SyntaxMatchArm> {
        self.source?;
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerPatternPayload<'ast> {
    pub(in crate::compiler) fn from_fallback(
        source: Option<SourceId>,
        syntax: Option<SyntaxPattern>,
        fallback: &'ast Pattern,
    ) -> Self {
        let (record_fields_fallback, tuple_fields_fallback) = match fallback {
            Pattern::RecordVariant { fields, .. } => (Some(fields.as_slice()), None),
            Pattern::TupleVariant { fields, .. } => (None, Some(fields.as_slice())),
            Pattern::Wildcard | Pattern::Literal(_) | Pattern::Binding(_) | Pattern::Path(_) => {
                (None, None)
            }
        };
        Self {
            source,
            syntax,
            record_fields_fallback,
            tuple_fields_fallback,
        }
    }

    pub(in crate::compiler) fn has_syntax(&self) -> bool {
        self.source.is_some() && self.syntax.is_some()
    }

    pub(in crate::compiler) fn syntax_pattern_kind(&self) -> Option<SyntaxPatternKind> {
        self.source?;
        self.syntax.as_ref()?.pattern_kind()
    }

    pub(in crate::compiler) fn syntax_literal(&self) -> Option<vela_syntax::ast::Literal> {
        self.source?;
        self.syntax.as_ref()?.literal()
    }

    pub(in crate::compiler) fn syntax_path_segments(&self) -> Option<Vec<String>> {
        self.source?;
        let segments = self.syntax.as_ref()?.path_segments();
        (!segments.is_empty()).then_some(segments)
    }

    pub(in crate::compiler) fn syntax_binding_name(&self) -> Option<String> {
        self.source?;
        self.syntax.as_ref()?.binding_name()
    }

    pub(in crate::compiler) fn record_field_payloads(
        &self,
    ) -> Option<Vec<CompilerRecordPatternFieldPayload<'ast>>> {
        let syntax_fields = self
            .syntax
            .as_ref()?
            .record_pattern()?
            .fields()
            .collect::<Vec<_>>();
        let fields = self.record_fields_fallback?;
        Some(
            fields
                .iter()
                .enumerate()
                .map(|(index, fallback)| CompilerRecordPatternFieldPayload {
                    source: self.source,
                    syntax: syntax_fields.get(index).cloned(),
                    pattern_fallback: fallback.pattern.as_ref(),
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn has_extra_record_pattern_fields(&self) -> bool {
        if self.source.is_none() {
            return false;
        }
        let Some(fields) = self.record_fields_fallback else {
            return false;
        };
        let Some(syntax) = self.syntax.as_ref().and_then(SyntaxPattern::record_pattern) else {
            return false;
        };
        syntax.fields().count() > fields.len()
    }

    pub(in crate::compiler) fn tuple_pattern_payloads(
        &self,
    ) -> Option<Vec<CompilerPatternPayload<'ast>>> {
        let syntax_fields = self
            .syntax
            .as_ref()?
            .tuple_pattern()?
            .patterns()
            .collect::<Vec<_>>();
        let fields = self.tuple_fields_fallback?;
        Some(
            fields
                .iter()
                .enumerate()
                .map(|(index, fallback)| {
                    CompilerPatternPayload::from_fallback(
                        self.source,
                        syntax_fields.get(index).cloned(),
                        fallback,
                    )
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn has_extra_tuple_pattern_fields(&self) -> bool {
        if self.source.is_none() {
            return false;
        }
        let Some(fields) = self.tuple_fields_fallback else {
            return false;
        };
        let Some(syntax) = self.syntax.as_ref().and_then(SyntaxPattern::tuple_pattern) else {
            return false;
        };
        syntax.patterns().count() > fields.len()
    }

    #[cfg(test)]
    pub(in crate::compiler) fn syntax(
        syntax: vela_syntax::ast::SyntaxPattern,
        fallback: &'ast Pattern,
    ) -> Self {
        Self::from_fallback(Some(SourceId::new(1)), Some(syntax), fallback)
    }

    #[cfg(test)]
    pub(in crate::compiler) fn missing_child_payload_context(
        syntax: vela_syntax::ast::SyntaxPattern,
        fallback: &'ast Pattern,
    ) -> Self {
        Self::from_fallback(None, Some(syntax), fallback)
    }

    #[cfg(test)]
    pub(in crate::compiler) fn missing_syntax(source: SourceId, fallback: &'ast Pattern) -> Self {
        Self::from_fallback(Some(source), None, fallback)
    }

    #[cfg(test)]
    pub(in crate::compiler) fn syntax_pattern(&self) -> Option<&vela_syntax::ast::SyntaxPattern> {
        self.source?;
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerRecordPatternFieldPayload<'ast> {
    pub(in crate::compiler) fn has_syntax(&self) -> bool {
        self.source.is_some() && self.syntax.is_some()
    }

    pub(in crate::compiler) fn syntax_label_name(&self) -> Option<String> {
        self.source?;
        self.syntax
            .as_ref()
            .and_then(SyntaxRecordPatternField::label_text)
    }

    pub(in crate::compiler) fn syntax_is_shorthand(&self) -> Option<bool> {
        self.source?;
        self.syntax
            .as_ref()
            .map(SyntaxRecordPatternField::is_shorthand)
    }

    pub(in crate::compiler) fn syntax_pattern_kind(&self) -> Option<SyntaxPatternKind> {
        self.source?;
        self.syntax.as_ref()?.pattern()?.pattern_kind()
    }

    pub(in crate::compiler) fn pattern_payload(&self) -> Option<CompilerPatternPayload<'ast>> {
        self.source?;
        Some(CompilerPatternPayload::from_fallback(
            self.source,
            self.syntax
                .as_ref()
                .and_then(SyntaxRecordPatternField::pattern),
            self.pattern_fallback?,
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn syntax(
        syntax: SyntaxRecordPatternField,
        fallback: &'ast vela_syntax::ast::RecordPatternField,
    ) -> Self {
        Self {
            source: Some(SourceId::new(1)),
            syntax: Some(syntax),
            pattern_fallback: fallback.pattern.as_ref(),
        }
    }
}
