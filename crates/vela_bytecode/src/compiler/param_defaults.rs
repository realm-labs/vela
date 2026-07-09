mod calls;
mod fields;
mod matches;
mod records;

use vela_common::{PrimitiveTag, ScalarValue, SourceId, Span};
use vela_hir::binding::LocalBindingKind;
use vela_hir::body::HirBody;
use vela_hir::module_graph::ModuleGraph;
use vela_syntax::ast::{
    AstNode, BinaryOp, FloatSuffix, IntegerSuffix, Literal, SyntaxBlock, SyntaxElseBranch,
    SyntaxExpression, SyntaxExpressionKind, SyntaxIfExpr, SyntaxLetStmt, SyntaxLiteral,
    SyntaxMapEntry, SyntaxParam, SyntaxParamList, UnaryOp,
};
use vela_syntax::token::{InterpolatedStringTokenPart, TokenKind};

use crate::{FormatStringPart, GuardLocation, Register, UnlinkedInstructionKind};

use super::const_eval::{
    compile_literal_constant, compile_literal_constant_for_type, compile_negated_literal_constant,
};
use super::script_types::type_hint_script_type;
use super::value_types::{
    ExpectedTypeOutcome, RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type,
    type_hint_value_type,
};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler, frame_slot_kind};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ParamDefaultValue {
    pub(super) source: SourceId,
    pub(super) expression: SyntaxExpression,
}

pub(super) fn param_default_values(
    source: SourceId,
    params: Option<SyntaxParamList>,
    graph: &ModuleGraph,
    hir_body: &HirBody,
) -> Vec<Option<ParamDefaultValue>> {
    let syntax_params = params
        .map(|params| params.params().collect::<Vec<_>>())
        .unwrap_or_default();
    hir_body
        .params
        .iter()
        .map(|param| {
            let default_body = graph.body(param.default_body?)?;
            let expression =
                syntax_default_expression_for_span(&syntax_params, default_body.origin.span)?;
            Some(ParamDefaultValue { source, expression })
        })
        .collect()
}

fn syntax_default_expression_for_span(
    params: &[SyntaxParam],
    span: Span,
) -> Option<SyntaxExpression> {
    params
        .iter()
        .filter_map(SyntaxParam::default_value)
        .find(|expression| span_for_range(span.source, expression.syntax().text_range()) == span)
}

impl Compiler<'_, '_> {
    pub(super) fn compile_param_default_value(
        &mut self,
        default: &ParamDefaultValue,
    ) -> CompileResult<Register> {
        self.compile_param_default_expression(default.source, &default.expression)
    }

    fn compile_param_default_expression(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Register> {
        match expression.expression_kind() {
            SyntaxExpressionKind::Literal => {
                let Some(literal) = expression.as_literal() else {
                    return Err(param_default_unsupported(source, expression));
                };
                if let Some(literal) = literal.literal() {
                    self.compile_param_default_literal(source, expression, &literal)
                } else {
                    self.compile_param_default_interpolated_string(source, expression, &literal)
                }
            }
            SyntaxExpressionKind::Path => {
                let span = span_for(source, expression);
                let Some(path) = self.hir_value_path_for_span(span) else {
                    return Err(param_default_unsupported(source, expression));
                };
                self.compile_path_expr(span, &path)
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
                self.emit(UnlinkedInstructionKind::TryPropagate {
                    dst,
                    src,
                    expected: None,
                });
                Ok(dst)
            }
            SyntaxExpressionKind::Block => {
                let Some(block) = expression.as_block() else {
                    return Err(param_default_unsupported(source, expression));
                };
                self.compile_param_default_block(source, &block)
            }
            SyntaxExpressionKind::If => {
                let Some(if_expr) = expression.as_if() else {
                    return Err(param_default_unsupported(source, expression));
                };
                self.compile_param_default_if(source, expression, &if_expr)
            }
            SyntaxExpressionKind::Index => {
                let Some(index) = expression.as_index() else {
                    return Err(param_default_unsupported(source, expression));
                };
                let Some(receiver) = index.receiver() else {
                    return Err(param_default_unsupported(source, expression));
                };
                let Some(index) = index.index() else {
                    return Err(param_default_unsupported(source, expression));
                };
                let base = self.compile_param_default_expression(source, &receiver)?;
                let index = self.compile_param_default_expression(source, &index)?;
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::GetIndex { dst, base, index });
                Ok(dst)
            }
            SyntaxExpressionKind::Call => {
                let Some(call) = expression.as_call() else {
                    return Err(param_default_unsupported(source, expression));
                };
                self.compile_param_default_call(source, expression, &call)
            }
            SyntaxExpressionKind::Record => {
                let Some(record) = expression.as_record() else {
                    return Err(param_default_unsupported(source, expression));
                };
                self.compile_param_default_record(source, expression, &record)
            }
            SyntaxExpressionKind::Field => self.compile_param_default_field(source, expression),
            SyntaxExpressionKind::Match => {
                let Some(match_expr) = expression.as_match() else {
                    return Err(param_default_unsupported(source, expression));
                };
                self.compile_param_default_match(source, expression, &match_expr)
            }
            SyntaxExpressionKind::Unit
            | SyntaxExpressionKind::Tuple
            | SyntaxExpressionKind::Assign
            | SyntaxExpressionKind::Lambda => Err(param_default_unsupported(source, expression)),
        }
    }

