use vela_common::{SourceId, Span};
use vela_syntax::ast::{
    AstNode, BinaryOp, ExprKind, InterpolatedStringPart, Literal, Pattern, SyntaxExpression,
    SyntaxLambdaBody, SyntaxMapEntry, SyntaxMatchArm, SyntaxRecordExprField,
    SyntaxRecordPatternField,
};

use super::{
    CompilerArgumentPayload, CompilerBodyPayload, CompilerExpressionPayload, CompilerIfPayload,
    CompilerMapEntryPayload, CompilerMatchArmPayload, CompilerPatternPayload,
    CompilerRecordFieldPayload, CompilerRecordPatternFieldPayload, if_payload_for_fallback,
    match_arm_payloads_for_fallback, match_scrutinee_payload_for_fallback,
    syntax_argument_for_fallback, syntax_expression_for_fallback, syntax_map_entry_for_fallback,
    syntax_pattern_for_fallback, syntax_record_field_for_fallback,
    syntax_record_pattern_field_for_fallback,
};

impl<'ast> CompilerExpressionPayload<'ast> {
    pub(in crate::compiler) fn syntax(
        source: SourceId,
        syntax: SyntaxExpression,
        fallback: &'ast vela_syntax::ast::Expr,
    ) -> Self {
        Self {
            source: Some(source),
            syntax: Some(syntax),
            fallback,
        }
    }

