use vela_common::{SourceId, Span};
use vela_syntax::ast::{
    AstNode, BinaryOp, Expr, Literal, SyntaxBlock, SyntaxExpression, SyntaxExpressionKind,
    SyntaxMapEntry, UnaryOp,
};

use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::syntax_payloads::ParamDefaultExpression;
use crate::{Register, UnlinkedInstructionKind};

use super::const_eval::{compile_literal_constant, compile_negated_literal_constant};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ParamDefaultValue {
    pub(super) source: SourceId,
    pub(super) expression: SyntaxExpression,
    fallback: Option<Expr>,
}

pub(super) fn param_default_values(
    syntax_defaults: &[Option<ParamDefaultExpression>],
    legacy_defaults: &[Option<&Expr>],
) -> Vec<Option<ParamDefaultValue>> {
    syntax_defaults
        .iter()
        .enumerate()
        .map(|(index, syntax_default)| {
            let syntax_default = syntax_default.clone()?;
            let direct_cst_lowering = param_default_cst_lowering_covers(&syntax_default.expression);
            let fallback = if direct_cst_lowering {
                None
            } else {
                legacy_defaults
                    .get(index)
                    .copied()
                    .flatten()
                    .filter(|fallback| {
                        syntax_range_overlaps_span(
                            syntax_default.expression.syntax().text_range(),
                            fallback.span,
                        )
                    })
                    .cloned()
            };
            if fallback.is_none() && !direct_cst_lowering {
                return None;
            }
            Some(ParamDefaultValue {
                source: syntax_default.source,
                expression: syntax_default.expression,
                fallback,
            })
        })
        .collect()
}

impl Compiler<'_, '_> {
    pub(super) fn compile_param_default_value(
        &mut self,
        default: &ParamDefaultValue,
    ) -> CompileResult<Register> {
        if param_default_cst_lowering_covers(&default.expression) {
            return self.compile_param_default_expression(default.source, &default.expression);
        }
        let Some(fallback) = default.fallback.as_ref() else {
            return Err(param_default_unsupported(
                default.source,
                &default.expression,
            ));
        };
        let payload =
            CompilerExpressionPayload::syntax(default.source, default.expression.clone(), fallback);
        self.compile_expr_with_payload(fallback, Some(&payload))
    }