    fn compile_param_default_if(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        if_expr: &SyntaxIfExpr,
    ) -> CompileResult<Register> {
        let Some(condition) = if_expr.condition() else {
            return Err(param_default_unsupported(source, expression));
        };
        let Some(then_block) = if_expr.then_block() else {
            return Err(param_default_unsupported(source, expression));
        };

        let condition = self.compile_param_default_expression(source, &condition)?;
        let dst = self.alloc_register()?;
        let jump_to_else = self.emit_jump_if_false(condition);

        let then_value = self.compile_param_default_block(source, &then_block)?;
        self.emit(UnlinkedInstructionKind::Move {
            dst,
            src: then_value,
        });
        let jump_to_end = self.emit_jump();

        self.patch_jump(jump_to_else, self.current_offset())?;
        match if_expr.else_branch() {
            Some(SyntaxElseBranch::If(else_if)) => {
                let else_value = self.compile_param_default_if(source, expression, &else_if)?;
                self.emit(UnlinkedInstructionKind::Move {
                    dst,
                    src: else_value,
                });
            }
            Some(SyntaxElseBranch::Block(block)) => {
                let else_value = self.compile_param_default_block(source, &block)?;
                self.emit(UnlinkedInstructionKind::Move {
                    dst,
                    src: else_value,
                });
            }
            None => self.emit_constant_to(dst, crate::Constant::Unit),
        }
        self.patch_jump(jump_to_end, self.current_offset())?;

        Ok(dst)
    }

    fn compile_param_default_block(
        &mut self,
        source: SourceId,
        block: &SyntaxBlock,
    ) -> CompileResult<Register> {
        let statements = block.statements().collect::<Vec<_>>();
        match statements.as_slice() {
            [] => self.emit_constant(crate::Constant::Unit),
            [statements @ .., tail] => {
                for statement in statements {
                    if let Some(let_stmt) = statement.as_let() {
                        self.compile_param_default_let(source, block, &let_stmt)?;
                    } else if let Some(expr_stmt) = statement.as_expr() {
                        let Some(expression) = expr_stmt.expression() else {
                            return Err(param_default_block_unsupported(source, block));
                        };
                        if expr_stmt.semicolon_token().is_none() {
                            return Err(param_default_block_unsupported(source, block));
                        }
                        self.compile_param_default_expression(source, &expression)?;
                    } else {
                        return Err(param_default_block_unsupported(source, block));
                    }
                }

                if let Some(let_stmt) = tail.as_let() {
                    self.compile_param_default_let(source, block, &let_stmt)?;
                    return self.emit_constant(crate::Constant::Unit);
                }
                let Some(expr_stmt) = tail.as_expr() else {
                    return Err(param_default_block_unsupported(source, block));
                };
                let Some(expression) = expr_stmt.expression() else {
                    return Err(param_default_block_unsupported(source, block));
                };
                let value = self.compile_param_default_expression(source, &expression)?;
                if expr_stmt.semicolon_token().is_some() {
                    self.emit_constant(crate::Constant::Unit)
                } else {
                    Ok(value)
                }
            }
        }
    }

