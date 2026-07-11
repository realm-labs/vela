use super::*;

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_hir_index(
        &mut self,
        span: Span,
        index: &vela_hir::body::HirIndex,
    ) -> CompileResult<Register> {
        if let Some(resolved) = self.hir_host_path(index.expression)
            && !resolved.path.segments.is_empty()
        {
            self.reject_invalid_hir_host_index_access(
                index.expression,
                HostIndexAccessKind::Read,
                span,
            )?;
            let root = self.compile_host_path_root(&resolved.path.root)?;
            let dst = self.alloc_register()?;
            self.emit_host_read(dst, root, resolved.path, span)?;
            return Ok(dst);
        }
        let base = self.compile_hir_expression(index.receiver)?;
        let dst = self.alloc_register()?;
        let key = self.hir_expression_record(index.index)?.1;
        if let HirExprKind::Literal(HirLiteral::String(key)) = key {
            let key = self.code.push_constant(Constant::String(key));
            self.emit(UnlinkedInstructionKind::GetStringKeyIndex { dst, base, key });
        } else {
            let index = self.compile_hir_expression(index.index)?;
            self.emit(UnlinkedInstructionKind::GetIndex { dst, base, index });
        }
        Ok(dst)
    }

    pub(in crate::compiler) fn compile_hir_binary(
        &mut self,
        span: Span,
        op: HirBinaryOp,
        lhs: HirExprId,
        rhs: HirExprId,
    ) -> CompileResult<Register> {
        if matches!(op, HirBinaryOp::And | HirBinaryOp::Or) {
            return self.compile_hir_logical(op, lhs, rhs);
        }
        self.reject_hir_binary_operands(op, span, lhs, rhs)?;
        if let Some(register) = self.compile_hir_numeric_literal_binary(span, op, lhs, rhs)? {
            return Ok(register);
        }
        let i64_operands = self.hir_value_type(lhs)
            == Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64))
            && self.hir_value_type(rhs)
                == Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64));
        if i64_operands
            && !matches!(self.hir_expression_record(lhs)?.1, HirExprKind::Literal(_))
            && let Some(immediate) = self.hir_i64_literal(rhs)?
            && crate::compiler::operators::i64_immediate_op_supported(op, immediate)
        {
            let lhs = self.compile_hir_expression(lhs)?;
            let dst = self.alloc_register()?;
            let instruction =
                crate::compiler::operators::i64_immediate_instruction(op, dst, lhs, immediate)
                    .expect("supported i64 immediate operation has an instruction");
            self.emit_spanned(instruction, span);
            return Ok(dst);
        }
        let lhs = self.compile_hir_expression(lhs)?;
        let rhs = self.compile_hir_expression(rhs)?;
        let dst = self.alloc_register()?;
        let instruction = if i64_operands {
            match op {
                HirBinaryOp::Add => Some(UnlinkedInstructionKind::I64Add { dst, lhs, rhs }),
                HirBinaryOp::Sub => Some(UnlinkedInstructionKind::I64Sub { dst, lhs, rhs }),
                HirBinaryOp::Mul => Some(UnlinkedInstructionKind::I64Mul { dst, lhs, rhs }),
                HirBinaryOp::Rem => Some(UnlinkedInstructionKind::I64Rem { dst, lhs, rhs }),
                _ => None,
            }
        } else {
            None
        }
        .unwrap_or_else(|| match op {
            HirBinaryOp::Range | HirBinaryOp::RangeInclusive => {
                UnlinkedInstructionKind::MakeRange {
                    dst,
                    start: lhs,
                    end: rhs,
                    inclusive: op == HirBinaryOp::RangeInclusive,
                }
            }
            HirBinaryOp::Add => UnlinkedInstructionKind::Add { dst, lhs, rhs },
            HirBinaryOp::Sub => UnlinkedInstructionKind::Sub { dst, lhs, rhs },
            HirBinaryOp::Mul => UnlinkedInstructionKind::Mul { dst, lhs, rhs },
            HirBinaryOp::Div => UnlinkedInstructionKind::Div { dst, lhs, rhs },
            HirBinaryOp::Rem => UnlinkedInstructionKind::Rem { dst, lhs, rhs },
            HirBinaryOp::Equal => UnlinkedInstructionKind::Equal { dst, lhs, rhs },
            HirBinaryOp::NotEqual => UnlinkedInstructionKind::NotEqual { dst, lhs, rhs },
            HirBinaryOp::IdentityEqual => UnlinkedInstructionKind::IdentityEqual { dst, lhs, rhs },
            HirBinaryOp::IdentityNotEqual => {
                UnlinkedInstructionKind::IdentityNotEqual { dst, lhs, rhs }
            }
            HirBinaryOp::Less => UnlinkedInstructionKind::Less { dst, lhs, rhs },
            HirBinaryOp::LessEqual => UnlinkedInstructionKind::LessEqual { dst, lhs, rhs },
            HirBinaryOp::Greater => UnlinkedInstructionKind::Greater { dst, lhs, rhs },
            HirBinaryOp::GreaterEqual => UnlinkedInstructionKind::GreaterEqual { dst, lhs, rhs },
            HirBinaryOp::And | HirBinaryOp::Or => unreachable!("logical operators return above"),
        });
        self.emit_spanned(instruction, span);
        Ok(dst)
    }

    fn compile_hir_numeric_literal_binary(
        &mut self,
        span: Span,
        op: HirBinaryOp,
        lhs: HirExprId,
        rhs: HirExprId,
    ) -> CompileResult<Option<Register>> {
        let Some((value_expression, literal_expression, literal, side)) =
            self.hir_numeric_literal_operands(lhs, rhs)?
        else {
            return Ok(None);
        };
        let Some(literal_op) = crate::compiler::operators::binary_literal_op(op) else {
            return Ok(None);
        };
        let value_type = self.hir_value_type(value_expression);
        if let Some(RuntimeTypeFact::Primitive(tag)) = value_type {
            if tag == vela_common::PrimitiveTag::I64 {
                return Ok(None);
            }
            let hir_literal = literal.to_hir_literal();
            let constant = match literal.sign() {
                vela_analysis::literals::LiteralSign::Positive => {
                    crate::compiler::const_eval::compile_literal_constant_for_type(
                        &hir_literal,
                        tag,
                    )
                }
                vela_analysis::literals::LiteralSign::Negated => {
                    crate::compiler::const_eval::compile_negated_literal_constant_for_type(
                        &hir_literal,
                        tag,
                    )
                }
            }
            .map_err(|error| {
                error.with_span(self.expression_span(literal_expression).unwrap_or(span))
            })?;
            let Some(constant) = constant else {
                return Ok(None);
            };
            let value = self.compile_hir_expression(value_expression)?;
            let literal = self.emit_constant(constant)?;
            let dst = self.alloc_register()?;
            self.emit_spanned(
                hir_non_logical_binary_instruction(op, dst, value, literal, side),
                span,
            );
            return Ok(Some(dst));
        }
        if value_type.is_some() {
            return Ok(None);
        }
        if literal.sign() == vela_analysis::literals::LiteralSign::Negated {
            return Ok(None);
        }

        let hir_literal = literal.to_hir_literal();
        let literal_span = self.expression_span(literal_expression).unwrap_or(span);
        let deferred = crate::compiler::const_eval::validate_deferred_numeric_literal(&hir_literal)
            .map_err(|error| error.with_span(literal_span))?;
        let value_register = self.compile_hir_expression(value_expression)?;
        let dst = self.alloc_register()?;
        let instruction = match deferred.kind() {
            vela_analysis::literals::NumericLiteralKind::Integer => {
                UnlinkedInstructionKind::BinaryIntLiteral {
                    dst,
                    op: literal_op,
                    value: value_register,
                    literal: deferred.text().to_owned(),
                    side,
                }
            }
            vela_analysis::literals::NumericLiteralKind::Float => {
                UnlinkedInstructionKind::BinaryFloatLiteral {
                    dst,
                    op: literal_op,
                    value: value_register,
                    literal: deferred.text().to_owned(),
                    side,
                }
            }
        };
        self.emit_spanned(instruction, span);
        Ok(Some(dst))
    }

    fn hir_numeric_literal_operands(
        &self,
        lhs: HirExprId,
        rhs: HirExprId,
    ) -> CompileResult<
        Option<(
            HirExprId,
            HirExprId,
            HirInlineNumericLiteral,
            BinaryLiteralSide,
        )>,
    > {
        let lhs_literal = self.hir_inline_numeric_literal(lhs)?;
        let rhs_literal = self.hir_inline_numeric_literal(rhs)?;
        Ok(match (lhs_literal, rhs_literal) {
            (None, Some((literal_expression, literal))) => {
                Some((lhs, literal_expression, literal, BinaryLiteralSide::Right))
            }
            (Some((literal_expression, literal)), None) => {
                Some((rhs, literal_expression, literal, BinaryLiteralSide::Left))
            }
            (Some(_), Some(_)) | (None, None) => None,
        })
    }

    fn hir_inline_numeric_literal(
        &self,
        mut expression: HirExprId,
    ) -> CompileResult<Option<(HirExprId, HirInlineNumericLiteral)>> {
        loop {
            match self.hir_expression_record(expression)?.1 {
                HirExprKind::Paren {
                    expression: Some(inner),
                } => expression = inner,
                HirExprKind::Literal(HirLiteral::Integer(value)) if value.suffix.is_none() => {
                    return Ok(Some((
                        expression,
                        HirInlineNumericLiteral::Integer {
                            literal: value,
                            sign: vela_analysis::literals::LiteralSign::Positive,
                        },
                    )));
                }
                HirExprKind::Literal(HirLiteral::Float(value)) if value.suffix.is_none() => {
                    return Ok(Some((
                        expression,
                        HirInlineNumericLiteral::Float {
                            literal: value,
                            sign: vela_analysis::literals::LiteralSign::Positive,
                        },
                    )));
                }
                HirExprKind::Unary {
                    op: Some(HirUnaryOp::Negate),
                    operand: Some(operand),
                } => match self.hir_expression_record(operand)?.1 {
                    HirExprKind::Literal(HirLiteral::Integer(value)) if value.suffix.is_none() => {
                        return Ok(Some((
                            operand,
                            HirInlineNumericLiteral::Integer {
                                literal: value,
                                sign: vela_analysis::literals::LiteralSign::Negated,
                            },
                        )));
                    }
                    HirExprKind::Literal(HirLiteral::Float(value)) if value.suffix.is_none() => {
                        return Ok(Some((
                            operand,
                            HirInlineNumericLiteral::Float {
                                literal: value,
                                sign: vela_analysis::literals::LiteralSign::Negated,
                            },
                        )));
                    }
                    _ => return Ok(None),
                },
                _ => return Ok(None),
            }
        }
    }

    pub(in crate::compiler) fn compile_hir_jump_if_false(
        &mut self,
        expression: HirExprId,
    ) -> CompileResult<usize> {
        if let HirExprKind::Binary {
            op: Some(op),
            lhs: Some(lhs),
            rhs: Some(rhs),
        } = self.hir_expression_record(expression)?.1
            && self.hir_value_type(lhs)
                == Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64))
            && let Some(immediate) = self.hir_i64_literal(rhs)?
            && let Some(compare) = crate::compiler::operators::i64_compare_op(op)
        {
            let lhs = self.compile_hir_expression(lhs)?;
            let offset = self.current_offset();
            self.emit(UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
                op: compare,
                lhs,
                imm: immediate,
                target: crate::InstructionOffset(usize::MAX),
            });
            return Ok(offset);
        }
        let condition = self.compile_hir_expression(expression)?;
        Ok(self.emit_jump_if_false(condition))
    }

    pub(in crate::compiler) fn hir_i64_literal(
        &self,
        expression: HirExprId,
    ) -> CompileResult<Option<i64>> {
        let HirExprKind::Literal(literal @ HirLiteral::Integer(_)) =
            self.hir_expression_record(expression)?.1
        else {
            return Ok(None);
        };
        match crate::compiler::const_eval::compile_literal_constant(&literal)? {
            Constant::Scalar(vela_common::ScalarValue::I64(value)) => Ok(Some(value)),
            _ => Ok(None),
        }
    }

    pub(in crate::compiler) fn reject_hir_binary_operands(
        &self,
        op: HirBinaryOp,
        span: Span,
        lhs: HirExprId,
        rhs: HirExprId,
    ) -> CompileResult<()> {
        if matches!(
            op,
            HirBinaryOp::IdentityEqual | HirBinaryOp::IdentityNotEqual
        ) {
            for (side, operand) in [("left", lhs), ("right", rhs)] {
                let scalar = self.hir_value_type(operand).and_then(|fact| match fact {
                    RuntimeTypeFact::Primitive(_)
                    | RuntimeTypeFact::Standard(
                        crate::compiler::value_types::StandardRuntimeType::Range,
                    ) => Some(fact.source_type_display()),
                    _ => None,
                });
                let type_name = scalar.or_else(|| {
                    self.value_shape_for_hir_expression(operand)
                        .and_then(|shape| match shape {
                            ValueShape::Scalar(name) => Some(name),
                            _ => None,
                        })
                });
                if let Some(type_name) = type_name {
                    let operand_span = self.expression_span(operand).unwrap_or(span);
                    return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                        vec![
                            Diagnostic::error(format!(
                                "`{}` requires reference identity operands, but the {side} operand has type `{type_name}`",
                                hir_binary_op_name(op)
                            ))
                            .with_code("compiler::invalid_identity_comparison")
                            .with_span(span)
                            .with_label(span, "identity comparison requires reference operands")
                            .with_label(
                                operand_span,
                                format!("{side} operand is statically `{type_name}`"),
                            ),
                        ],
                    )));
                }
            }
            return Ok(());
        }
        let left = self
            .script_fact_for_hir_expression(lhs)
            .map(|fact| fact.type_name);
        let right = self
            .script_fact_for_hir_expression(rhs)
            .map(|fact| fact.type_name);
        self.reject_static_script_path_binary_operands(op, span, left.as_deref(), right.as_deref())
    }

    pub(in crate::compiler) fn compile_hir_logical(
        &mut self,
        op: HirBinaryOp,
        lhs: HirExprId,
        rhs: HirExprId,
    ) -> CompileResult<Register> {
        let operands = self.hir_logical_operands(op, lhs, rhs)?;
        let dst = self.alloc_register()?;
        match op {
            HirBinaryOp::And => {
                let (last, prefix) = operands
                    .split_last()
                    .expect("a binary logical expression has at least two operands");
                let mut false_branches = Vec::with_capacity(prefix.len());
                for operand in prefix {
                    let value = self.compile_hir_expression(*operand)?;
                    false_branches.push(self.emit_jump_if_false(value));
                }

                let last = self.compile_hir_expression(*last)?;
                self.emit_truthy_to_bool(dst, last)?;
                let end = self.emit_jump();
                for branch in false_branches {
                    self.patch_jump(branch, self.current_offset())?;
                }
                self.emit_bool_constant_to(dst, false);
                self.patch_jump(end, self.current_offset())?;
            }
            HirBinaryOp::Or => {
                let (last, prefix) = operands
                    .split_last()
                    .expect("a binary logical expression has at least two operands");
                let mut end_jumps = Vec::with_capacity(prefix.len());
                for operand in prefix {
                    let value = self.compile_hir_expression(*operand)?;
                    let next_operand = self.emit_jump_if_false(value);
                    self.emit_bool_constant_to(dst, true);
                    end_jumps.push(self.emit_jump());
                    self.patch_jump(next_operand, self.current_offset())?;
                }

                let last = self.compile_hir_expression(*last)?;
                self.emit_truthy_to_bool(dst, last)?;
                for end in end_jumps {
                    self.patch_jump(end, self.current_offset())?;
                }
            }
            _ => unreachable!("logical lowering requires logical operator"),
        }
        Ok(dst)
    }

    fn hir_logical_operands(
        &self,
        op: HirBinaryOp,
        lhs: HirExprId,
        rhs: HirExprId,
    ) -> CompileResult<Vec<HirExprId>> {
        let mut operands = Vec::new();
        let mut pending = vec![rhs, lhs];
        while let Some(expression) = pending.pop() {
            match self.hir_expression_record(expression)?.1 {
                HirExprKind::Binary {
                    op: Some(nested_op),
                    lhs: Some(nested_lhs),
                    rhs: Some(nested_rhs),
                } if nested_op == op => {
                    pending.push(nested_rhs);
                    pending.push(nested_lhs);
                }
                _ => operands.push(expression),
            }
        }
        Ok(operands)
    }
}