    fn compile_param_default_expression(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Register> {
        match expression.expression_kind() {
            SyntaxExpressionKind::Literal => {
                let Some(literal) = expression
                    .as_literal()
                    .and_then(|literal| literal.literal())
                else {
                    return Err(param_default_unsupported(source, expression));
                };
                self.compile_param_default_literal(source, expression, &literal)
            }
            SyntaxExpressionKind::Path => {
                let Some(path) = expression.as_path() else {
                    return Err(param_default_unsupported(source, expression));
                };
                self.compile_path_expr(span_for(source, expression), &path.path_segments())
            }
            SyntaxExpressionKind::Paren => {
                let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) else {
                    return Err(param_default_unsupported(source, expression));
                };
                self.compile_param_default_expression(source, &inner)
            }
            SyntaxExpressionKind::Unary => {
                let Some(unary) = expression.as_unary() else {
                    return Err(param_default_unsupported(source, expression));
                };
                let Some(op) = unary.operator() else {
                    return Err(param_default_unsupported(source, expression));
                };
                let Some(operand) = unary.expression() else {
                    return Err(param_default_unsupported(source, expression));
                };
                self.compile_param_default_unary(source, expression, op, &operand)
            }
            SyntaxExpressionKind::Binary => {
                let Some(binary) = expression.as_binary() else {
                    return Err(param_default_unsupported(source, expression));
                };
                let Some(op) = binary.operator() else {
                    return Err(param_default_unsupported(source, expression));
                };
                if matches!(op, BinaryOp::Or | BinaryOp::And) {
                    return self.compile_param_default_logical_chain(source, expression, op);
                }
                let Some(left) = binary.lhs() else {
                    return Err(param_default_unsupported(source, expression));
                };
                let Some(right) = binary.rhs() else {
                    return Err(param_default_unsupported(source, expression));
                };
                self.compile_param_default_binary(source, expression, op, &left, &right)
            }
            SyntaxExpressionKind::Array => {
                let Some(array) = expression.as_array() else {
                    return Err(param_default_unsupported(source, expression));
                };
                let elements = array
                    .expressions()
                    .map(|element| self.compile_param_default_expression(source, &element))
                    .collect::<CompileResult<Vec<_>>>()?;
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::MakeArray { dst, elements });
                Ok(dst)
            }
            SyntaxExpressionKind::Map => {
                let Some(map) = expression.as_map() else {
                    return Err(param_default_unsupported(source, expression));
                };
                let entries = map
                    .entries()
                    .map(|entry| self.compile_param_default_map_entry(source, &entry))
                    .collect::<CompileResult<Vec<_>>>()?;
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::MakeMap { dst, entries });
                Ok(dst)
            }
            SyntaxExpressionKind::Try => {
                let Some(operand) = expression
                    .as_try()
                    .and_then(|try_expr| try_expr.expression())
                else {
                    return Err(param_default_unsupported(source, expression));
                };
                let src = self.compile_param_default_expression(source, &operand)?;
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::TryPropagate { dst, src });
                Ok(dst)
            }
            SyntaxExpressionKind::Block => {
                let Some(block) = expression.as_block() else {
                    return Err(param_default_unsupported(source, expression));
                };
                self.compile_param_default_block(source, &block)
            }
            SyntaxExpressionKind::Assign
            | SyntaxExpressionKind::Field
            | SyntaxExpressionKind::Call
            | SyntaxExpressionKind::Index
            | SyntaxExpressionKind::Record
            | SyntaxExpressionKind::Lambda
            | SyntaxExpressionKind::If
            | SyntaxExpressionKind::Match => Err(param_default_unsupported(source, expression)),
        }
    }

    fn compile_param_default_block(
        &mut self,
        source: SourceId,
        block: &SyntaxBlock,
    ) -> CompileResult<Register> {
        let statements = block.statements().collect::<Vec<_>>();
        match statements.as_slice() {
            [] => self.emit_constant(crate::Constant::Null),
            [statement] => {
                let Some(expr_stmt) = statement.as_expr() else {
                    return Err(param_default_block_unsupported(source, block));
                };
                let Some(expression) = expr_stmt.expression() else {
                    return Err(param_default_block_unsupported(source, block));
                };
                let value = self.compile_param_default_expression(source, &expression)?;
                if expr_stmt.semicolon_token().is_some() {
                    self.emit_constant(crate::Constant::Null)
                } else {
                    Ok(value)
                }
            }
            _ => Err(param_default_block_unsupported(source, block)),
        }
    }

    fn compile_param_default_literal(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        literal: &Literal,
    ) -> CompileResult<Register> {
        let span = span_for(source, expression);
        let constant = compile_literal_constant(literal).map_err(|error| error.with_span(span))?;
        self.emit_constant(constant)
    }

    fn compile_param_default_unary(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        op: UnaryOp,
        operand: &SyntaxExpression,
    ) -> CompileResult<Register> {
        let span = span_for(source, expression);
        if op == UnaryOp::Negate
            && let Some(literal) = operand.as_literal().and_then(|literal| literal.literal())
            && let Some(constant) = compile_negated_literal_constant(&literal)
                .map_err(|error| error.with_span(span_for(source, operand)))?
        {
            return self.emit_constant(constant);
        }
        let src = self.compile_param_default_expression(source, operand)?;
        let dst = self.alloc_register()?;
        let instruction = match op {
            UnaryOp::Not => UnlinkedInstructionKind::Not { dst, src },
            UnaryOp::Negate => UnlinkedInstructionKind::Negate { dst, src },
        };
        self.emit_spanned(instruction, span);
        Ok(dst)
    }

    fn compile_param_default_binary(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        op: BinaryOp,
        left: &SyntaxExpression,
        right: &SyntaxExpression,
    ) -> CompileResult<Register> {
        if matches!(op, BinaryOp::Range | BinaryOp::RangeInclusive) {
            return Err(param_default_unsupported(source, expression));
        }
        let lhs = self.compile_param_default_expression(source, left)?;
        let rhs = self.compile_param_default_expression(source, right)?;
        let dst = self.alloc_register()?;
        let instruction = match op {
            BinaryOp::Add => UnlinkedInstructionKind::Add { dst, lhs, rhs },
            BinaryOp::Sub => UnlinkedInstructionKind::Sub { dst, lhs, rhs },
            BinaryOp::Mul => UnlinkedInstructionKind::Mul { dst, lhs, rhs },
            BinaryOp::Div => UnlinkedInstructionKind::Div { dst, lhs, rhs },
            BinaryOp::Rem => UnlinkedInstructionKind::Rem { dst, lhs, rhs },
            BinaryOp::Equal => UnlinkedInstructionKind::Equal { dst, lhs, rhs },
            BinaryOp::NotEqual => UnlinkedInstructionKind::NotEqual { dst, lhs, rhs },
            BinaryOp::IdentityEqual => UnlinkedInstructionKind::IdentityEqual { dst, lhs, rhs },
            BinaryOp::IdentityNotEqual => {
                UnlinkedInstructionKind::IdentityNotEqual { dst, lhs, rhs }
            }
            BinaryOp::Less => UnlinkedInstructionKind::Less { dst, lhs, rhs },
            BinaryOp::LessEqual => UnlinkedInstructionKind::LessEqual { dst, lhs, rhs },
            BinaryOp::Greater => UnlinkedInstructionKind::Greater { dst, lhs, rhs },
            BinaryOp::GreaterEqual => UnlinkedInstructionKind::GreaterEqual { dst, lhs, rhs },
            BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::Or | BinaryOp::And => {
                unreachable!("unsupported binary operators were rejected before compiling operands")
            }
        };
        self.emit_spanned(instruction, span_for(source, expression));
        Ok(dst)
    }

    fn compile_param_default_logical_chain(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        op: BinaryOp,
    ) -> CompileResult<Register> {
        let Some(operands) = logical_chain_syntax_operands(expression, op) else {
            return Err(param_default_unsupported(source, expression));
        };
        match op {
            BinaryOp::And => self.compile_param_default_logical_and_chain(source, &operands),
            BinaryOp::Or => self.compile_param_default_logical_or_chain(source, &operands),
            _ => unreachable!("logical chain only supports && and ||"),
        }
    }

    fn compile_param_default_logical_and_chain(
        &mut self,
        source: SourceId,
        operands: &[SyntaxExpression],
    ) -> CompileResult<Register> {
        let dst = self.alloc_register()?;
        let Some((last, prefix)) = operands.split_last() else {
            self.emit_bool_constant_to(dst, true);
            return Ok(dst);
        };

        let mut false_branches = Vec::with_capacity(prefix.len());
        for operand in prefix {
            let value = self.compile_param_default_expression(source, operand)?;
            false_branches.push(self.emit_jump_if_false(value));
        }

        let last = self.compile_param_default_expression(source, last)?;
        self.emit_truthy_to_bool(dst, last)?;
        let end = self.emit_jump();

        for false_branch in false_branches {
            self.patch_jump(false_branch, self.current_offset())?;
        }
        self.emit_bool_constant_to(dst, false);
        self.patch_jump(end, self.current_offset())?;

        Ok(dst)
    }

    fn compile_param_default_logical_or_chain(
        &mut self,
        source: SourceId,
        operands: &[SyntaxExpression],
    ) -> CompileResult<Register> {
        let dst = self.alloc_register()?;
        let Some((last, prefix)) = operands.split_last() else {
            self.emit_bool_constant_to(dst, false);
            return Ok(dst);
        };

        let mut end_jumps = Vec::with_capacity(prefix.len());
        for operand in prefix {
            let value = self.compile_param_default_expression(source, operand)?;
            let next_operand = self.emit_jump_if_false(value);
            self.emit_bool_constant_to(dst, true);
            end_jumps.push(self.emit_jump());
            self.patch_jump(next_operand, self.current_offset())?;
        }

        let last = self.compile_param_default_expression(source, last)?;
        self.emit_truthy_to_bool(dst, last)?;
        for end in end_jumps {
            self.patch_jump(end, self.current_offset())?;
        }

        Ok(dst)
    }

    fn compile_param_default_map_entry(
        &mut self,
        source: SourceId,
        entry: &SyntaxMapEntry,
    ) -> CompileResult<(String, Register)> {
        let Some(key) = entry.key() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "parameter default map key",
            ))
            .with_span(span_for_range(source, entry.syntax().text_range())));
        };
        let Some(value) = entry.value() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "parameter default map value",
            ))
            .with_span(span_for_range(source, entry.syntax().text_range())));
        };
        let key = syntax_map_key_name(source, &key)?;
        let value = self.compile_param_default_expression(source, &value)?;
        Ok((key, value))
    }
}

