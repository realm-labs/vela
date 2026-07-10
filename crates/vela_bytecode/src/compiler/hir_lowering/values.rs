use super::*;

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_hir_if_value_to(
        &mut self,
        value: &HirIf,
        dst: Register,
    ) -> CompileResult<bool> {
        let condition = value.condition.ok_or_else(|| {
            CompileError::new(CompileErrorKind::UnsupportedSyntax("if condition"))
        })?;
        let jump_to_else = self.compile_hir_jump_if_false(condition)?;
        let then_returned = match value.then_block {
            Some(block) => self.compile_hir_block_value_to(block, dst)?,
            None => {
                self.emit_constant_to(dst, Constant::Unit);
                false
            }
        };
        let jump_to_end = (!then_returned).then(|| self.emit_jump());
        self.patch_jump(jump_to_else, self.current_offset())?;
        let else_returned = match value.else_branch.as_ref() {
            Some(HirElseBranch::If(value)) => self.compile_hir_if_value_to(value, dst)?,
            Some(HirElseBranch::Block(block)) => self.compile_hir_block_value_to(*block, dst)?,
            None => {
                self.emit_constant_to(dst, Constant::Unit);
                false
            }
        };
        if let Some(jump_to_end) = jump_to_end {
            self.patch_jump(jump_to_end, self.current_offset())?;
        }
        Ok(then_returned && else_returned)
    }

    pub(in crate::compiler) fn compile_hir_record(
        &mut self,
        expression: HirExprId,
        span: Span,
        hir_fields: &[vela_hir::body::HirRecordField],
    ) -> CompileResult<Register> {
        let path = self
            .hir_constructor_path(expression)
            .ok_or_else(|| hir_unsupported("record constructor", span))?
            .to_vec();
        let field_uses = hir_fields
            .iter()
            .map(|field| ConstructorFieldUse {
                name: field.name.clone(),
                span: field.name_origin.span,
            })
            .collect::<Vec<_>>();
        let (enum_constructor, record_type_name, shape) = if let Some((enum_name, variant)) =
            enum_variant_path(&path)
        {
            let resolved = self.type_symbol_for_expression(expression);
            let enum_name = resolved.clone().unwrap_or(enum_name);
            if resolved.is_some() && !self.enum_constructor_variant_exists(&enum_name, &variant) {
                return Err(self.constructor_diagnostics_error(vec![
                    unknown_enum_variant_diagnostic(&enum_name, &variant, span),
                ]));
            }
            let shape = self.enum_constructor_shape(&enum_name, &variant);
            self.reject_constructor_diagnostics(record_constructor_field_diagnostics(
                &format!("{enum_name}::{variant}"),
                shape.as_ref(),
                &field_uses,
                span,
            ))?;
            (Some((enum_name, variant)), None, shape)
        } else {
            let type_name = self
                .type_symbol_for_expression(expression)
                .unwrap_or_else(|| path.join("::"));
            let shape = self.record_constructor_shape(&type_name);
            self.reject_constructor_diagnostics(record_constructor_field_diagnostics(
                &type_name,
                shape.as_ref(),
                &field_uses,
                span,
            ))?;
            (None, Some(type_name), shape)
        };

        let mut fields = Vec::with_capacity(hir_fields.len());
        let mut explicit_names = BTreeSet::new();
        for field in hir_fields {
            let value = field
                .value
                .ok_or_else(|| hir_unsupported("record field", field.name_origin.span))?;
            let expected = shape
                .as_ref()
                .and_then(|shape| shape.field_value_type(&field.name));
            let value = self.compile_hir_constructor_field_value(value, expected, &field.name)?;
            explicit_names.insert(field.name.clone());
            fields.push((field.name.clone(), value));
        }
        self.compile_schema_default_fields(
            &mut fields,
            &explicit_names,
            schema_default_fields(shape.as_ref()),
            shape.as_ref(),
        )?;
        let dst = self.alloc_register()?;
        if let Some((enum_name, variant)) = enum_constructor {
            self.emit(UnlinkedInstructionKind::MakeEnum {
                dst,
                enum_name,
                variant,
                fields,
            });
        } else {
            self.emit(UnlinkedInstructionKind::MakeRecord {
                dst,
                type_name: record_type_name.expect("record constructor has a type"),
                fields,
            });
        }
        Ok(dst)
    }

    pub(in crate::compiler) fn compile_hir_constructor_field_value(
        &mut self,
        expression: HirExprId,
        expected: Option<RuntimeTypeFact>,
        field_name: &str,
    ) -> CompileResult<Register> {
        let Some(expected) = expected else {
            return self.compile_hir_expression(expression);
        };
        let (span, kind) = self.hir_expression_record(expression)?;
        let context = TypeContractContext::Field {
            name: field_name.to_owned(),
        };
        let outcome = check_expected_type(
            self.hir_static_type(expression),
            expected,
            span,
            context.clone(),
        )?;
        if let ExpectedTypeOutcome::Contextualized(RuntimeTypeFact::Primitive(tag)) = &outcome
            && let HirExprKind::Literal(literal) = &kind
            && let Some(constant) =
                crate::compiler::const_eval::compile_literal_constant_for_type(literal, *tag)
                    .map_err(|error| error.with_span(span))?
        {
            return self.emit_constant(constant);
        }
        let value = self.compile_hir_expression(expression)?;
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(expected) = &outcome
            && let Some((location, name)) = guard_location_and_name(context)
            && let Some(plan) = crate::compiler::type_guard_plan_for_runtime_type(expected)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: value,
                    guard: UnlinkedTypeGuard::new(
                        plan,
                        UnlinkedGuardContext::new(GuardKind::Contract, location, name),
                    ),
                },
                span,
            );
        }
        Ok(value)
    }

    pub(in crate::compiler) fn compile_hir_let_initializer(
        &mut self,
        expression: HirExprId,
        hint: Option<&vela_hir::type_hint::HirTypeHint>,
        name: &str,
        span: Span,
    ) -> CompileResult<Register> {
        let Some(hint) = hint else {
            return self.compile_hir_expression(expression);
        };
        let Some(expected) = crate::compiler::value_types::type_hint_value_type(hint) else {
            let value = self.compile_hir_expression(expression)?;
            let hinted_script = crate::compiler::script_types::type_hint_script_type(
                hint,
                self.facts.known_type_names().iter(),
            );
            let actual_script = self.script_fact_for_hir_expression(expression);
            let proven_script = actual_script.is_some()
                && (hinted_script.is_none()
                    || hinted_script.as_deref().is_some_and(|expected| {
                        actual_script.as_ref().is_some_and(|actual| {
                            actual.type_name == expected
                                || actual.type_name.ends_with(&format!("::{expected}"))
                        })
                    })
                    || matches!(
                        self.hir_expression_record(expression).map(|(_, kind)| kind),
                        Ok(HirExprKind::Record { .. })
                    ));
            if proven_script {
                return Ok(value);
            }
            if let Some(guard) = crate::compiler::type_guard_for_hint(
                hint,
                crate::GuardLocation::Local,
                name,
                &self.facts,
            ) {
                self.emit_spanned(
                    UnlinkedInstructionKind::GuardType { src: value, guard },
                    span,
                );
            }
            return Ok(value);
        };
        let context = TypeContractContext::TypedLet {
            name: name.to_owned(),
        };
        let outcome = check_expected_type(
            self.hir_static_type(expression),
            expected,
            span,
            context.clone(),
        )?;
        let (expression_span, kind) = self.hir_expression_record(expression)?;
        if let ExpectedTypeOutcome::Contextualized(RuntimeTypeFact::Primitive(tag)) = &outcome
            && let HirExprKind::Literal(literal) = &kind
            && let Some(constant) =
                crate::compiler::const_eval::compile_literal_constant_for_type(literal, *tag)
                    .map_err(|error| error.with_span(expression_span))?
        {
            return self.emit_constant(constant);
        }
        let value = self.compile_hir_expression(expression)?;
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(expected) = outcome
            && let Some(plan) = crate::compiler::type_guard_plan_for_runtime_type(&expected)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: value,
                    guard: UnlinkedTypeGuard::new(
                        plan,
                        UnlinkedGuardContext::new(
                            GuardKind::Contract,
                            crate::GuardLocation::Local,
                            name,
                        ),
                    ),
                },
                span,
            );
        }
        Ok(value)
    }

    pub(in crate::compiler) fn hir_static_type(&self, expression: HirExprId) -> StaticExprType {
        let Ok((_, kind)) = self.hir_expression_record(expression) else {
            return StaticExprType::Dynamic;
        };
        match kind {
            HirExprKind::Literal(HirLiteral::Integer(value)) if value.suffix.is_none() => {
                StaticExprType::UnsuffixedIntegerLiteral
            }
            HirExprKind::Literal(HirLiteral::Float(value)) if value.suffix.is_none() => {
                StaticExprType::UnsuffixedFloatLiteral
            }
            HirExprKind::Literal(HirLiteral::Bool(_)) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Bool))
            }
            HirExprKind::Literal(HirLiteral::Char(_)) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Char))
            }
            HirExprKind::Literal(HirLiteral::String(_) | HirLiteral::Interpolated { .. }) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::String,
                ))
            }
            HirExprKind::Literal(HirLiteral::Bytes(_)) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Bytes))
            }
            HirExprKind::Literal(HirLiteral::Integer(value)) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(integer_suffix_tag(value.suffix)))
            }
            HirExprKind::Literal(HirLiteral::Float(value)) => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(float_suffix_tag(value.suffix)))
            }
            HirExprKind::Unit => {
                StaticExprType::Exact(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::Unit))
            }
            HirExprKind::Path(_) => self
                .local_for_expression(expression)
                .and_then(|local| self.value_types.local(local))
                .map(StaticExprType::Exact)
                .unwrap_or(StaticExprType::Dynamic),
            HirExprKind::Binary {
                op: Some(op),
                lhs: Some(lhs),
                rhs: Some(rhs),
            } => {
                if matches!(
                    op,
                    HirBinaryOp::Equal
                        | HirBinaryOp::NotEqual
                        | HirBinaryOp::IdentityEqual
                        | HirBinaryOp::IdentityNotEqual
                        | HirBinaryOp::Less
                        | HirBinaryOp::LessEqual
                        | HirBinaryOp::Greater
                        | HirBinaryOp::GreaterEqual
                        | HirBinaryOp::And
                        | HirBinaryOp::Or
                ) {
                    StaticExprType::Exact(RuntimeTypeFact::primitive(
                        vela_common::PrimitiveTag::Bool,
                    ))
                } else if matches!(
                    op,
                    HirBinaryOp::Add
                        | HirBinaryOp::Sub
                        | HirBinaryOp::Mul
                        | HirBinaryOp::Div
                        | HirBinaryOp::Rem
                ) && self.hir_value_type(lhs)
                    == Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64))
                    && self.hir_value_type(rhs)
                        == Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64))
                {
                    StaticExprType::Exact(RuntimeTypeFact::primitive(
                        vela_common::PrimitiveTag::I64,
                    ))
                } else {
                    StaticExprType::Dynamic
                }
            }
            _ => StaticExprType::Dynamic,
        }
    }

    pub(in crate::compiler) fn hir_value_type(
        &self,
        expression: HirExprId,
    ) -> Option<RuntimeTypeFact> {
        match self.hir_static_type(expression) {
            StaticExprType::Exact(fact) => Some(fact),
            StaticExprType::UnsuffixedIntegerLiteral => {
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::I64))
            }
            StaticExprType::UnsuffixedFloatLiteral => {
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::F64))
            }
            StaticExprType::Dynamic => self
                .value_shape_for_hir_expression(expression)
                .and_then(|shape| shape.value_type()),
        }
    }

    pub(in crate::compiler) fn hir_expression_record(
        &self,
        expression: HirExprId,
    ) -> CompileResult<(Span, HirExprKind)> {
        self.hir_bodies
            .iter()
            .find_map(|body| body.expression(expression))
            .map(|expression| (expression.origin.span, expression.kind.clone()))
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax("HIR expression")))
    }

    pub(in crate::compiler) fn compile_hir_expressions(
        &mut self,
        expressions: &[HirExprId],
    ) -> CompileResult<Vec<Register>> {
        expressions
            .iter()
            .map(|expression| self.compile_hir_expression(*expression))
            .collect()
    }

    pub(in crate::compiler) fn compile_hir_literal(
        &mut self,
        span: Span,
        literal: &HirLiteral,
    ) -> CompileResult<Register> {
        match literal {
            HirLiteral::Interpolated {
                source_text,
                expressions,
            } => self.compile_hir_interpolated_string(span, source_text, expressions),
            HirLiteral::Invalid { .. } => Err(hir_unsupported("literal", span)),
            literal => self.compile_literal(Some(span), literal),
        }
    }

    pub(in crate::compiler) fn compile_hir_interpolated_string(
        &mut self,
        span: Span,
        source_text: &str,
        expressions: &[HirExprId],
    ) -> CompileResult<Register> {
        let parts = vela_syntax::lexer::lex(span.source, source_text)
            .tokens
            .into_iter()
            .find_map(|token| match token.kind {
                TokenKind::InterpolatedString(parts) => Some(parts),
                _ => None,
            })
            .ok_or_else(|| hir_unsupported("interpolated string", span))?;
        let mut expressions = expressions.iter();
        let mut compiled = Vec::with_capacity(parts.len());
        for part in parts {
            match part {
                InterpolatedStringTokenPart::Text(value) => {
                    let constant = self.code.push_constant(Constant::String(value));
                    compiled.push(FormatStringPart::Text(constant));
                }
                InterpolatedStringTokenPart::Expr { .. } => {
                    let expression = expressions
                        .next()
                        .ok_or_else(|| hir_unsupported("interpolated expression", span))?;
                    compiled.push(FormatStringPart::Value(
                        self.compile_hir_expression(*expression)?,
                    ));
                }
            }
        }
        if expressions.next().is_some() {
            return Err(hir_unsupported("interpolated expression", span));
        }
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::FormatString {
            dst,
            parts: compiled,
        });
        Ok(dst)
    }

    pub(in crate::compiler) fn hir_map_key(&self, expression: HirExprId) -> CompileResult<String> {
        let (span, kind) = self.hir_expression_record(expression)?;
        match kind {
            HirExprKind::Literal(HirLiteral::String(value)) => Ok(value),
            HirExprKind::Literal(HirLiteral::Char(value)) => Ok(value.to_string()),
            HirExprKind::Literal(HirLiteral::Integer(value)) => Ok(integer_text(&value)),
            HirExprKind::Literal(HirLiteral::Float(value)) => Ok(float_text(&value)),
            HirExprKind::Path(path) => self
                .hir_bodies
                .iter()
                .find_map(|body| body.paths.get(&path))
                .filter(|path| path.kind == HirPathKind::Value)
                .map(|path| path.path.join("::"))
                .filter(|path| !path.is_empty())
                .ok_or_else(|| hir_unsupported("map key", span)),
            _ => Err(hir_unsupported("map key", span)),
        }
    }

    pub(in crate::compiler) fn compile_hir_unary(
        &mut self,
        span: Span,
        op: HirUnaryOp,
        operand: HirExprId,
    ) -> CompileResult<Register> {
        if op == HirUnaryOp::Negate
            && let HirExprKind::Literal(literal) = self.hir_expression_record(operand)?.1
            && let Some(constant) =
                crate::compiler::const_eval::compile_negated_literal_constant(&literal)
                    .map_err(|error| error.with_span(span))?
        {
            return self.emit_constant(constant);
        }
        if op == HirUnaryOp::Not {
            let mut equality = operand;
            while let HirExprKind::Paren {
                expression: Some(inner),
            } = self.hir_expression_record(equality)?.1
            {
                equality = inner;
            }
            if let HirExprKind::Binary {
                op: Some(binary),
                lhs: Some(lhs),
                rhs: Some(rhs),
            } = self.hir_expression_record(equality)?.1
            {
                let inverse = match binary {
                    HirBinaryOp::Equal => Some(HirBinaryOp::NotEqual),
                    HirBinaryOp::NotEqual => Some(HirBinaryOp::Equal),
                    HirBinaryOp::IdentityEqual => Some(HirBinaryOp::IdentityNotEqual),
                    HirBinaryOp::IdentityNotEqual => Some(HirBinaryOp::IdentityEqual),
                    _ => None,
                };
                if let Some(inverse) = inverse {
                    return self.compile_hir_binary(span, inverse, lhs, rhs);
                }
            }
        }
        let src = self.compile_hir_expression(operand)?;
        let dst = self.alloc_register()?;
        self.emit_spanned(
            match op {
                HirUnaryOp::Not => UnlinkedInstructionKind::Not { dst, src },
                HirUnaryOp::Negate => UnlinkedInstructionKind::Negate { dst, src },
            },
            span,
        );
        Ok(dst)
    }

    pub(in crate::compiler) fn compile_hir_field(
        &mut self,
        span: Span,
        field: &vela_hir::body::HirField,
    ) -> CompileResult<Register> {
        if let Some(resolved) = self.hir_host_path(field.expression)
            && !resolved.path.segments.is_empty()
        {
            let root = self.compile_host_path_root(&resolved.path.root)?;
            let dst = self.alloc_register()?;
            self.emit_host_read(dst, root, resolved.path, span)?;
            return Ok(dst);
        }
        let record = self.compile_hir_expression(field.receiver)?;
        let dst = self.alloc_register()?;
        if let Ok(index) = field.name.parse() {
            self.emit(UnlinkedInstructionKind::GetTupleField {
                dst,
                value: record,
                index,
            });
            return Ok(dst);
        }
        let fact = self.script_fact_for_hir_expression(field.receiver);
        if let Some(enum_fact) = fact.as_ref().filter(|fact| fact.enum_variant.is_some()) {
            let slot = enum_fact
                .enum_variant
                .as_deref()
                .and_then(|variant| {
                    self.facts.script_field_slots.enum_variant(
                        &enum_fact.type_name,
                        variant,
                        &field.name,
                    )
                })
                .or_else(|| self.hir_record_field_slot(field.receiver, &field.name));
            if let Some(slot) = slot {
                self.emit(UnlinkedInstructionKind::GetEnumSlot {
                    dst,
                    value: record,
                    field: field.name.clone(),
                    slot,
                });
            } else {
                self.emit(UnlinkedInstructionKind::GetEnumField {
                    dst,
                    value: record,
                    field: field.name.clone(),
                });
            }
        } else if let Some(slot) = fact
            .as_ref()
            .and_then(|fact| self.script_record_field_slot_for_type(&fact.type_name, &field.name))
            .or_else(|| {
                self.value_shape_for_hir_expression(field.receiver)
                    .and_then(|shape| {
                        shape
                            .as_record()
                            .and_then(|shape| shape.field_slot(&field.name))
                    })
            })
        {
            self.emit(UnlinkedInstructionKind::GetRecordSlot {
                dst,
                record,
                field: field.name.clone(),
                slot,
            });
        } else {
            self.emit(UnlinkedInstructionKind::GetRecordField {
                dst,
                record,
                field: field.name.clone(),
            });
        }
        Ok(dst)
    }

    pub(in crate::compiler) fn hir_record_field_slot(
        &self,
        expression: HirExprId,
        name: &str,
    ) -> Option<usize> {
        let HirExprKind::Record { fields, .. } = self.hir_expression_record(expression).ok()?.1
        else {
            return None;
        };
        let mut names = fields
            .into_iter()
            .map(|field| field.name)
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        names.iter().position(|field| field == name)
    }
}