    pub(in crate::compiler) fn block_body_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let ExprKind::Block(block) = &self.fallback.kind else {
            return None;
        };
        Some(CompilerBodyPayload::syntax(
            self.source?,
            self.syntax.as_ref()?.as_block()?,
            block,
        ))
    }

    pub(in crate::compiler) fn if_payload(&self) -> Option<CompilerIfPayload<'ast>> {
        let ExprKind::If(if_expr) = &self.fallback.kind else {
            return None;
        };
        if_payload_for_fallback(self.source, self.syntax.as_ref()?.as_if()?, if_expr)
    }

    pub(in crate::compiler) fn match_arm_payloads(
        &self,
    ) -> Option<Vec<CompilerMatchArmPayload<'ast>>> {
        let ExprKind::Match(match_expr) = &self.fallback.kind else {
            return None;
        };
        Some(match_arm_payloads_for_fallback(
            self.source,
            self.syntax.as_ref()?.as_match()?,
            match_expr,
        ))
    }

    pub(in crate::compiler) fn match_scrutinee_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let ExprKind::Match(match_expr) = &self.fallback.kind else {
            return None;
        };
        Some(match_scrutinee_payload_for_fallback(
            self.source,
            self.syntax.as_ref()?.as_match()?,
            match_expr,
        ))
    }

    pub(in crate::compiler) fn syntax_span(&self) -> Option<Span> {
        Some(syntax_expression_span(self.source?, self.syntax.as_ref()?))
    }

    pub(in crate::compiler) fn syntax_path_segments(&self) -> Option<Vec<String>> {
        let segments = self.syntax.as_ref()?.as_path()?.path_segments();
        (!segments.is_empty()).then_some(segments)
    }

    pub(in crate::compiler) fn syntax_is_self(&self) -> bool {
        self.syntax
            .as_ref()
            .and_then(SyntaxExpression::as_path)
            .is_some_and(|path| path.is_self())
    }

    pub(in crate::compiler) fn syntax_record_path_segments(&self) -> Option<Vec<String>> {
        let segments = self.syntax.as_ref()?.as_record()?.path_segments();
        (!segments.is_empty()).then_some(segments)
    }

    pub(in crate::compiler) fn syntax_call_callee_path_segments(&self) -> Option<Vec<String>> {
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

    pub(in crate::compiler) fn literal(&self) -> Option<Literal> {
        let ExprKind::Literal(_) = &self.fallback.kind else {
            return None;
        };
        self.syntax.as_ref()?.as_literal()?.literal()
    }

    pub(in crate::compiler) fn paren_inner_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.syntax.as_ref()?.as_paren()?.expression(),
            fallback: self.fallback,
        })
    }

    pub(in crate::compiler) fn unary_operand_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let ExprKind::Unary { expr, .. } = &self.fallback.kind else {
            return None;
        };
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.syntax.as_ref()?.as_unary()?.expression(),
            fallback: expr,
        })
    }

    pub(in crate::compiler) fn try_operand_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let ExprKind::Try(expr) = &self.fallback.kind else {
            return None;
        };
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.syntax.as_ref()?.as_try()?.expression(),
            fallback: expr,
        })
    }

    pub(in crate::compiler) fn binary_operand_payloads(
        &self,
    ) -> Option<(
        CompilerExpressionPayload<'ast>,
        CompilerExpressionPayload<'ast>,
    )> {
        let ExprKind::Binary { left, right, .. } = &self.fallback.kind else {
            return None;
        };
        let syntax = self.syntax.as_ref()?.as_binary()?;
        Some((
            CompilerExpressionPayload {
                source: self.source,
                syntax: syntax.lhs(),
                fallback: left,
            },
            CompilerExpressionPayload {
                source: self.source,
                syntax: syntax.rhs(),
                fallback: right,
            },
        ))
    }

    pub(in crate::compiler) fn logical_chain_operand_payloads(
        &self,
        op: BinaryOp,
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
        fn collect_fallback<'ast>(
            fallback: &'ast vela_syntax::ast::Expr,
            op: BinaryOp,
            operands: &mut Vec<&'ast vela_syntax::ast::Expr>,
        ) {
            if let ExprKind::Binary {
                op: expr_op,
                left,
                right,
            } = &fallback.kind
                && *expr_op == op
            {
                collect_fallback(left, op, operands);
                collect_fallback(right, op, operands);
            } else {
                operands.push(fallback);
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
                collect_syntax(binary.lhs()?, op, operands)?;
                collect_syntax(binary.rhs()?, op, operands)?;
                return Some(());
            }

            operands.push(syntax);
            Some(())
        }

        let ExprKind::Binary { op: expr_op, .. } = &self.fallback.kind else {
            return None;
        };
        if *expr_op != op {
            return None;
        }

        let mut fallback_operands = Vec::new();
        collect_fallback(self.fallback, op, &mut fallback_operands);

        let syntax_operands = if let Some(syntax) = self.syntax.clone() {
            let mut syntax_operands = Vec::new();
            collect_syntax(syntax, op, &mut syntax_operands)?;
            if syntax_operands.len() != fallback_operands.len() {
                return None;
            }
            syntax_operands.into_iter().map(Some).collect()
        } else {
            vec![None; fallback_operands.len()]
        };

        Some(
            fallback_operands
                .into_iter()
                .zip(syntax_operands)
                .map(|(fallback, syntax)| CompilerExpressionPayload {
                    source: self.source,
                    syntax,
                    fallback,
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn call_argument_payloads(
        &self,
    ) -> Option<Vec<CompilerArgumentPayload<'ast>>> {
        let ExprKind::Call { args, .. } = &self.fallback.kind else {
            return None;
        };
        let syntax_args = self.syntax.as_ref()?.as_call()?.arguments();
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
        let ExprKind::Call { callee, .. } = &self.fallback.kind else {
            return None;
        };
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.syntax.as_ref()?.as_call()?.callee(),
            fallback: callee,
        })
    }

    pub(in crate::compiler) fn field_base_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let ExprKind::Field { base, .. } = &self.fallback.kind else {
            return None;
        };
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.syntax.as_ref()?.as_field()?.receiver(),
            fallback: base,
        })
    }

    pub(in crate::compiler) fn field_name(&self) -> Option<String> {
        let ExprKind::Field { .. } = &self.fallback.kind else {
            return None;
        };
        self.syntax.as_ref()?.as_field()?.name_text()
    }

    pub(in crate::compiler) fn index_operand_payloads(
        &self,
    ) -> Option<(
        CompilerExpressionPayload<'ast>,
        CompilerExpressionPayload<'ast>,
    )> {
        let ExprKind::Index { base, index } = &self.fallback.kind else {
            return None;
        };
        let syntax = self.syntax.as_ref()?.as_index()?;
        Some((
            CompilerExpressionPayload {
                source: self.source,
                syntax: syntax.receiver(),
                fallback: base,
            },
            CompilerExpressionPayload {
                source: self.source,
                syntax: syntax.index(),
                fallback: index,
            },
        ))
    }

    pub(in crate::compiler) fn lambda_body_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        let ExprKind::Lambda { body, .. } = &self.fallback.kind else {
            return None;
        };
        let syntax = match self.syntax.as_ref()?.as_lambda()?.body()? {
            SyntaxLambdaBody::Expression(expression) => Some(expression),
            SyntaxLambdaBody::Block(block) => SyntaxExpression::cast(block.syntax().clone()),
        };
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax,
            fallback: body,
        })
    }

    pub(in crate::compiler) fn array_element_payloads(
        &self,
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
        let ExprKind::Array(items) = &self.fallback.kind else {
            return None;
        };
        let syntax_items = self
            .syntax
            .as_ref()?
            .as_array()?
            .expressions()
            .collect::<Vec<_>>();
        Some(
            items
                .iter()
                .map(|fallback| CompilerExpressionPayload {
                    source: self.source,
                    syntax: syntax_expression_for_fallback(&syntax_items, fallback),
                    fallback,
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn map_entry_payloads(
        &self,
    ) -> Option<Vec<CompilerMapEntryPayload<'ast>>> {
        let ExprKind::Map(entries) = &self.fallback.kind else {
            return None;
        };
        let syntax_entries = self
            .syntax
            .as_ref()?
            .as_map()?
            .entries()
            .collect::<Vec<_>>();
        Some(
            entries
                .iter()
                .map(|fallback| CompilerMapEntryPayload {
                    source: self.source,
                    syntax: syntax_map_entry_for_fallback(&syntax_entries, fallback),
                    fallback,
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn record_field_payloads(
        &self,
    ) -> Option<Vec<CompilerRecordFieldPayload<'ast>>> {
        let ExprKind::Record { fields, .. } = &self.fallback.kind else {
            return None;
        };
        let syntax_fields = self.syntax.as_ref()?.as_record()?.fields();
        Some(
            fields
                .iter()
                .map(|fallback| CompilerRecordFieldPayload {
                    source: self.source,
                    syntax: syntax_record_field_for_fallback(&syntax_fields, fallback),
                    fallback,
                })
                .collect(),
        )
    }

    pub(in crate::compiler) fn interpolated_expression_payloads(
        &self,
    ) -> Option<Vec<CompilerExpressionPayload<'ast>>> {
        let ExprKind::InterpolatedString(parts) = &self.fallback.kind else {
            return None;
        };
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
                .map(|fallback| CompilerExpressionPayload {
                    source: self.source,
                    syntax: syntax_expression_for_fallback(&syntax_expressions, fallback),
                    fallback,
                })
                .collect(),
        )
    }
}

fn syntax_expression_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}

impl<'ast> CompilerMapEntryPayload<'ast> {
    pub(in crate::compiler) fn syntax_key_name(&self) -> Option<String> {
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

    pub(in crate::compiler) fn value_expression_payload(&self) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload {
            source: self.source,
            syntax: self.syntax.as_ref().and_then(SyntaxMapEntry::value),
            fallback: &self.fallback.value,
        }
    }
}

impl<'ast> CompilerRecordFieldPayload<'ast> {
    pub(in crate::compiler) fn syntax_label_name(&self) -> Option<String> {
        self.syntax
            .as_ref()
            .and_then(SyntaxRecordExprField::label_text)
    }

    pub(in crate::compiler) fn value_expression_payload(
        &self,
    ) -> Option<CompilerExpressionPayload<'ast>> {
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self
                .syntax
                .as_ref()
                .and_then(SyntaxRecordExprField::expression),
            fallback: self.fallback.value.as_ref()?,
        })
    }
}