fn param_default_cst_lowering_covers(expression: &SyntaxExpression) -> bool {
    match expression.expression_kind() {
        SyntaxExpressionKind::Literal | SyntaxExpressionKind::Path => true,
        SyntaxExpressionKind::Paren => expression
            .as_paren()
            .and_then(|paren| paren.expression())
            .is_some_and(|inner| param_default_cst_lowering_covers(&inner)),
        SyntaxExpressionKind::Unary => {
            let Some(unary) = expression.as_unary() else {
                return false;
            };
            unary.operator().is_some()
                && unary
                    .expression()
                    .is_some_and(|operand| param_default_cst_lowering_covers(&operand))
        }
        SyntaxExpressionKind::Binary => {
            let Some(binary) = expression.as_binary() else {
                return false;
            };
            let Some(op) = binary.operator() else {
                return false;
            };
            if matches!(op, BinaryOp::Range | BinaryOp::RangeInclusive) {
                return false;
            }
            if matches!(op, BinaryOp::Or | BinaryOp::And) {
                return logical_chain_syntax_operands(expression, op).is_some_and(|operands| {
                    operands.iter().all(param_default_cst_lowering_covers)
                });
            }
            binary
                .lhs()
                .is_some_and(|left| param_default_cst_lowering_covers(&left))
                && binary
                    .rhs()
                    .is_some_and(|right| param_default_cst_lowering_covers(&right))
        }
        SyntaxExpressionKind::Array => expression.as_array().is_some_and(|array| {
            array
                .expressions()
                .all(|element| param_default_cst_lowering_covers(&element))
        }),
        SyntaxExpressionKind::Map => expression.as_map().is_some_and(|map| {
            map.entries().all(|entry| {
                entry
                    .key()
                    .is_some_and(|key| syntax_map_key_supported(&key))
                    && entry
                        .value()
                        .is_some_and(|value| param_default_cst_lowering_covers(&value))
            })
        }),
        SyntaxExpressionKind::Try => expression
            .as_try()
            .and_then(|try_expr| try_expr.expression())
            .is_some_and(|operand| param_default_cst_lowering_covers(&operand)),
        SyntaxExpressionKind::Block => expression
            .as_block()
            .is_some_and(|block| param_default_block_cst_lowering_covers(&block)),
        SyntaxExpressionKind::Assign
        | SyntaxExpressionKind::Field
        | SyntaxExpressionKind::Call
        | SyntaxExpressionKind::Index
        | SyntaxExpressionKind::Record
        | SyntaxExpressionKind::Lambda
        | SyntaxExpressionKind::If
        | SyntaxExpressionKind::Match => false,
    }
}