    fn compile_param_default_let(
        &mut self,
        source: SourceId,
        block: &SyntaxBlock,
        let_stmt: &SyntaxLetStmt,
    ) -> CompileResult<()> {
        if let_stmt.attributes().next().is_some() {
            return Err(param_default_block_unsupported(source, block));
        }
        let Some(name_token) = let_stmt.name_token() else {
            return Err(param_default_block_unsupported(source, block));
        };
        let Some(pattern) = let_stmt.pattern() else {
            return Err(param_default_block_unsupported(source, block));
        };
        let name = name_token.text().to_owned();
        let span = span_for_range(source, let_stmt.syntax().text_range());
        let hir_patterns = self.hir_pattern_ids_for_syntax_pattern(source, &pattern)?;
        let local_binding = self.let_local_binding_for_patterns(&hir_patterns);
        let hir_type_hint = local_binding.as_ref().and_then(|(_, hint)| hint.as_ref());
        let hinted_value_type = hir_type_hint.and_then(type_hint_value_type);
        let register = if let Some(initializer) = let_stmt.initializer() {
            let outcome = hinted_value_type
                .clone()
                .map(|expected| {
                    check_expected_type(
                        self.param_default_static_type(source, &initializer),
                        expected,
                        span_for(source, &initializer),
                        TypeContractContext::TypedLet { name: name.clone() },
                    )
                })
                .transpose()?;
            let register =
                self.compile_param_default_initializer(source, &initializer, outcome.as_ref())?;
            if matches!(outcome, Some(ExpectedTypeOutcome::RequiresRuntimeGuard(_)))
                && let Some(hint) = hir_type_hint
                && let Some(guard) = super::type_guard_for_hint(
                    hint,
                    GuardLocation::Local,
                    name.clone(),
                    &self.facts,
                )
            {
                self.emit_spanned(
                    UnlinkedInstructionKind::GuardType {
                        src: register,
                        guard,
                    },
                    span_for(source, &initializer),
                );
            }
            register
        } else {
            self.emit_constant(crate::Constant::Unit)?
        };
        let known_type_names = self.facts.known_type_names();
        let script_type =
            hir_type_hint.and_then(|hint| type_hint_script_type(hint, known_type_names.iter()));
        self.locals.insert(name.clone(), register);
        let local = local_binding.as_ref().map(|(local, _)| *local);
        if let Some(local) = local {
            self.hir_locals.insert(local, register);
            self.script_types.set_local(local, &name, script_type);
            self.value_types
                .set_local(local, &name, hinted_value_type.clone());
            self.value_shapes.set_local(local, &name, None);
        } else {
            self.script_types.set_name(&name, script_type);
            self.value_types.set_name(&name, hinted_value_type.clone());
            self.value_shapes.set_name(&name, None);
        }
        self.record_frame_slot(
            name,
            register,
            frame_slot_kind(LocalBindingKind::Let),
            local,
            Some(span),
        );
        Ok(())
    }

    fn compile_param_default_initializer(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        outcome: Option<&ExpectedTypeOutcome>,
    ) -> CompileResult<Register> {
        if let Some(ExpectedTypeOutcome::Contextualized(RuntimeTypeFact::Primitive(tag))) = outcome
            && let Some(literal) = expression
                .as_literal()
                .and_then(|literal| literal.literal())
            && let Some(constant) = compile_literal_constant_for_type(&literal, *tag)
                .map_err(|error| error.with_span(span_for(source, expression)))?
        {
            return self.emit_constant(constant);
        }
        self.compile_param_default_expression(source, expression)
    }

