use super::*;
use crate::compiler::host_paths::CompiledHostTarget;

enum PreparedAssignmentTarget {
    Local {
        expression: HirExprId,
    },
    Index(PreparedIndexAssignment),
    Field(PreparedFieldAssignment),
    Host {
        root: Register,
        target: CompiledHostTarget,
    },
}

struct PreparedIndexAssignment {
    base: Register,
    key: PreparedIndexKey,
}

#[derive(Clone, Copy)]
enum PreparedIndexKey {
    Dynamic(Register),
    String(crate::ConstantId),
}

struct PreparedFieldAssignment {
    fields: Vec<String>,
    slots: Vec<Option<usize>>,
    records: Vec<Register>,
    indexed_root: Option<PreparedIndexedRoot>,
}

enum PreparedIndexedRoot {
    Dynamic {
        collection: Register,
        index: Register,
    },
    String {
        collection: Register,
        key: crate::ConstantId,
    },
}

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
                        .logical_key
                        .ok_or_else(|| hir_unsupported("map key", entry.origin.span))?;
                    let value = entry
                        .value
                        .ok_or_else(|| hir_unsupported("map value", entry.origin.span))?;
                    compiled.push((key, self.compile_hir_expression(value)?));
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
        let expected = if op == HirAssignOp::Set
            && let HirExprKind::Field(field) = self.hir_expression_record(target)?.1
            && let Some(expected) = self
                .script_fact_for_hir_expression(field.receiver)
                .and_then(|fact| self.record_constructor_shape(&fact.type_name))
                .and_then(|shape| shape.field_value_type(&field.name))
        {
            Some((expected, TypeContractContext::Field { name: field.name }))
        } else {
            None
        };
        let target = self.prepare_hir_assignment_target(target, op, span)?;
        let value = if let Some((expected, context)) = expected {
            self.compile_hir_expression_for_expected_type(value, expected, context, &[])?
                .0
        } else {
            self.compile_hir_expression(value)?
        };
        self.finish_hir_assignment(target, op, value, span)
    }

    fn prepare_hir_assignment_target(
        &mut self,
        target: HirExprId,
        op: HirAssignOp,
        span: Span,
    ) -> CompileResult<PreparedAssignmentTarget> {
        if let Some(resolved) = self.hir_host_path(target)
            && !resolved.path.segments.is_empty()
        {
            self.reject_invalid_hir_host_assignment(target, op, span)?;
            let root = self.compile_host_path_root(&resolved.path.root)?;
            let target = self.compile_host_target(resolved.path)?;
            return Ok(PreparedAssignmentTarget::Host { root, target });
        }

        match self.hir_expression_record(target)?.1 {
            HirExprKind::Path(_) => Ok(PreparedAssignmentTarget::Local { expression: target }),
            HirExprKind::Index(index) => self
                .prepare_hir_index_assignment(&index)
                .map(PreparedAssignmentTarget::Index),
            HirExprKind::Field(field) => self
                .prepare_hir_field_assignment(&field)
                .map(PreparedAssignmentTarget::Field),
            _ => Err(hir_unsupported("assignment target", span)),
        }
    }

    fn finish_hir_assignment(
        &mut self,
        target: PreparedAssignmentTarget,
        op: HirAssignOp,
        value: Register,
        span: Span,
    ) -> CompileResult<Register> {
        match target {
            PreparedAssignmentTarget::Local { expression } => {
                self.compile_hir_local_assignment(op, expression, value, span)
            }
            PreparedAssignmentTarget::Index(target) => {
                self.finish_hir_index_assignment(op, target, value)
            }
            PreparedAssignmentTarget::Field(target) => {
                self.finish_hir_field_assignment(op, target, value)
            }
            PreparedAssignmentTarget::Host { root, target } => {
                match op {
                    HirAssignOp::Set => {
                        self.emit_compiled_host_write(root, target, value, span);
                    }
                    _ => self.emit_compiled_host_mutate(
                        root,
                        target,
                        hir_host_mutation_op(op).expect("compound assignment has host mutation op"),
                        value,
                        span,
                    ),
                }
                Ok(value)
            }
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

    fn prepare_hir_index_assignment(
        &mut self,
        index: &vela_hir::body::HirIndex,
    ) -> CompileResult<PreparedIndexAssignment> {
        let base = self.compile_hir_expression(index.receiver)?;
        let base = self.capture_evaluated_value(base)?;
        let key = if let HirExprKind::Literal(HirLiteral::String(key)) =
            self.hir_expression_record(index.index)?.1
        {
            PreparedIndexKey::String(self.code.push_constant(Constant::String(key)))
        } else {
            let key = self.compile_hir_expression(index.index)?;
            PreparedIndexKey::Dynamic(self.capture_evaluated_value(key)?)
        };
        Ok(PreparedIndexAssignment { base, key })
    }

    fn finish_hir_index_assignment(
        &mut self,
        op: HirAssignOp,
        target: PreparedIndexAssignment,
        value: Register,
    ) -> CompileResult<Register> {
        let assigned = if op == HirAssignOp::Set {
            value
        } else {
            let current = self.alloc_register()?;
            match target.key {
                PreparedIndexKey::Dynamic(index) => {
                    self.emit(UnlinkedInstructionKind::GetIndex {
                        dst: current,
                        base: target.base,
                        index,
                    });
                }
                PreparedIndexKey::String(key) => {
                    self.emit(UnlinkedInstructionKind::GetStringKeyIndex {
                        dst: current,
                        base: target.base,
                        key,
                    });
                }
            }
            let dst = self.alloc_register()?;
            self.emit(
                hir_compound_instruction(op, dst, current, value, false)
                    .expect("compound assignment operator"),
            );
            dst
        };
        match target.key {
            PreparedIndexKey::Dynamic(index) => {
                self.emit(UnlinkedInstructionKind::SetIndex {
                    base: target.base,
                    index,
                    src: assigned,
                });
            }
            PreparedIndexKey::String(key) => {
                self.emit(UnlinkedInstructionKind::SetStringKeyIndex {
                    base: target.base,
                    key,
                    src: assigned,
                });
            }
        }
        Ok(assigned)
    }

    fn prepare_hir_field_assignment(
        &mut self,
        target: &vela_hir::body::HirField,
    ) -> CompileResult<PreparedFieldAssignment> {
        let mut fields = Vec::new();
        let mut base = target.expression;
        while let HirExprKind::Field(field) = self.hir_expression_record(base)?.1 {
            fields.push(field.name);
            base = field.receiver;
        }
        fields.reverse();
        let base_kind = self.hir_expression_record(base)?.1;
        let (root, indexed_root) = match base_kind {
            HirExprKind::Index(index) => {
                let collection = self.compile_hir_expression(index.receiver)?;
                let collection = self.capture_evaluated_value(collection)?;
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
                    (root, Some(PreparedIndexedRoot::String { collection, key }))
                } else {
                    let index = self.compile_hir_expression(index.index)?;
                    let index = self.capture_evaluated_value(index)?;
                    self.emit(UnlinkedInstructionKind::GetIndex {
                        dst: root,
                        base: collection,
                        index,
                    });
                    (
                        root,
                        Some(PreparedIndexedRoot::Dynamic { collection, index }),
                    )
                }
            }
            _ => {
                let root = self.compile_hir_expression(base)?;
                (self.capture_evaluated_value(root)?, None)
            }
        };
        let mut records = vec![root];
        let mut shape = self
            .value_shape_for_hir_expression(base)
            .and_then(|shape| shape.as_record().cloned());
        let direct_slot = (fields.len() == 1)
            .then(|| {
                self.script_fact_for_hir_expression(target.receiver)
                    .and_then(|fact| {
                        self.script_record_field_slot_for_type(&fact.type_name, &target.name)
                    })
            })
            .flatten();
        let mut slots = Vec::with_capacity(fields.len());
        for (index, field) in fields.iter().enumerate() {
            let slot =
                direct_slot.or_else(|| shape.as_ref().and_then(|shape| shape.field_slot(field)));
            slots.push(slot);
            if index + 1 == fields.len() {
                break;
            }
            let record = *records.last().expect("nested assignment root");
            let dst = self.alloc_register()?;
            if let Some(slot) = slot {
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
            shape = shape
                .as_ref()
                .and_then(|shape| shape.field_record_shape(field))
                .cloned();
            records.push(dst);
        }

        Ok(PreparedFieldAssignment {
            fields,
            slots,
            records,
            indexed_root,
        })
    }

    fn finish_hir_field_assignment(
        &mut self,
        op: HirAssignOp,
        target: PreparedFieldAssignment,
        value: Register,
    ) -> CompileResult<Register> {
        let leaf_record = *target
            .records
            .last()
            .expect("nested assignment leaf parent");
        let leaf = target
            .fields
            .last()
            .expect("nested assignment field")
            .clone();
        let leaf_slot = *target.slots.last().expect("nested assignment leaf slot");
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
        for index in (0..target.fields.len().saturating_sub(1)).rev() {
            let field = target.fields[index].clone();
            let slot = target.slots[index];
            if let Some(slot) = slot {
                self.emit(UnlinkedInstructionKind::SetRecordSlot {
                    record: target.records[index],
                    field,
                    slot,
                    src: target.records[index + 1],
                });
            } else {
                self.emit(UnlinkedInstructionKind::SetRecordField {
                    record: target.records[index],
                    field,
                    src: target.records[index + 1],
                });
            }
        }
        if let Some(indexed_root) = target.indexed_root {
            match indexed_root {
                PreparedIndexedRoot::Dynamic { collection, index } => {
                    self.emit(UnlinkedInstructionKind::SetIndex {
                        base: collection,
                        index,
                        src: target.records[0],
                    });
                }
                PreparedIndexedRoot::String { collection, key } => {
                    self.emit(UnlinkedInstructionKind::SetStringKeyIndex {
                        base: collection,
                        key,
                        src: target.records[0],
                    });
                }
            }
        }
        Ok(assigned)
    }
}