fn param_default_block_cst_lowering_covers(block: &SyntaxBlock) -> bool {
    let statements = block.statements().collect::<Vec<_>>();
    match statements.as_slice() {
        [] => true,
        [statement] => statement
            .as_expr()
            .and_then(|statement| statement.expression())
            .is_some_and(|expression| param_default_cst_lowering_covers(&expression)),
        _ => false,
    }
}

fn logical_chain_syntax_operands(
    expression: &SyntaxExpression,
    op: BinaryOp,
) -> Option<Vec<SyntaxExpression>> {
    fn collect(
        expression: SyntaxExpression,
        op: BinaryOp,
        operands: &mut Vec<SyntaxExpression>,
    ) -> Option<()> {
        if let Some(binary) = expression.as_binary()
            && binary.operator() == Some(op)
        {
            collect(binary.lhs()?, op, operands)?;
            collect(binary.rhs()?, op, operands)?;
            return Some(());
        }

        operands.push(expression);
        Some(())
    }

    let mut operands = Vec::new();
    collect(expression.clone(), op, &mut operands)?;
    Some(operands)
}

fn syntax_map_key_supported(key: &SyntaxExpression) -> bool {
    match key.expression_kind() {
        SyntaxExpressionKind::Literal => key
            .as_literal()
            .and_then(|literal| literal.literal())
            .is_some_and(|literal| {
                matches!(
                    literal,
                    Literal::String(_) | Literal::Char(_) | Literal::Integer(_) | Literal::Float(_)
                )
            }),
        SyntaxExpressionKind::Path => key
            .as_path()
            .is_some_and(|path| !path.path_segments().is_empty()),
        _ => false,
    }
}

