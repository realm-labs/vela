use vela_common::{SourceId, Span};
#[cfg(test)]
use vela_syntax::ast::AssignOp;
#[cfg(test)]
use vela_syntax::ast::BinaryOp;
#[cfg(test)]
use vela_syntax::ast::Literal;
#[cfg(test)]
use vela_syntax::ast::SyntaxExpressionKind;
#[cfg(test)]
use vela_syntax::ast::SyntaxLambdaBody;
#[cfg(test)]
use vela_syntax::ast::SyntaxMapEntry;
#[cfg(test)]
use vela_syntax::ast::SyntaxRecordExprField;
use vela_syntax::ast::{AstNode, SyntaxExpression};
#[cfg(test)]
use vela_syntax::ast::{
    SyntaxMatchArm, SyntaxPattern, SyntaxPatternKind, SyntaxRecordPatternField,
};

#[cfg(test)]
#[cfg(test)]
use super::CompilerArgumentPayload;
#[cfg(test)]
use super::CompilerBodyPayload;
use super::CompilerExpressionPayload;
#[cfg(test)]
use super::CompilerInterpolationPayload;
#[cfg(test)]
use super::CompilerRecordFieldPayload;
#[cfg(test)]
use super::{CompilerArrayElementPayload, CompilerMapEntryPayload};
#[cfg(test)]
use super::{CompilerIfPayload, if_payload_for_syntax};
#[cfg(test)]
use super::{
    CompilerMatchArmPayload, CompilerPatternPayload, CompilerRecordPatternFieldPayload,
    match_arm_payloads_for_syntax,
};

impl<'ast> CompilerExpressionPayload<'ast> {
    #[cfg(test)]
    pub(in crate::compiler) fn missing_child_payload_context(syntax: SyntaxExpression) -> Self {
        Self::from_syntax(None, Some(syntax))
    }

