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
        let target = self.placed_constructor_target(expression)?;
        let dst = self.alloc_register()?;
        match target {
            vela_mir::CompileConstructorTarget::Record {
                type_id,
                evaluation_order,
                fields,
                ..
            } => {
                let type_name = self
                    .type_symbol_for_expression(expression)
                    .unwrap_or_else(|| path.join("::"));
                self.require_constructor_type_name(expression, type_id, &type_name)?;
                let shape = self.record_constructor_shape(&type_name);
                let fields = self.compile_placed_constructor_fields(
                    expression,
                    hir_fields,
                    &evaluation_order,
                    &fields,
                    shape.as_ref(),
                )?;
                self.emit(UnlinkedInstructionKind::MakeRecord {
                    dst,
                    type_name,
                    fields,
                });
            }
            vela_mir::CompileConstructorTarget::Variant {
                type_id,
                variant: variant_id,
                evaluation_order,
                fields,
            } => {
                let Some((fallback_owner, fallback_variant)) = enum_variant_path(&path) else {
                    return Err(self.compile_target_input_error(
                        expression,
                        "variant placement disagrees with record-constructor HIR path",
                    ));
                };
                let enum_name = self
                    .type_symbol_for_expression(expression)
                    .unwrap_or(fallback_owner);
                self.require_constructor_type_name(expression, type_id, &enum_name)?;
                let (variant_owner, variant_name) = self
                    .facts
                    .semantic_input
                    .targets()
                    .variant_descriptor(variant_id)
                    .map(|descriptor| (descriptor.owner, descriptor.name.clone()))
                    .ok_or_else(|| {
                        self.compile_target_input_error(
                            expression,
                            "variant constructor descriptor is missing",
                        )
                    })?;
                if variant_owner != type_id || variant_name != fallback_variant {
                    return Err(self.compile_target_input_error(
                        expression,
                        "variant placement disagrees with the HIR constructor path",
                    ));
                }
                let shape = self.enum_constructor_shape(&enum_name, &variant_name);
                let fields = self.compile_placed_constructor_fields(
                    expression,
                    hir_fields,
                    &evaluation_order,
                    &fields,
                    shape.as_ref(),
                )?;
                self.emit(UnlinkedInstructionKind::MakeEnum {
                    dst,
                    enum_name,
                    variant: variant_name,
                    fields,
                });
            }
            vela_mir::CompileConstructorTarget::DynamicRecord { type_name, fields } => {
                let fields =
                    self.compile_dynamic_constructor_fields(expression, hir_fields, fields)?;
                self.emit(UnlinkedInstructionKind::MakeRecord {
                    dst,
                    type_name,
                    fields,
                });
            }
            vela_mir::CompileConstructorTarget::DynamicVariant {
                owner_name,
                variant_name,
                fields,
            } => {
                let fields =
                    self.compile_dynamic_constructor_fields(expression, hir_fields, fields)?;
                self.emit(UnlinkedInstructionKind::MakeEnum {
                    dst,
                    enum_name: owner_name,
                    variant: variant_name,
                    fields,
                });
            }
        }
        Ok(dst)
    }

    pub(in crate::compiler) fn require_constructor_type_name(
        &self,
        expression: HirExprId,
        type_id: vela_def::TypeId,
        name: &str,
    ) -> CompileResult<()> {
        let source_matches = self.facts.type_symbols.iter().any(|(declaration, symbol)| {
            symbol == name
                && self
                    .facts
                    .semantic_input
                    .targets()
                    .type_for_declaration(*declaration)
                    == Some(type_id)
        });
        let descriptor_matches = self
            .facts
            .semantic_input
            .targets()
            .type_descriptor(type_id)
            .is_some_and(|descriptor| descriptor.runtime_name == name);
        if source_matches || descriptor_matches {
            Ok(())
        } else {
            Err(self.compile_target_input_error(
                expression,
                format!("constructor type #{type_id:?} disagrees with HIR name `{name}`"),
            ))
        }
    }

    fn compile_placed_constructor_fields(
        &mut self,
        expression: HirExprId,
        hir_fields: &[vela_hir::body::HirRecordField],
        evaluation_order: &[HirExprId],
        fields: &[vela_mir::CompileConstructorField],
        shape: Option<&crate::compiler::schema_defaults::ConstructorShape>,
    ) -> CompileResult<Vec<(String, Register)>> {
        if evaluation_order.len() != hir_fields.len() {
            return Err(self.compile_target_input_error(
                expression,
                "constructor evaluation order does not cover every HIR field",
            ));
        }
        for (source, field) in evaluation_order.iter().zip(hir_fields) {
            if field.value != Some(*source) {
                return Err(self.compile_target_input_error(
                    expression,
                    "constructor evaluation order disagrees with HIR source fields",
                ));
            }
        }

        let placed = fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                if usize::try_from(field.parameter) != Ok(index) {
                    return Err(self.compile_target_input_error(
                        expression,
                        "constructor fields are not in contiguous target order",
                    ));
                }
                let name = self
                    .facts
                    .semantic_input
                    .targets()
                    .field_descriptor(field.field)
                    .map(|descriptor| descriptor.name.clone())
                    .ok_or_else(|| {
                        self.compile_target_input_error(
                            expression,
                            "constructor field descriptor is missing",
                        )
                    })?;
                Ok((name, field.value))
            })
            .collect::<CompileResult<Vec<_>>>()?;

        let mut source_registers = vec![None; evaluation_order.len()];
        for (source_index, source) in evaluation_order.iter().copied().enumerate() {
            let (field_name, value) = placed
                .iter()
                .find_map(|(name, value)| match value {
                    vela_mir::CompileConstructorValue::Explicit {
                        source_index: candidate,
                        value,
                    } if usize::try_from(*candidate) == Ok(source_index) => Some((name, *value)),
                    vela_mir::CompileConstructorValue::Explicit { .. }
                    | vela_mir::CompileConstructorValue::EvaluatedDefault(_) => None,
                })
                .ok_or_else(|| {
                    self.compile_target_input_error(
                        expression,
                        "constructor source is not referenced by a field slot",
                    )
                })?;
            if value != source || hir_fields[source_index].name != *field_name {
                return Err(self.compile_target_input_error(
                    expression,
                    "constructor field slot disagrees with source evaluation order",
                ));
            }
            let expected = shape.and_then(|shape| shape.field_value_type(field_name));
            source_registers[source_index] =
                Some(self.compile_hir_constructor_field_value(source, expected, field_name)?);
        }

        let mut compiled = Vec::with_capacity(placed.len());
        for (field_name, value) in placed {
            let register = match value {
                vela_mir::CompileConstructorValue::Explicit { source_index, .. } => {
                    let source_index = usize::try_from(source_index).map_err(|_| {
                        self.compile_target_input_error(
                            expression,
                            "constructor source index exceeds usize",
                        )
                    })?;
                    source_registers
                        .get(source_index)
                        .copied()
                        .flatten()
                        .ok_or_else(|| {
                            self.compile_target_input_error(
                                expression,
                                "constructor source index is out of bounds",
                            )
                        })?
                }
                vela_mir::CompileConstructorValue::EvaluatedDefault(body) => {
                    let value = self
                        .facts
                        .semantic_input
                        .targets()
                        .evaluated_schema_default(body)
                        .cloned()
                        .ok_or_else(|| {
                            self.compile_target_input_error(
                                expression,
                                format!("constructor default {body:?} is missing"),
                            )
                        })?;
                    self.emit_constant(
                        crate::compiler::constant_encoding::encode_evaluated_constant(&value),
                    )?
                }
            };
            compiled.push((field_name, register));
        }
        Ok(compiled)
    }

    fn compile_dynamic_constructor_fields(
        &mut self,
        expression: HirExprId,
        hir_fields: &[vela_hir::body::HirRecordField],
        fields: Vec<vela_mir::CompileDynamicConstructorField>,
    ) -> CompileResult<Vec<(String, Register)>> {
        if fields.len() != hir_fields.len() {
            return Err(self.compile_target_input_error(
                expression,
                "dynamic constructor placement does not cover every HIR field",
            ));
        }
        fields
            .into_iter()
            .zip(hir_fields)
            .map(|(field, hir)| {
                if hir.name != field.name || hir.value != Some(field.value) {
                    return Err(self.compile_target_input_error(
                        expression,
                        "dynamic constructor placement disagrees with HIR fields",
                    ));
                }
                self.compile_hir_expression(field.value)
                    .map(|register| (field.name, register))
            })
            .collect()
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
        let (span, _) = self.hir_expression_record(expression)?;
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
            && let Some(constant) = self.compile_hir_contextual_numeric_literal(expression, *tag)?
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
        if let ExpectedTypeOutcome::Contextualized(RuntimeTypeFact::Primitive(tag)) = &outcome
            && let Some(constant) = self.compile_hir_contextual_numeric_literal(expression, *tag)?
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
            HirExprKind::Unary {
                op: Some(HirUnaryOp::Negate),
                operand: Some(operand),
            } => match self.hir_static_type(operand) {
                known @ (StaticExprType::UnsuffixedIntegerLiteral
                | StaticExprType::UnsuffixedFloatLiteral) => known,
                known @ StaticExprType::Exact(RuntimeTypeFact::Primitive(tag))
                    if tag.numeric_tag().is_some() =>
                {
                    known
                }
                StaticExprType::Exact(_) | StaticExprType::Dynamic => StaticExprType::Dynamic,
            },
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

    pub(in crate::compiler) fn compile_hir_contextual_numeric_literal(
        &self,
        expression: HirExprId,
        expected: vela_common::PrimitiveTag,
    ) -> CompileResult<Option<Constant>> {
        let (span, kind) = self.hir_expression_record(expression)?;
        match kind {
            HirExprKind::Literal(literal) => {
                crate::compiler::const_eval::compile_literal_constant_for_type(&literal, expected)
                    .map_err(|error| error.with_span(span))
            }
            HirExprKind::Unary {
                op: Some(HirUnaryOp::Negate),
                operand: Some(operand),
            } => {
                let (operand_span, operand) = self.hir_expression_record(operand)?;
                let HirExprKind::Literal(literal) = operand else {
                    return Ok(None);
                };
                crate::compiler::const_eval::compile_negated_literal_constant_for_type(
                    &literal, expected,
                )
                .map_err(|error| error.with_span(operand_span))
            }
            _ => Ok(None),
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
            HirLiteral::Interpolated { parts } => self.compile_hir_interpolated_string(parts),
            HirLiteral::Invalid { .. } => Err(hir_unsupported("literal", span)),
            literal => self.compile_literal(Some(span), literal),
        }
    }

    pub(in crate::compiler) fn compile_hir_interpolated_string(
        &mut self,
        parts: &[HirInterpolatedStringPart],
    ) -> CompileResult<Register> {
        let mut compiled = Vec::with_capacity(parts.len());
        for part in parts {
            match part {
                HirInterpolatedStringPart::Text(value) => {
                    let constant = self.code.push_constant(Constant::String(value.clone()));
                    compiled.push(FormatStringPart::Text(constant));
                }
                HirInterpolatedStringPart::Expr(expression) => {
                    compiled.push(FormatStringPart::Value(
                        self.compile_hir_expression(*expression)?,
                    ));
                }
            }
        }
        let dst = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::FormatString {
            dst,
            parts: compiled,
        });
        Ok(dst)
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

    pub(in crate::compiler) fn hir_block_tail_expression(
        &self,
        block: HirBlockId,
    ) -> Option<HirExprId> {
        let statement = self
            .hir_bodies
            .iter()
            .find_map(|body| body.blocks.get(&block))?
            .statements
            .last()?;
        match self
            .hir_bodies
            .iter()
            .find_map(|body| body.statements.get(statement))?
            .kind
        {
            HirStmtKind::Expr {
                expression: Some(expression),
                ..
            } => Some(expression),
            _ => None,
        }
    }
}