#[derive(Clone)]
enum HirInlineNumericLiteral {
    Integer {
        literal: vela_hir::body::HirIntegerLiteral,
        sign: vela_analysis::literals::LiteralSign,
    },
    Float {
        literal: vela_hir::body::HirFloatLiteral,
        sign: vela_analysis::literals::LiteralSign,
    },
}

impl HirInlineNumericLiteral {
    fn to_hir_literal(&self) -> HirLiteral {
        match self {
            Self::Integer { literal, .. } => HirLiteral::Integer(literal.clone()),
            Self::Float { literal, .. } => HirLiteral::Float(literal.clone()),
        }
    }

    const fn sign(&self) -> vela_analysis::literals::LiteralSign {
        match self {
            Self::Integer { sign, .. } | Self::Float { sign, .. } => *sign,
        }
    }
}

fn hir_non_logical_binary_instruction(
    op: HirBinaryOp,
    dst: Register,
    value: Register,
    literal: Register,
    side: BinaryLiteralSide,
) -> UnlinkedInstructionKind {
    let (lhs, rhs) = match side {
        BinaryLiteralSide::Left => (literal, value),
        BinaryLiteralSide::Right => (value, literal),
    };
    match op {
        HirBinaryOp::Add => UnlinkedInstructionKind::Add { dst, lhs, rhs },
        HirBinaryOp::Sub => UnlinkedInstructionKind::Sub { dst, lhs, rhs },
        HirBinaryOp::Mul => UnlinkedInstructionKind::Mul { dst, lhs, rhs },
        HirBinaryOp::Div => UnlinkedInstructionKind::Div { dst, lhs, rhs },
        HirBinaryOp::Rem => UnlinkedInstructionKind::Rem { dst, lhs, rhs },
        HirBinaryOp::Less => UnlinkedInstructionKind::Less { dst, lhs, rhs },
        HirBinaryOp::LessEqual => UnlinkedInstructionKind::LessEqual { dst, lhs, rhs },
        HirBinaryOp::Greater => UnlinkedInstructionKind::Greater { dst, lhs, rhs },
        HirBinaryOp::GreaterEqual => UnlinkedInstructionKind::GreaterEqual { dst, lhs, rhs },
        _ => unreachable!("numeric literal operators were checked before lowering"),
    }
}