    fn param_default_static_type(
        &self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> StaticExprType {
        match expression.expression_kind() {
            SyntaxExpressionKind::Literal => expression
                .as_literal()
                .and_then(|literal| literal.literal())
                .map_or(StaticExprType::Dynamic, syntax_literal_static_type),
            SyntaxExpressionKind::Path => {
                let span = span_for(source, expression);
                let expression_id = self.expression_at_span(span);
                if let Some(fact) = expression_id
                    .and_then(|expression| self.local_for_expression(expression))
                    .and_then(|local| self.value_types.local(local))
                {
                    StaticExprType::Exact(fact)
                } else if let Some(constant) = self.const_value_at_span(span) {
                    constant_static_type(&constant)
                        .map(StaticExprType::Exact)
                        .unwrap_or(StaticExprType::Dynamic)
                } else {
                    StaticExprType::Dynamic
                }
            }
            SyntaxExpressionKind::Paren => expression
                .as_paren()
                .and_then(|paren| paren.expression())
                .map_or(StaticExprType::Dynamic, |inner| {
                    self.param_default_static_type(source, &inner)
                }),
            _ => StaticExprType::Dynamic,
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

    fn compile_param_default_interpolated_string(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        literal: &SyntaxLiteral,
    ) -> CompileResult<Register> {
        if !param_default_interpolated_string_supported(literal) {
            return Err(param_default_unsupported(source, expression));
        }
        let Some(parts) = interpolated_string_parts(literal) else {
            return Err(param_default_unsupported(source, expression));
        };
        let mut interpolation_expressions = literal.interpolation_expressions();
        let mut compiled = Vec::with_capacity(parts.len());
        for part in parts {
            match part {
                InterpolatedStringTokenPart::Text(value) => {
                    let constant = self.code.push_constant(crate::Constant::String(value));
                    compiled.push(FormatStringPart::Text(constant));
                }
                InterpolatedStringTokenPart::Expr { .. } => {
                    let Some(expression) = interpolation_expressions.next() else {
                        return Err(param_default_unsupported(source, expression));
                    };
                    let value = self.compile_param_default_expression(source, &expression)?;
                    compiled.push(FormatStringPart::Value(value));
                }
            }
        }
        if interpolation_expressions.next().is_some() {
            return Err(param_default_unsupported(source, expression));
        }
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::FormatString {
            dst,
            parts: compiled,
        });
        Ok(dst)
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
            return self.compile_param_default_range(
                source,
                left,
                right,
                op == BinaryOp::RangeInclusive,
            );
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

    fn compile_param_default_range(
        &mut self,
        source: SourceId,
        left: &SyntaxExpression,
        right: &SyntaxExpression,
        inclusive: bool,
    ) -> CompileResult<Register> {
        let start = self.compile_param_default_expression(source, left)?;
        let end = self.compile_param_default_expression(source, right)?;
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::MakeRange {
            dst,
            start,
            end,
            inclusive,
        });
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
        let key = self.map_key_name(source, &key)?;
        let value = self.compile_param_default_expression(source, &value)?;
        Ok((key, value))
    }

    pub(in crate::compiler) fn map_key_name(
        &self,
        source: SourceId,
        key: &SyntaxExpression,
    ) -> CompileResult<String> {
        match key.expression_kind() {
            SyntaxExpressionKind::Literal => syntax_literal_map_key_name(source, key),
            SyntaxExpressionKind::Path => {
                let span = span_for(source, key);
                self.hir_value_path_for_span(span)
                    .map(|path| path.join("::"))
                    .filter(|path| !path.is_empty())
                    .ok_or_else(|| param_default_unsupported(source, key))
            }
            _ => Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "parameter default map key",
            ))
            .with_span(span_for(source, key))),
        }
    }
}