fn syntax_map_key_name(source: SourceId, key: &SyntaxExpression) -> CompileResult<String> {
    match key.expression_kind() {
        SyntaxExpressionKind::Literal => {
            let Some(literal) = key.as_literal().and_then(|literal| literal.literal()) else {
                return Err(param_default_unsupported(source, key));
            };
            match literal {
                Literal::String(value) => Ok(value),
                Literal::Char(value) => Ok(value.to_string()),
                Literal::Integer(value) => Ok(value.source_text_with_suffix()),
                Literal::Float(value) => Ok(value.source_text_with_suffix()),
                _ => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "parameter default map key",
                ))
                .with_span(span_for(source, key))),
            }
        }
        SyntaxExpressionKind::Path => key
            .as_path()
            .map(|path| path.path_segments().join("::"))
            .filter(|path| !path.is_empty())
            .ok_or_else(|| param_default_unsupported(source, key)),
        _ => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "parameter default map key",
        ))
        .with_span(span_for(source, key))),
    }
}

fn param_default_unsupported(source: SourceId, expression: &SyntaxExpression) -> CompileError {
    CompileError::new(CompileErrorKind::UnsupportedSyntax(
        "parameter default expression",
    ))
    .with_span(span_for(source, expression))
}

fn param_default_block_unsupported(source: SourceId, block: &SyntaxBlock) -> CompileError {
    CompileError::new(CompileErrorKind::UnsupportedSyntax(
        "parameter default block expression",
    ))
    .with_span(span_for_range(source, block.syntax().text_range()))
}

fn span_for(source: SourceId, expression: &SyntaxExpression) -> Span {
    span_for_range(source, expression.syntax().text_range())
}

fn span_for_range(source: SourceId, range: vela_syntax::TextRange) -> Span {
    Span::new(source, range.start().into(), range.end().into())
}

fn syntax_range_overlaps_span(range: vela_syntax::TextRange, span: Span) -> bool {
    let start = u32::from(range.start());
    let end = u32::from(range.end());
    start < span.end && span.start < end
}

#[cfg(test)]
mod tests {
    use vela_common::{SourceId, Span};
    use vela_syntax::ast::{AstNode, Expr, ExprKind};
    use vela_syntax::parse::parse_source_with_id as parse_syntax_source;

    use crate::compiler::syntax_payloads::ParamDefaultExpression;

    use super::{param_default_cst_lowering_covers, param_default_values};

    #[test]
    fn param_default_values_keep_cst_expression_payloads() {
        let source = SourceId::new(1);
        let text = r#"
fn cst(first = 1) {
    return first;
}
"#;
        let syntax = parse_syntax_source(source, text);
        let cst_function = syntax
            .tree()
            .functions()
            .find(|function| function.name_text().as_deref() == Some("cst"))
            .expect("CST function");
        let syntax_expression = cst_function
            .param_list()
            .and_then(|params| params.params().next())
            .and_then(|param| param.default_value())
            .expect("CST default expression");
        let syntax_defaults = vec![Some(ParamDefaultExpression {
            source,
            expression: syntax_expression,
        })];
        let fallback_expr = Expr {
            kind: ExprKind::Error,
            span: Span::new(source, 16, 17),
        };

        let defaults = param_default_values(&syntax_defaults, &[Some(&fallback_expr)]);

        assert_eq!(defaults.len(), 1);
        assert_eq!(
            defaults[0]
                .as_ref()
                .expect("default")
                .expression
                .syntax()
                .text()
                .to_string(),
            "1"
        );
        assert!(
            defaults[0].as_ref().expect("default").fallback.is_none(),
            "directly lowered CST defaults should not retain a legacy expression fallback"
        );
    }

