use super::*;

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_hir_expression(
        &mut self,
        expression: HirExprId,
    ) -> CompileResult<Register> {
        let (span, kind) = self.hir_expression_record(expression)?;
        match kind {
            HirExprKind::Literal(literal) => self.compile_hir_literal(span, &literal),
            HirExprKind::Path(path) => {
                let path = self
                    .hir_bodies
                    .iter()
                    .find_map(|body| body.paths.get(&path))
                    .ok_or_else(|| hir_unsupported("path", span))?;
                self.compile_path_expr(expression, span, &path.path)
            }
            HirExprKind::Paren {
                expression: Some(inner),
            } => self.compile_hir_expression(inner),
            HirExprKind::Unit => self.emit_constant(Constant::Unit),
            HirExprKind::Tuple { elements } => {
                let elements = self.compile_hir_expressions(&elements)?;
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::MakeTuple { dst, elements });
                Ok(dst)
            }
            HirExprKind::Array { elements } => {
                let elements = self.compile_hir_expressions(&elements)?;
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::MakeArray { dst, elements });
                Ok(dst)
            }
            HirExprKind::Map { entries } => {
                let mut compiled = Vec::with_capacity(entries.len());
                for entry in entries {
                    let key = entry
                        .key
                        .ok_or_else(|| hir_unsupported("map key", entry.origin.span))?;
                    let value = entry
                        .value
                        .ok_or_else(|| hir_unsupported("map value", entry.origin.span))?;
                    compiled.push((self.hir_map_key(key)?, self.compile_hir_expression(value)?));
                }
                let dst = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::MakeMap {
                    dst,
                    entries: compiled,
                });
                Ok(dst)
            }
            HirExprKind::Unary {
                op: Some(op),
                operand: Some(operand),
            } => self.compile_hir_unary(span, op, operand),
            HirExprKind::Binary {
                op: Some(op),
                lhs: Some(lhs),
                rhs: Some(rhs),
            } => self.compile_hir_binary(span, op, lhs, rhs),
            HirExprKind::Field(field) => self.compile_hir_field(span, &field),
            HirExprKind::Call(call) => self.compile_hir_call(span, &call),
            HirExprKind::Index(index) => self.compile_hir_index(span, &index),
            HirExprKind::Record { fields, .. } => {
                self.compile_hir_record(expression, span, &fields)
            }
            HirExprKind::Block { block } => {
                let dst = self.alloc_register()?;
                self.compile_hir_block_value_to(block, dst)?;
                Ok(dst)
            }
            HirExprKind::If(value) => {
                let dst = self.alloc_register()?;
                self.compile_hir_if_value_to(&value, dst)?;
                Ok(dst)
            }
            HirExprKind::Try {
                expression: Some(inner),
            } => {
                let src = self.compile_hir_expression(inner)?;
                let dst = self.alloc_register()?;
                self.emit_spanned(
                    UnlinkedInstructionKind::TryPropagate {
                        dst,
                        src,
                        expected: self.expected_try_propagation_family(),
                    },
                    span,
                );
                Ok(dst)
            }
            HirExprKind::Assign {
                op: Some(op),
                target: Some(target),
                value: Some(value),
            } => self.compile_hir_assignment(span, op, target, value),
            HirExprKind::Match(value) => {
                let dst = self.alloc_register()?;
                self.compile_hir_match(&value, Some(dst))?;
                Ok(dst)
            }
            HirExprKind::Lambda { .. } => self.compile_hir_lambda(expression, &[]),
            HirExprKind::Paren { expression: None }
            | HirExprKind::Unary { .. }
            | HirExprKind::Binary { .. }
            | HirExprKind::Assign { .. }
            | HirExprKind::Try { expression: None }
            | HirExprKind::Missing => Err(hir_unsupported("expression", span)),
        }
    }

    pub(in crate::compiler) fn compile_hir_assignment(
        &mut self,
        span: Span,
        op: HirAssignOp,
        mut target: HirExprId,
        value: HirExprId,
    ) -> CompileResult<Register> {
        while let HirExprKind::Paren {
            expression: Some(inner),
        } = self.hir_expression_record(target)?.1
        {
            target = inner;
        }
        let value = if op == HirAssignOp::Set
            && let HirExprKind::Field(field) = self.hir_expression_record(target)?.1
            && let Some(expected) = self
                .script_fact_for_hir_expression(field.receiver)
                .and_then(|fact| self.record_constructor_shape(&fact.type_name))
                .and_then(|shape| shape.field_value_type(&field.name))
        {
            self.compile_hir_expression_for_expected_type(
                value,
                expected,
                TypeContractContext::Field {
                    name: field.name.clone(),
                },
                &[],
            )?
            .0
        } else {
            self.compile_hir_expression(value)?
        };
        if let Some(resolved) = self.hir_host_path(target)
            && !resolved.path.segments.is_empty()
        {
            self.reject_invalid_hir_host_assignment(target, op, span)?;
            let root = self.compile_host_path_root(&resolved.path.root)?;
            match op {
                HirAssignOp::Set => self.emit_host_write(root, resolved.path, value, span)?,
                _ => self.emit_host_mutate(
                    root,
                    resolved.path,
                    hir_host_mutation_op(op).expect("compound assignment has host mutation op"),
                    value,
                    span,
                )?,
            }
            return Ok(value);
        }

        let (_, target_kind) = self.hir_expression_record(target)?;
        match target_kind {
            HirExprKind::Path(_) => self.compile_hir_local_assignment(op, target, value, span),
            HirExprKind::Index(index) => self.compile_hir_index_assignment(op, &index, value),
            HirExprKind::Field(field) => self.compile_hir_field_assignment(op, &field, value),
            _ => Err(hir_unsupported("assignment target", span)),
        }
    }

    pub(in crate::compiler) fn reject_invalid_hir_host_assignment(
        &self,
        target: HirExprId,
        op: HirAssignOp,
        span: Span,
    ) -> CompileResult<()> {
        let (_, kind) = self.hir_expression_record(target)?;
        match kind {
            HirExprKind::Index(_) => self.reject_invalid_hir_host_index_access(
                target,
                if op == HirAssignOp::Set {
                    HostIndexAccessKind::Write
                } else {
                    HostIndexAccessKind::Mutate
                },
                span,
            ),
            HirExprKind::Field(field) => {
                let receiver_type = self
                    .script_fact_for_hir_expression(field.receiver)
                    .map(|fact| fact.type_name)
                    .or_else(|| {
                        self.hir_host_path(field.receiver)
                            .and_then(|resolved| resolved.type_name)
                    });
                let Some(access) = receiver_type
                    .as_deref()
                    .and_then(|receiver| self.host_field_info(Some(receiver), &field.name))
                else {
                    return Ok(());
                };
                if access.writable || access.variant_field {
                    return Ok(());
                }
                Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                    vec![
                        Diagnostic::error("field is read-only for script writes")
                            .with_code("analysis::field_not_writable")
                            .with_span(span)
                            .with_label(span, "assignment targets a read-only field")
                            .with_label(
                                span,
                                "write through an exposed method or a writable field instead",
                            ),
                    ],
                )))
            }
            _ => Ok(()),
        }
    }

    pub(in crate::compiler) fn reject_invalid_hir_host_index_access(
        &self,
        expression: HirExprId,
        kind: HostIndexAccessKind,
        error_span: Span,
    ) -> CompileResult<()> {
        let index = self
            .hir_index_for_expression(expression)
            .ok_or_else(|| hir_unsupported("host index", error_span))?;
        let receiver_span = self.expression_span(index.receiver).unwrap_or(error_span);
        let receiver_type = self
            .script_fact_for_hir_expression(index.receiver)
            .map(|fact| fact.type_name)
            .filter(|type_name| self.host_runtime_type_id(type_name).is_some())
            .or_else(|| {
                self.hir_host_path(index.receiver)
                    .and_then(|resolved| resolved.type_name)
            });
        let Some(receiver_type) = receiver_type else {
            return Ok(());
        };
        let Some(capability) = self.facts.options.host_index_capability(&receiver_type) else {
            return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                vec![
                    Diagnostic::error(format!(
                        "type `{receiver_type}` does not support host index access"
                    ))
                    .with_code("analysis::host_index_not_supported")
                    .with_span(error_span)
                    .with_label(
                        error_span,
                        "host index access is not registered for this type",
                    )
                    .with_label(
                        receiver_span,
                        "register a host index capability or expose a field/method instead",
                    ),
                ],
            )));
        };
        if !kind.allowed_by(capability) {
            return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                vec![
                    Diagnostic::error(format!(
                        "type `{receiver_type}` does not allow host index {}",
                        kind.access_name()
                    ))
                    .with_code(kind.denied_code())
                    .with_span(error_span)
                    .with_label(error_span, kind.capability_label())
                    .with_label(receiver_span, kind.enable_label()),
                ],
            )));
        }
        if let Some(expected) = capability.key_type.as_deref()
            && let Some(actual) = self.hir_value_type(index.index)
            && actual.source_type_name() != expected
            && actual.std_type_name() != expected
        {
            let index_span = self.expression_span(index.index).unwrap_or(error_span);
            return Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
                vec![
                    Diagnostic::error(format!(
                        "host index key for `{receiver_type}` must be `{expected}`"
                    ))
                    .with_code("analysis::host_index_key_mismatch")
                    .with_span(error_span)
                    .with_label(
                        index_span,
                        format!("index expression has type `{}`", actual.source_type_name()),
                    ),
                ],
            )));
        }
        Ok(())
    }

    pub(in crate::compiler) fn compile_hir_local_assignment(
        &mut self,
        op: HirAssignOp,
        target: HirExprId,
        value: Register,
        span: Span,
    ) -> CompileResult<Register> {
        let local = self
            .local_for_expression(target)
            .ok_or_else(|| hir_unsupported("local assignment", span))?;
        let target = self
            .hir_locals
            .get(&local)
            .copied()
            .ok_or_else(|| hir_unsupported("local assignment", span))?;
        if op == HirAssignOp::Set {
            self.emit(UnlinkedInstructionKind::Move {
                dst: target,
                src: value,
            });
            return Ok(value);
        }
        let dst = self.alloc_register()?;
        let instruction = hir_compound_instruction(
            op,
            dst,
            target,
            value,
            self.value_types.local(local)
                == Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64)),
        )
        .ok_or_else(|| hir_unsupported("compound assignment", span))?;
        self.emit(instruction);
        self.emit(UnlinkedInstructionKind::Move {
            dst: target,
            src: dst,
        });
        Ok(dst)
    }

    pub(in crate::compiler) fn compile_hir_index_assignment(
        &mut self,
        op: HirAssignOp,
        index: &vela_hir::body::HirIndex,
        value: Register,
    ) -> CompileResult<Register> {
        let base = self.compile_hir_expression(index.receiver)?;
        if let HirExprKind::Literal(HirLiteral::String(key)) =
            self.hir_expression_record(index.index)?.1
        {
            let key = self.code.push_constant(Constant::String(key));
            let assigned = if op == HirAssignOp::Set {
                value
            } else {
                let current = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::GetStringKeyIndex {
                    dst: current,
                    base,
                    key,
                });
                let dst = self.alloc_register()?;
                self.emit(
                    hir_compound_instruction(op, dst, current, value, false)
                        .expect("compound assignment operator"),
                );
                dst
            };
            self.emit(UnlinkedInstructionKind::SetStringKeyIndex {
                base,
                key,
                src: assigned,
            });
            return Ok(assigned);
        }
        let key = self.compile_hir_expression(index.index)?;
        let assigned = if op == HirAssignOp::Set {
            value
        } else {
            let current = self.alloc_register()?;
            self.emit(UnlinkedInstructionKind::GetIndex {
                dst: current,
                base,
                index: key,
            });
            let dst = self.alloc_register()?;
            self.emit(
                hir_compound_instruction(op, dst, current, value, false)
                    .expect("compound assignment operator"),
            );
            dst
        };
        self.emit(UnlinkedInstructionKind::SetIndex {
            base,
            index: key,
            src: assigned,
        });
        Ok(assigned)
    }

    pub(in crate::compiler) fn compile_hir_field_assignment(
        &mut self,
        op: HirAssignOp,
        field: &vela_hir::body::HirField,
        value: Register,
    ) -> CompileResult<Register> {
        if let Some(assigned) = self.compile_hir_nested_field_assignment(op, field, value)? {
            return Ok(assigned);
        }
        let record = self.compile_hir_expression(field.receiver)?;
        let fact = self.script_fact_for_hir_expression(field.receiver);
        let slot = fact
            .as_ref()
            .and_then(|fact| self.script_record_field_slot_for_type(&fact.type_name, &field.name))
            .or_else(|| {
                self.value_shape_for_hir_expression(field.receiver)
                    .and_then(|shape| {
                        shape
                            .as_record()
                            .and_then(|shape| shape.field_slot(&field.name))
                    })
            });
        let assigned = if op == HirAssignOp::Set {
            value
        } else {
            let current = self.alloc_register()?;
            if let Some(slot) = slot {
                self.emit(UnlinkedInstructionKind::GetRecordSlot {
                    dst: current,
                    record,
                    field: field.name.clone(),
                    slot,
                });
            } else {
                self.emit(UnlinkedInstructionKind::GetRecordField {
                    dst: current,
                    record,
                    field: field.name.clone(),
                });
            }
            let dst = self.alloc_register()?;
            self.emit(
                hir_compound_instruction(op, dst, current, value, false)
                    .expect("compound assignment operator"),
            );
            dst
        };
        if let Some(slot) = slot {
            self.emit(UnlinkedInstructionKind::SetRecordSlot {
                record,
                field: field.name.clone(),
                slot,
                src: assigned,
            });
        } else {
            self.emit(UnlinkedInstructionKind::SetRecordField {
                record,
                field: field.name.clone(),
                src: assigned,
            });
        }
        Ok(assigned)
    }

    pub(in crate::compiler) fn compile_hir_nested_field_assignment(
        &mut self,
        op: HirAssignOp,
        target: &vela_hir::body::HirField,
        value: Register,
    ) -> CompileResult<Option<Register>> {
        let mut fields = Vec::new();
        let mut base = target.expression;
        while let HirExprKind::Field(field) = self.hir_expression_record(base)?.1 {
            fields.push(field.name);
            base = field.receiver;
        }
        fields.reverse();
        let base_kind = self.hir_expression_record(base)?.1;
        if fields.len() <= 1 && !matches!(base_kind, HirExprKind::Index(_)) {
            return Ok(None);
        }

        enum IndexedRoot {
            Dynamic {
                collection: Register,
                index: Register,
            },
            String {
                collection: Register,
                key: crate::ConstantId,
            },
        }
        let (root, indexed_root) = match base_kind {
            HirExprKind::Index(index) => {
                let collection = self.compile_hir_expression(index.receiver)?;
                let root = self.alloc_register()?;
                if let HirExprKind::Literal(HirLiteral::String(key)) =
                    self.hir_expression_record(index.index)?.1
                {
                    let key = self.code.push_constant(Constant::String(key));
                    self.emit(UnlinkedInstructionKind::GetStringKeyIndex {
                        dst: root,
                        base: collection,
                        key,
                    });
                    (root, Some(IndexedRoot::String { collection, key }))
                } else {
                    let index = self.compile_hir_expression(index.index)?;
                    self.emit(UnlinkedInstructionKind::GetIndex {
                        dst: root,
                        base: collection,
                        index,
                    });
                    (root, Some(IndexedRoot::Dynamic { collection, index }))
                }
            }
            _ => (self.compile_hir_expression(base)?, None),
        };
        let mut records = vec![root];
        let mut shapes = vec![
            self.value_shape_for_hir_expression(base)
                .and_then(|shape| shape.as_record().cloned()),
        ];
        for field in fields.iter().take(fields.len().saturating_sub(1)) {
            let record = *records.last().expect("nested assignment root");
            let shape = shapes.last().and_then(|shape| shape.as_ref());
            let dst = self.alloc_register()?;
            if let Some(slot) = shape.and_then(|shape| shape.field_slot(field)) {
                self.emit(UnlinkedInstructionKind::GetRecordSlot {
                    dst,
                    record,
                    field: field.clone(),
                    slot,
                });
            } else {
                self.emit(UnlinkedInstructionKind::GetRecordField {
                    dst,
                    record,
                    field: field.clone(),
                });
            }
            shapes.push(
                shape
                    .and_then(|shape| shape.field_record_shape(field))
                    .cloned(),
            );
            records.push(dst);
        }
        let leaf_record = *records.last().expect("nested assignment leaf parent");
        let leaf = fields.last().expect("nested assignment field").clone();
        let leaf_slot = shapes
            .last()
            .and_then(|shape| shape.as_ref())
            .and_then(|shape| shape.field_slot(&leaf));
        let assigned = if op == HirAssignOp::Set {
            value
        } else {
            let current = self.alloc_register()?;
            if let Some(slot) = leaf_slot {
                self.emit(UnlinkedInstructionKind::GetRecordSlot {
                    dst: current,
                    record: leaf_record,
                    field: leaf.clone(),
                    slot,
                });
            } else {
                self.emit(UnlinkedInstructionKind::GetRecordField {
                    dst: current,
                    record: leaf_record,
                    field: leaf.clone(),
                });
            }
            let dst = self.alloc_register()?;
            self.emit(
                hir_compound_instruction(op, dst, current, value, false)
                    .expect("compound assignment operator"),
            );
            dst
        };
        if let Some(slot) = leaf_slot {
            self.emit(UnlinkedInstructionKind::SetRecordSlot {
                record: leaf_record,
                field: leaf,
                slot,
                src: assigned,
            });
        } else {
            self.emit(UnlinkedInstructionKind::SetRecordField {
                record: leaf_record,
                field: leaf,
                src: assigned,
            });
        }
        for index in (0..fields.len().saturating_sub(1)).rev() {
            let field = fields[index].clone();
            let slot = shapes[index]
                .as_ref()
                .and_then(|shape| shape.field_slot(&field));
            if let Some(slot) = slot {
                self.emit(UnlinkedInstructionKind::SetRecordSlot {
                    record: records[index],
                    field,
                    slot,
                    src: records[index + 1],
                });
            } else {
                self.emit(UnlinkedInstructionKind::SetRecordField {
                    record: records[index],
                    field,
                    src: records[index + 1],
                });
            }
        }
        if let Some(indexed_root) = indexed_root {
            match indexed_root {
                IndexedRoot::Dynamic { collection, index } => {
                    self.emit(UnlinkedInstructionKind::SetIndex {
                        base: collection,
                        index,
                        src: root,
                    });
                }
                IndexedRoot::String { collection, key } => {
                    self.emit(UnlinkedInstructionKind::SetStringKeyIndex {
                        base: collection,
                        key,
                        src: root,
                    });
                }
            }
        }
        Ok(Some(assigned))
    }
}