impl<'ast> CompilerMatchArmPayload<'ast> {
    pub(in crate::compiler) fn pattern_payload(&self) -> CompilerPatternPayload<'ast> {
        CompilerPatternPayload {
            syntax: self.syntax.as_ref().and_then(SyntaxMatchArm::pattern),
            fallback: &self.fallback.pattern,
        }
    }

    pub(in crate::compiler) fn guard_payload(&self) -> Option<CompilerExpressionPayload<'ast>> {
        Some(CompilerExpressionPayload {
            source: self.source,
            syntax: self.syntax.as_ref()?.guard(),
            fallback: self.fallback.guard.as_ref()?,
        })
    }

    pub(in crate::compiler) fn body_block_payload(&self) -> Option<CompilerBodyPayload<'ast>> {
        let ExprKind::Block(block) = &self.fallback.body.kind else {
            return None;
        };
        Some(CompilerBodyPayload::syntax(
            self.source?,
            self.syntax.as_ref()?.body_block()?,
            block,
        ))
    }

    pub(in crate::compiler) fn body_expression_payload(&self) -> CompilerExpressionPayload<'ast> {
        CompilerExpressionPayload {
            source: self.source,
            syntax: self
                .syntax
                .as_ref()
                .and_then(SyntaxMatchArm::body_as_expression),
            fallback: &self.fallback.body,
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn syntax_arm(&self) -> Option<&SyntaxMatchArm> {
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerPatternPayload<'ast> {
    pub(in crate::compiler) fn literal(&self) -> Option<vela_syntax::ast::Literal> {
        let Pattern::Literal(_) = self.fallback else {
            return None;
        };
        self.syntax.as_ref()?.literal()
    }

    pub(in crate::compiler) fn path_segments(&self) -> Option<Vec<String>> {
        if !matches!(
            self.fallback,
            Pattern::Path(_) | Pattern::TupleVariant { .. } | Pattern::RecordVariant { .. }
        ) {
            return None;
        }
        let segments = self.syntax.as_ref()?.path_segments();
        (!segments.is_empty()).then_some(segments)
    }

    pub(in crate::compiler) fn binding_name(&self) -> Option<String> {
        let Pattern::Binding(_) = self.fallback else {
            return None;
        };
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
        let Pattern::RecordVariant { fields, .. } = self.fallback else {
            return None;
        };
        Some(
            fields
                .iter()
                .map(|fallback| CompilerRecordPatternFieldPayload {
                    syntax: syntax_record_pattern_field_for_fallback(&syntax_fields, fallback),
                    fallback,
                })
                .collect(),
        )
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
        let Pattern::TupleVariant { fields, .. } = self.fallback else {
            return None;
        };
        Some(
            fields
                .iter()
                .map(|fallback| CompilerPatternPayload {
                    syntax: syntax_pattern_for_fallback(&syntax_fields, fallback),
                    fallback,
                })
                .collect(),
        )
    }

    #[cfg(test)]
    pub(in crate::compiler) fn syntax(
        syntax: vela_syntax::ast::SyntaxPattern,
        fallback: &'ast Pattern,
    ) -> Self {
        Self {
            syntax: Some(syntax),
            fallback,
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn syntax_pattern(&self) -> Option<&vela_syntax::ast::SyntaxPattern> {
        self.syntax.as_ref()
    }
}

impl<'ast> CompilerRecordPatternFieldPayload<'ast> {
    pub(in crate::compiler) fn syntax_label_name(&self) -> Option<String> {
        self.syntax
            .as_ref()
            .and_then(SyntaxRecordPatternField::label_text)
    }

    pub(in crate::compiler) fn pattern_payload(&self) -> Option<CompilerPatternPayload<'ast>> {
        Some(CompilerPatternPayload {
            syntax: self
                .syntax
                .as_ref()
                .and_then(SyntaxRecordPatternField::pattern),
            fallback: self.fallback.pattern.as_ref()?,
        })
    }
}