fn param_default_expression_supported(expression: &SyntaxExpression) -> bool {
    match expression.expression_kind() {
        SyntaxExpressionKind::Literal => expression.as_literal().is_some_and(|literal| {
            literal.literal().is_some() || param_default_interpolated_string_supported(&literal)
        }),
        SyntaxExpressionKind::Path => true,
        SyntaxExpressionKind::Paren => expression
            .as_paren()
            .and_then(|paren| paren.expression())
            .is_some_and(|inner| param_default_expression_supported(&inner)),
        SyntaxExpressionKind::Unary => {
            let Some(unary) = expression.as_unary() else {
                return false;
            };
            unary.operator().is_some()
                && unary
                    .expression()
                    .is_some_and(|operand| param_default_expression_supported(&operand))
        }
        SyntaxExpressionKind::Binary => {
            let Some(binary) = expression.as_binary() else {
                return false;
            };
            let Some(op) = binary.operator() else {
                return false;
            };
            if matches!(op, BinaryOp::Or | BinaryOp::And) {
                return logical_chain_syntax_operands(expression, op).is_some_and(|operands| {
                    operands.iter().all(param_default_expression_supported)
                });
            }
            binary
                .lhs()
                .is_some_and(|left| param_default_expression_supported(&left))
                && binary
                    .rhs()
                    .is_some_and(|right| param_default_expression_supported(&right))
        }
        SyntaxExpressionKind::Array => expression.as_array().is_some_and(|array| {
            array
                .expressions()
                .all(|element| param_default_expression_supported(&element))
        }),
        SyntaxExpressionKind::Map => expression.as_map().is_some_and(|map| {
            map.entries().all(|entry| {
                entry
                    .key()
                    .is_some_and(|key| syntax_map_key_supported(&key))
                    && entry
                        .value()
                        .is_some_and(|value| param_default_expression_supported(&value))
            })
        }),
        SyntaxExpressionKind::Try => expression
            .as_try()
            .and_then(|try_expr| try_expr.expression())
            .is_some_and(|operand| param_default_expression_supported(&operand)),
        SyntaxExpressionKind::Block => expression
            .as_block()
            .is_some_and(|block| param_default_block_supported(&block)),
        SyntaxExpressionKind::If => expression
            .as_if()
            .is_some_and(|if_expr| param_default_if_supported(&if_expr)),
        SyntaxExpressionKind::Index => expression.as_index().is_some_and(|index| {
            index
                .receiver()
                .is_some_and(|receiver| param_default_expression_supported(&receiver))
                && index
                    .index()
                    .is_some_and(|index| param_default_expression_supported(&index))
        }),
        SyntaxExpressionKind::Call => calls::param_default_call_supported(expression),
        SyntaxExpressionKind::Record => records::param_default_record_supported(expression),
        SyntaxExpressionKind::Field => fields::param_default_field_supported(expression),
        SyntaxExpressionKind::Match => matches::param_default_match_supported(expression),
        SyntaxExpressionKind::Unit
        | SyntaxExpressionKind::Tuple
        | SyntaxExpressionKind::Assign
        | SyntaxExpressionKind::Lambda => false,
    }
}

fn param_default_interpolated_string_supported(literal: &SyntaxLiteral) -> bool {
    literal.token_kind() == Some(vela_syntax::SyntaxKind::InterpolatedString)
        && literal
            .interpolation_expressions()
            .all(|expression| param_default_expression_supported(&expression))
}

fn interpolated_string_parts(literal: &SyntaxLiteral) -> Option<Vec<InterpolatedStringTokenPart>> {
    let text = literal.token_text()?;
    vela_syntax::lexer::lex(SourceId::new(0), &text)
        .tokens
        .into_iter()
        .find_map(|token| match token.kind {
            TokenKind::InterpolatedString(parts) => Some(parts),
            _ => None,
        })
}

fn param_default_if_supported(if_expr: &SyntaxIfExpr) -> bool {
    if !if_expr
        .condition()
        .is_some_and(|condition| param_default_expression_supported(&condition))
    {
        return false;
    }
    if !if_expr
        .then_block()
        .is_some_and(|block| param_default_block_supported(&block))
    {
        return false;
    }
    match if_expr.else_branch() {
        Some(SyntaxElseBranch::If(else_if)) => param_default_if_supported(&else_if),
        Some(SyntaxElseBranch::Block(block)) => param_default_block_supported(&block),
        None => true,
    }
}