    fn child_payload(&self, syntax: Option<SyntaxExpression>) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload::from_syntax(self.source, syntax)
    }

    #[cfg(test)]
    pub(in crate::compiler) fn block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let body = self.syntax.as_ref()?.as_block()?;
        Some(CompilerBodyPayload::nested_syntax(self.source?, body))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn if_payload(&self) -> Option<CompilerIfPayload<'ast>> {
        if !self.matches_syntax_kind(SyntaxExpressionKind::If) {
            return None;
        }
        if_payload_for_syntax(self.source, self.syntax.as_ref()?.as_if()?)
    }

    #[cfg(test)]
    pub(in crate::compiler) fn match_arm_payloads(&self) -> Option<Vec<CompilerMatchArmPayload>> {
        if !self.matches_syntax_kind(SyntaxExpressionKind::Match) {
            return None;
        }
        match_arm_payloads_for_syntax(self.source, self.syntax.as_ref()?.as_match()?)
    }

    #[cfg(test)]
    pub(in crate::compiler) fn match_scrutinee_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        self.source?;
        let syntax = self.syntax.as_ref()?.as_match()?;
        Some(self.child_payload(syntax.scrutinee()))
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

    #[cfg(test)]
    pub(in crate::compiler) fn syntax_literal(&self) -> Option<Literal> {
        self.source?;
        self.syntax.as_ref()?.as_literal()?.literal()
    }

    #[cfg(test)]
    pub(in crate::compiler) fn assignment_target_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        self.source?;
        let syntax = self.syntax.as_ref()?.as_assign()?.target();
        Some(self.child_payload(syntax))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn assignment_value_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        self.source?;
        let syntax = self.syntax.as_ref()?.as_assign()?.value();
        Some(self.child_payload(syntax))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn syntax_assignment_operator(&self) -> Option<AssignOp> {
        if !self.matches_syntax_kind(SyntaxExpressionKind::Assign) {
            return None;
        }
        self.source?;
        self.syntax.as_ref()?.as_assign()?.operator()
    }

    #[cfg(test)]
    pub(in crate::compiler) fn paren_inner_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        self.source?;
        let syntax = self.syntax.as_ref()?.as_paren()?.expression();
        Some(self.child_payload(syntax))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn unary_operand_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        self.source?;
        let syntax = self.syntax.as_ref()?.as_unary()?.expression();
        Some(self.child_payload(syntax))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn syntax_unary_operator(&self) -> Option<vela_syntax::ast::UnaryOp> {
        self.source?;
        self.syntax.as_ref()?.as_unary()?.operator()
    }

    #[cfg(test)]
    pub(in crate::compiler) fn try_operand_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        self.source?;
        let syntax = self.syntax.as_ref()?.as_try()?.expression();
        Some(self.child_payload(syntax))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn binary_operand_payloads(
        &self,
    ) -> Option<(
        CompilerExpressionPayload<'ast>,
        CompilerExpressionPayload<'ast>,
    )> {
        self.source?;
        let syntax = self.syntax.as_ref()?.as_binary()?;
        Some((
            self.child_payload(syntax.lhs()),
            self.child_payload(syntax.rhs()),
        ))
    }

    #[cfg(test)]
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

    #[cfg(test)]
    pub(in crate::compiler) fn logical_chain_operand_payloads(
        &self,
        op: BinaryOp,
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
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

        self.source?;
        let mut syntax_operands = Vec::new();
        collect_syntax(self.syntax.clone()?, op, &mut syntax_operands)?;
        Some(
            syntax_operands
                .into_iter()
                .map(|syntax| self.child_payload(Some(syntax)))
                .collect(),
        )
    }

    #[cfg(test)]
    #[cfg(test)]
    pub(in crate::compiler) fn call_argument_payloads(
        &self,
    ) -> Option<Vec<CompilerArgumentPayload>> {
        if !self.matches_syntax_kind(SyntaxExpressionKind::Call) {
            return None;
        }
        Some(
            self.syntax
                .as_ref()?
                .as_call()?
                .arguments()
                .into_iter()
                .map(|syntax| CompilerArgumentPayload {
                    source: self.source,
                    syntax: Some(syntax),
                })
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn call_argument_value_payloads(
        &self,
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
        Some(
            self.call_argument_payloads()?
                .into_iter()
                .map(|payload| payload.value_expression_payload())
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn call_callee_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        self.source?;
        let syntax = self.syntax.as_ref()?.as_call()?.callee();
        Some(self.child_payload(syntax))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn field_base_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        self.source?;
        let syntax = self.syntax.as_ref()?.as_field()?.receiver();
        Some(self.child_payload(syntax))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn syntax_field_name(&self) -> Option<String> {
        self.source?;
        self.syntax.as_ref()?.as_field()?.name_text()
    }

    #[allow(dead_code)]
    pub(in crate::compiler) fn index_operand_payloads(
        &self,
    ) -> Option<(
        CompilerExpressionPayload<'ast>,
        CompilerExpressionPayload<'ast>,
    )> {
        self.source?;
        let syntax = self.syntax.as_ref()?.as_index()?;
        Some((
            self.child_payload(syntax.receiver()),
            self.child_payload(syntax.index()),
        ))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn lambda_body_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        self.source?;
        let syntax = match self.syntax.as_ref()?.as_lambda()?.body()? {
            SyntaxLambdaBody::Expression(expression) => Some(expression),
            SyntaxLambdaBody::Block(block) => SyntaxExpression::cast(block.syntax().clone()),
        };
        Some(self.child_payload(syntax))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn array_element_payloads(
        &self,
    ) -> Option<Vec<CompilerArrayElementPayload>> {
        if !self.matches_syntax_kind(SyntaxExpressionKind::Array) {
            return None;
        }
        let source = self.source?;
        Some(
            self.syntax
                .as_ref()?
                .as_array()?
                .expressions()
                .map(|syntax| CompilerArrayElementPayload {
                    source: Some(source),
                    syntax: Some(syntax),
                })
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn array_element_value_payloads(
        &self,
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
        Some(
            self.array_element_payloads()?
                .into_iter()
                .map(|payload| payload.value_expression_payload())
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn map_entry_payloads(&self) -> Option<Vec<CompilerMapEntryPayload>> {
        if !self.matches_syntax_kind(SyntaxExpressionKind::Map) {
            return None;
        }
        let source = self.source?;
        Some(
            self.syntax
                .as_ref()?
                .as_map()?
                .entries()
                .map(|syntax| CompilerMapEntryPayload {
                    source: Some(source),
                    syntax: Some(syntax),
                })
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn map_entry_value_payloads(
        &self,
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
        Some(
            self.map_entry_payloads()?
                .into_iter()
                .map(|payload| payload.value_expression_payload())
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn record_field_payloads(
        &self,
    ) -> Option<Vec<CompilerRecordFieldPayload>> {
        if !self.matches_syntax_kind(SyntaxExpressionKind::Record) {
            return None;
        }
        let source = self.source?;
        Some(
            self.syntax
                .as_ref()?
                .as_record()?
                .fields()
                .into_iter()
                .map(|syntax| CompilerRecordFieldPayload {
                    source: Some(source),
                    syntax: Some(syntax),
                })
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn record_field_value_payloads(
        &self,
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
        Some(
            self.record_field_payloads()?
                .into_iter()
                .filter_map(|payload| payload.value_expression_payload())
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn interpolated_expression_payloads(
        &self,
    ) -> Option<Vec<CompilerInterpolationPayload>> {
        if !self.matches_syntax_kind(SyntaxExpressionKind::Literal) {
            return None;
        }
        let source = self.source?;
        Some(
            self.syntax
                .as_ref()?
                .as_literal()?
                .interpolation_expressions()
                .map(|syntax| CompilerInterpolationPayload {
                    source: Some(source),
                    syntax: Some(syntax),
                })
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn interpolated_expression_value_payloads(
        &self,
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
        Some(
            self.interpolated_expression_payloads()?
                .into_iter()
                .map(|payload| payload.value_expression_payload())
                .collect(),
        )
    }
}

fn syntax_expression_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}

#[cfg(test)]
impl CompilerArrayElementPayload {
    #[cfg(test)]
    pub(in crate::compiler) fn syntax_expression(&self) -> Option<&SyntaxExpression> {
        self.source?;
        self.syntax.as_ref()
    }

    pub(in crate::compiler) fn value_expression_payload<'ast>(
        &self,
    ) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload::from_syntax(self.source, self.syntax.clone())
    }
}

#[cfg(test)]
impl CompilerInterpolationPayload {
    #[cfg(test)]
    pub(in crate::compiler) fn syntax_expression(&self) -> Option<&SyntaxExpression> {
        self.source?;
        self.syntax.as_ref()
    }

    pub(in crate::compiler) fn value_expression_payload<'ast>(
        &self,
    ) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload::from_syntax(self.source, self.syntax.clone())
    }
}

#[cfg(test)]
impl CompilerMapEntryPayload {
    #[cfg(test)]
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

    #[cfg(test)]
    pub(in crate::compiler) fn has_value_syntax(&self) -> bool {
        self.source.is_some()
            && self
                .syntax
                .as_ref()
                .is_some_and(|entry| entry.value().is_some())
    }

    pub(in crate::compiler) fn value_expression_payload<'ast>(
        &self,
    ) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload::from_syntax(
            self.source,
            self.source
                .and_then(|_| self.syntax.as_ref().and_then(SyntaxMapEntry::value)),
        )
    }
}

#[cfg(test)]
impl CompilerRecordFieldPayload {
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

    pub(in crate::compiler) fn value_expression_payload<'ast>(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        Some(CompilerExpressionPayload::from_syntax(
            self.source,
            self.source.and_then(|_| {
                self.syntax
                    .as_ref()
                    .and_then(SyntaxRecordExprField::expression)
            }),
        ))
    }
}

#[cfg(test)]
impl CompilerMatchArmPayload {
    #[cfg(test)]
    pub(in crate::compiler) fn missing_child_payload_context(syntax: SyntaxMatchArm) -> Self {
        Self {
            source: None,
            syntax: Some(syntax),
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn missing_syntax() -> Self {
        Self {
            source: None,
            syntax: None,
        }
    }

    pub(in crate::compiler) fn pattern_payload(&self) -> CompilerPatternPayload {
        CompilerPatternPayload::from_syntax(
            self.source,
            self.source
                .and_then(|_| self.syntax.as_ref().and_then(SyntaxMatchArm::pattern)),
        )
    }

    pub(in crate::compiler) fn has_syntax(&self) -> bool {
        self.source.is_some() && self.syntax.is_some()
    }

    pub(in crate::compiler) fn syntax_body_expression_kind(&self) -> Option<SyntaxExpressionKind> {
        self.source?;
        self.syntax
            .as_ref()?
            .body_as_expression()
            .map(|body| body.expression_kind())
    }

    pub(in crate::compiler) fn guard_payload<'ast>(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        self.source?;
        Some(CompilerExpressionPayload::from_syntax(
            self.source,
            self.syntax.as_ref()?.guard(),
        ))
    }

    pub(in crate::compiler) fn body_block_payload(&self) -> Option<CompilerBodyPayload<'_>> {
        Some(CompilerBodyPayload::nested_syntax(
            self.source?,
            self.syntax.as_ref()?.body_block()?,
        ))
    }

    pub(in crate::compiler) fn body_expression_payload<'ast>(
        &self,
    ) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload::from_syntax(
            self.source,
            self.source.and_then(|_| {
                self.syntax
                    .as_ref()
                    .and_then(SyntaxMatchArm::body_as_expression)
            }),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn syntax_arm(&self) -> Option<&SyntaxMatchArm> {
        self.source?;
        self.syntax.as_ref()
    }
}

#[cfg(test)]
impl CompilerPatternPayload {
    pub(in crate::compiler) fn from_syntax(
        source: Option<SourceId>,
        syntax: Option<SyntaxPattern>,
    ) -> Self {
        Self { source, syntax }
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
    ) -> Option<Vec<CompilerRecordPatternFieldPayload>> {
        let syntax_fields = self
            .syntax
            .as_ref()?
            .record_pattern()?
            .fields()
            .collect::<Vec<_>>();
        Some(
            syntax_fields
                .into_iter()
                .map(|syntax| CompilerRecordPatternFieldPayload {
                    source: self.source,
                    syntax: Some(syntax),
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn tuple_pattern_payloads(
        &self,
    ) -> Option<Vec<CompilerPatternPayload>> {
        let syntax_fields = self
            .syntax
            .as_ref()?
            .tuple_pattern()?
            .patterns()
            .collect::<Vec<_>>();
        Some(
            syntax_fields
                .into_iter()
                .map(|syntax| CompilerPatternPayload::from_syntax(self.source, Some(syntax)))
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn missing_child_payload_context(
        syntax: vela_syntax::ast::SyntaxPattern,
    ) -> Self {
        Self::from_syntax(None, Some(syntax))
    }

    #[cfg(test)]
    pub(in crate::compiler) fn missing_syntax(source: SourceId) -> Self {
        Self::from_syntax(Some(source), None)
    }

    #[cfg(test)]
    pub(in crate::compiler) fn syntax_pattern(&self) -> Option<&vela_syntax::ast::SyntaxPattern> {
        self.source?;
        self.syntax.as_ref()
    }
}

#[cfg(test)]
impl CompilerRecordPatternFieldPayload {
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

    pub(in crate::compiler) fn pattern_payload(&self) -> Option<CompilerPatternPayload> {
        self.source?;
        Some(CompilerPatternPayload::from_syntax(
            self.source,
            self.syntax
                .as_ref()
                .and_then(SyntaxRecordPatternField::pattern),
        ))
    }
}