    #[test]
    fn mismatched_param_defaults_do_not_pair_by_index() {
        let source = SourceId::new(1);
        let text = r#"
fn cst(first = expensive()) {
    return first;
}
"#;
        let parsed = parse_syntax_source(source, text);
        let cst_function = parsed
            .tree()
            .functions()
            .find(|function| function.name_text().as_deref() == Some("cst"))
            .expect("CST function");
        let syntax_expression = cst_function
            .param_list()
            .and_then(|params| params.params().next())
            .and_then(|param| param.default_value())
            .expect("default expression");
        let syntax_defaults = vec![Some(ParamDefaultExpression {
            source,
            expression: syntax_expression,
        })];
        let fallback_expr = Expr {
            kind: ExprKind::Error,
            span: Span::new(source, 1000, 1001),
        };

        let defaults = param_default_values(&syntax_defaults, &[Some(&fallback_expr)]);

        assert_eq!(defaults.len(), 1);
        assert!(
            defaults[0].is_none(),
            "unsupported defaults must not receive mismatched legacy fallbacks by index"
        );
    }

    #[test]
    fn directly_lowered_param_defaults_do_not_require_legacy_fallbacks() {
        let source = SourceId::new(1);
        let syntax_defaults = vec![Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = 1 + 2) { return value; }"),
        })];

        let defaults = param_default_values(&syntax_defaults, &[]);

        let default = defaults[0].as_ref().expect("direct CST default");
        assert_eq!(default.expression.syntax().text().to_string(), "1 + 2");
        assert!(
            default.fallback.is_none(),
            "directly lowered CST defaults should not depend on a legacy expression"
        );
    }

    #[test]
    fn param_default_cst_lowering_covers_logical_chains() {
        assert!(
            param_default_cst_lowering_covers(&first_param_default(
                "fn cst(value = true || false || (1 < 2)) { return value; }"
            )),
            "logical defaults with supported operands should lower from CST"
        );
        assert!(
            param_default_cst_lowering_covers(&first_param_default(
                "fn cst(value = false && true && (2 > 1)) { return value; }"
            )),
            "logical defaults with parenthesized supported operands should lower from CST"
        );
        assert!(
            !param_default_cst_lowering_covers(&first_param_default(
                "fn cst(value = true || expensive()) { return value; }"
            )),
            "logical defaults keep the fallback when an operand is not CST-lowered yet"
        );
    }

    #[test]
    fn param_default_cst_lowering_covers_try_expressions() {
        let source = SourceId::new(1);
        let syntax_defaults = vec![Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = maybe?) { return value; }"),
        })];

        let defaults = param_default_values(&syntax_defaults, &[]);

        let default = defaults[0].as_ref().expect("direct CST default");
        assert_eq!(default.expression.syntax().text().to_string(), "maybe?");
        assert!(
            default.fallback.is_none(),
            "try defaults should be directly lowerable from CST"
        );
    }

    #[test]
    fn param_default_cst_lowering_covers_simple_block_expressions() {
        let source = SourceId::new(1);
        let syntax_defaults = vec![
            Some(ParamDefaultExpression {
                source,
                expression: first_param_default("fn cst(value = {}) { return value; }"),
            }),
            Some(ParamDefaultExpression {
                source,
                expression: first_param_default("fn cst(value = { 1 + 2 }) { return value; }"),
            }),
            Some(ParamDefaultExpression {
                source,
                expression: first_param_default("fn cst(value = { maybe?; }) { return value; }"),
            }),
        ];

        let defaults = param_default_values(&syntax_defaults, &[]);

        assert_eq!(defaults.len(), 3);
        for default in defaults {
            assert!(
                default.expect("direct CST default").fallback.is_none(),
                "simple block defaults should be directly lowerable from CST"
            );
        }
    }

    #[test]
    fn param_default_cst_lowering_keeps_complex_block_fallbacks() {
        let source = SourceId::new(1);
        let syntax_defaults = vec![Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = { let x = 1; x }) { return value; }"),
        })];

        let defaults = param_default_values(&syntax_defaults, &[]);

        assert!(
            defaults[0].is_none(),
            "multi-statement block defaults still require the temporary legacy fallback"
        );
    }

    fn first_param_default(text: &str) -> vela_syntax::ast::SyntaxExpression {
        parse_syntax_source(SourceId::new(1), text)
            .tree()
            .functions()
            .next()
            .expect("function")
            .param_list()
            .expect("parameter list")
            .params()
            .next()
            .expect("parameter")
            .default_value()
            .expect("default expression")
    }
}