fn param_default_block_supported(block: &SyntaxBlock) -> bool {
    let statements = block.statements().collect::<Vec<_>>();
    match statements.as_slice() {
        [] => true,
        [statements @ .., tail] => {
            for statement in statements {
                if let Some(let_stmt) = statement.as_let() {
                    if !param_default_let_supported(&let_stmt) {
                        return false;
                    }
                } else if let Some(expr_stmt) = statement.as_expr() {
                    if expr_stmt.semicolon_token().is_none()
                        || !expr_stmt.expression().is_some_and(|expression| {
                            param_default_expression_supported(&expression)
                        })
                    {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            if let Some(let_stmt) = tail.as_let() {
                return param_default_let_supported(&let_stmt);
            }
            tail.as_expr()
                .and_then(|statement| statement.expression())
                .is_some_and(|expression| param_default_expression_supported(&expression))
        }
    }
}

fn param_default_let_supported(let_stmt: &SyntaxLetStmt) -> bool {
    if let_stmt.attributes().next().is_some() {
        return false;
    }
    let_stmt.name_token().is_some()
        && let_stmt
            .initializer()
            .is_none_or(|initializer| param_default_expression_supported(&initializer))
}

fn syntax_literal_static_type(literal: Literal) -> StaticExprType {
    match literal {
        Literal::Bool(_) => StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::Bool)),
        Literal::Char(_) => StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::Char)),
        Literal::String(_) => {
            StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::String))
        }
        Literal::Bytes(_) => StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::Bytes)),
        Literal::Integer(value) => match value.suffix {
            None => StaticExprType::UnsuffixedIntegerLiteral,
            Some(IntegerSuffix::I8) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::I8))
            }
            Some(IntegerSuffix::I16) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::I16))
            }
            Some(IntegerSuffix::I32) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::I32))
            }
            Some(IntegerSuffix::I64) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::I64))
            }
            Some(IntegerSuffix::U8) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::U8))
            }
            Some(IntegerSuffix::U16) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::U16))
            }
            Some(IntegerSuffix::U32) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::U32))
            }
            Some(IntegerSuffix::U64) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::U64))
            }
        },
        Literal::Float(value) => match value.suffix {
            None => StaticExprType::UnsuffixedFloatLiteral,
            Some(FloatSuffix::F32) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::F32))
            }
            Some(FloatSuffix::F64) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::F64))
            }
        },
    }
}

fn constant_static_type(constant: &crate::Constant) -> Option<RuntimeTypeFact> {
    match constant {
        crate::Constant::Unit => Some(RuntimeTypeFact::primitive(PrimitiveTag::Unit)),
        crate::Constant::Bool(_) => Some(RuntimeTypeFact::primitive(PrimitiveTag::Bool)),
        crate::Constant::Char(_) => Some(RuntimeTypeFact::primitive(PrimitiveTag::Char)),
        crate::Constant::String(_) => Some(RuntimeTypeFact::primitive(PrimitiveTag::String)),
        crate::Constant::Bytes(_) => Some(RuntimeTypeFact::primitive(PrimitiveTag::Bytes)),
        crate::Constant::Scalar(value) => Some(RuntimeTypeFact::primitive(scalar_tag(value))),
        crate::Constant::Array(_) | crate::Constant::Map(_) => None,
    }
}

fn scalar_tag(value: &ScalarValue) -> PrimitiveTag {
    match value {
        ScalarValue::I8(_) => PrimitiveTag::I8,
        ScalarValue::I16(_) => PrimitiveTag::I16,
        ScalarValue::I32(_) => PrimitiveTag::I32,
        ScalarValue::I64(_) => PrimitiveTag::I64,
        ScalarValue::U8(_) => PrimitiveTag::U8,
        ScalarValue::U16(_) => PrimitiveTag::U16,
        ScalarValue::U32(_) => PrimitiveTag::U32,
        ScalarValue::U64(_) => PrimitiveTag::U64,
        ScalarValue::F32(_) => PrimitiveTag::F32,
        ScalarValue::F64(_) => PrimitiveTag::F64,
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

pub(in crate::compiler) fn syntax_map_key_supported(key: &SyntaxExpression) -> bool {
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
        SyntaxExpressionKind::Path => key.as_path().is_some(),
        _ => false,
    }
}

fn syntax_literal_map_key_name(source: SourceId, key: &SyntaxExpression) -> CompileResult<String> {
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

#[cfg(test)]
mod tests;
