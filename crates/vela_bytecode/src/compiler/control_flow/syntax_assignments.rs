use vela_common::{PrimitiveTag, SourceId, Span};
use vela_syntax::ast::{AssignOp, AstNode, Literal, SyntaxExpression};

use crate::compiler::body_payloads::expression_syntax_literal;
use crate::compiler::const_eval::compile_literal_constant_for_type;
use crate::compiler::expected_exprs::guard_location_and_name;
use crate::compiler::operators::{
    compound_assignment_instruction, i64_compound_assignment_instruction,
};
use crate::compiler::record_shapes::RecordShape;
use crate::compiler::value_types::{
    ExpectedTypeOutcome, RuntimeTypeFact, TypeContractContext, check_expected_type,
};
use crate::compiler::{CompileResult, Compiler, type_guard_plan_for_runtime_type};
use crate::{
    GuardKind, Register, UnlinkedGuardContext, UnlinkedInstructionKind, UnlinkedTypeGuard,
};

use super::spans::syntax_expression_span;

impl Compiler<'_, '_> {
    pub(super) fn compile_syntax_assignment(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> CompileResult<Option<Register>> {
        let Some(assign) = expression.as_assign() else {
            return Ok(None);
        };
        let Some(target_expression) = assign.target() else {
            return Ok(None);
        };
        let Some(value_expression) = assign.value() else {
            return Ok(None);
        };
        let Some(op) = assign.operator() else {
            return Ok(None);
        };
        let value_type = self.syntax_value_type_for_expression(Some(source), &value_expression);
        let target_span = syntax_expression_span(source, &target_expression);
        if let Some(target_path) = self.hir_value_path_for_span(target_span)
            && let [target_name] = target_path.as_slice()
        {
            let target_type = self.value_type_for_path(target_span, &target_path);
            let assigned_type = syntax_assignment_value_type(op, target_type, value_type);
            let target =
                self.required_local_register_at_hir_expression_span(target_span, target_name)?;
            let Some(value) = self.compile_syntax_expression(source, &value_expression)? else {
                return Ok(None);
            };
            return self.compile_syntax_local_assignment(op, target, value, assigned_type);
        }
        if let Some(index_target) = target_expression.as_index() {
            let Some(value) = self.compile_syntax_expression(source, &value_expression)? else {
                return Ok(None);
            };
            if let Some(assigned) = self.compile_syntax_host_index_assignment(
                source,
                expression,
                &target_expression,
                op,
                value,
            )? {
                return Ok(Some(assigned));
            }
            return self.compile_syntax_index_assignment(source, op, &index_target, value);
        }
        let Some(value) = self.compile_syntax_expression(source, &value_expression)? else {
            return Ok(None);
        };
        if let Some(assigned) = self.compile_syntax_host_field_assignment(
            source,
            expression,
            &target_expression,
            op,
            value,
        )? {
            return Ok(Some(assigned));
        }
        if let Some(assigned) = self.compile_syntax_record_field_assignment(
            source,
            &target_expression,
            op,
            &value_expression,
            value,
        )? {
            return Ok(Some(assigned));
        }
        Ok(None)
    }

    fn compile_syntax_record_field_assignment(
        &mut self,
        source: SourceId,
        target_expression: &SyntaxExpression,
        op: AssignOp,
        value_expression: &SyntaxExpression,
        value: Register,
    ) -> CompileResult<Option<Register>> {
        let target_span = syntax_expression_span(source, target_expression);
        let Some(field) = self.hir_field_for_span(target_span) else {
            return Ok(None);
        };
        let Some(receiver_span) = self.expression_span(field.receiver) else {
            return Ok(None);
        };
        let field_name = field.name.clone();
        let Some(receiver_expression) =
            syntax_assignment_expression_at_span(source, target_expression, receiver_span)
        else {
            return Ok(None);
        };
        if let Some(target) =
            self.syntax_indexed_record_field_assignment_target(source, target_expression)
        {
            return self.compile_syntax_indexed_record_field_assignment(
                source,
                op,
                target,
                value_expression,
                value,
            );
        }
        if let Some(target) =
            self.syntax_nested_record_field_assignment_target(source, target_expression)
        {
            return self.compile_syntax_nested_record_field_assignment(
                source,
                op,
                target,
                value_expression,
                value,
            );
        }
        let receiver_span = syntax_expression_span(source, &receiver_expression);
        let field_slot = self
            .hir_value_path_for_span(receiver_span)
            .and_then(|path| {
                let [root] = path.as_slice() else {
                    return None;
                };
                self.script_record_field_slot_for_path_root(receiver_span, root, &field_name)
            })
            .or_else(|| {
                self.value_shape_for_syntax_expression(Some(source), &receiver_expression)
                    .and_then(|shape| {
                        shape
                            .as_record()
                            .and_then(|shape| shape.field_slot(&field_name))
                    })
            });
        let value_type = self.syntax_record_field_assignment_value_type(
            source,
            target_expression,
            receiver_span,
        );
        let Some(record) = self.compile_syntax_expression(source, &receiver_expression)? else {
            return Ok(None);
        };
        let assigned = match op {
            AssignOp::Set => self.compile_syntax_record_field_value(
                source,
                value_expression,
                value_type,
                field_name.clone(),
                value,
            )?,
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let current = self.alloc_register()?;
                if let Some(slot) = field_slot {
                    self.emit(UnlinkedInstructionKind::GetRecordSlot {
                        dst: current,
                        record,
                        field: field_name.clone(),
                        slot,
                    });
                } else {
                    self.emit(UnlinkedInstructionKind::GetRecordField {
                        dst: current,
                        record,
                        field: field_name.clone(),
                    });
                }
                let dst = self.alloc_register()?;
                let instruction = compound_assignment_instruction(op, dst, current, value)
                    .ok_or_else(|| {
                        crate::compiler::CompileError::new(
                            crate::compiler::CompileErrorKind::UnsupportedSyntax(
                                "compound assignment",
                            ),
                        )
                    })?;
                self.emit(instruction);
                dst
            }
        };
        if let Some(slot) = field_slot {
            self.emit(UnlinkedInstructionKind::SetRecordSlot {
                record,
                field: field_name,
                slot,
                src: assigned,
            });
        } else {
            self.emit(UnlinkedInstructionKind::SetRecordField {
                record,
                field: field_name,
                src: assigned,
            });
        }
        Ok(Some(assigned))
    }

    fn syntax_record_field_assignment_value_type(
        &self,
        source: SourceId,
        target_expression: &SyntaxExpression,
        receiver_span: Span,
    ) -> Option<RuntimeTypeFact> {
        let target_span = syntax_expression_span(source, target_expression);
        let path = self.hir_value_path_for_span(target_span)?;
        let (root, fields) = path.split_first()?;
        let root_span = self
            .hir_value_path_for_span(receiver_span)
            .filter(|receiver| receiver.as_slice() == [root.as_str()])
            .map_or(target_span, |_| receiver_span);
        let root_type = self.script_type_for_path_root(root_span, root)?;
        self.schema_record_field_value_type(Some(root_type.as_str()), fields)
    }

    fn syntax_nested_record_field_assignment_target(
        &self,
        source: SourceId,
        target_expression: &SyntaxExpression,
    ) -> Option<SyntaxNestedRecordFieldAssignmentTarget> {
        let target_span = syntax_expression_span(source, target_expression);
        let path = self.hir_value_path_for_span(target_span)?;
        if path.len() <= 2 {
            return None;
        }
        let root = path.first()?.clone();
        let fields = path[1..].to_vec();
        let root_span = self
            .hir_value_path_root_span_for_span(target_span)
            .unwrap_or(target_span);
        let root_type = self.script_type_for_path_root(root_span, &root);
        let shape = self
            .record_shape_for_path_root(root_span, &root)
            .or_else(|| {
                root_type
                    .as_deref()
                    .and_then(|type_name| self.record_shape_for_type(type_name))
            });
        let value_type = self.schema_record_field_value_type(root_type.as_deref(), &fields);
        Some(SyntaxNestedRecordFieldAssignmentTarget {
            root,
            root_span,
            fields,
            shape,
            value_type,
        })
    }

    fn syntax_indexed_record_field_assignment_target(
        &self,
        source: SourceId,
        target_expression: &SyntaxExpression,
    ) -> Option<SyntaxIndexedRecordFieldAssignmentTarget> {
        let (collection, index, fields) =
            self.syntax_indexed_record_field_parts(source, target_expression.clone())?;
        let element_shape = self
            .value_shape_for_syntax_expression(Some(source), &collection)?
            .array_element_record()
            .cloned();
        Some(SyntaxIndexedRecordFieldAssignmentTarget {
            collection,
            index,
            fields,
            element_shape,
        })
    }

    fn compile_syntax_indexed_record_field_assignment(
        &mut self,
        source: SourceId,
        op: AssignOp,
        target: SyntaxIndexedRecordFieldAssignmentTarget,
        value_expression: &SyntaxExpression,
        value: Register,
    ) -> CompileResult<Option<Register>> {
        let Some(collection) = self.compile_syntax_expression(source, &target.collection)? else {
            return Ok(None);
        };
        let Some(index) = self.compile_syntax_expression(source, &target.index)? else {
            return Ok(None);
        };
        let record = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::GetIndex {
            dst: record,
            base: collection,
            index,
        });
        let assigned = self.compile_syntax_nested_record_field_assignment_at_root(
            source,
            op,
            SyntaxNestedRecordFieldAssignmentRoot {
                root: record,
                fields: target.fields,
                shape: target.element_shape,
                value_type: None,
            },
            value_expression,
            value,
        )?;
        self.emit(UnlinkedInstructionKind::SetIndex {
            base: collection,
            index,
            src: record,
        });
        Ok(Some(assigned))
    }

    fn compile_syntax_nested_record_field_assignment(
        &mut self,
        source: SourceId,
        op: AssignOp,
        target: SyntaxNestedRecordFieldAssignmentTarget,
        value_expression: &SyntaxExpression,
        value: Register,
    ) -> CompileResult<Option<Register>> {
        let root =
            self.required_local_register_at_hir_expression_span(target.root_span, &target.root)?;
        let assigned = self.compile_syntax_nested_record_field_assignment_at_root(
            source,
            op,
            SyntaxNestedRecordFieldAssignmentRoot {
                root,
                fields: target.fields,
                shape: target.shape,
                value_type: target.value_type,
            },
            value_expression,
            value,
        )?;
        Ok(Some(assigned))
    }

    fn compile_syntax_nested_record_field_assignment_at_root(
        &mut self,
        source: SourceId,
        op: AssignOp,
        target: SyntaxNestedRecordFieldAssignmentRoot,
        value_expression: &SyntaxExpression,
        value: Register,
    ) -> CompileResult<Register> {
        let SyntaxNestedRecordFieldAssignmentRoot {
            root,
            fields,
            shape,
            value_type,
        } = target;
        let mut records = vec![root];
        let mut shapes = vec![shape];
        for field in fields.iter().take(fields.len().saturating_sub(1)) {
            let dst = self.alloc_register()?;
            let record = *records
                .last()
                .expect("nested record assignment always has root");
            let shape = shapes.last().and_then(|shape| shape.as_ref());
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

        let leaf_record = *records
            .last()
            .expect("nested record assignment always has leaf parent");
        let leaf_field = fields
            .last()
            .expect("nested record assignment has at least one field")
            .clone();
        let assigned = match op {
            AssignOp::Set => self.compile_syntax_record_field_value(
                source,
                value_expression,
                value_type,
                leaf_field.clone(),
                value,
            )?,
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let current = self.alloc_register()?;
                let leaf_slot = shapes
                    .last()
                    .and_then(|shape| shape.as_ref())
                    .and_then(|shape| shape.field_slot(&leaf_field));
                if let Some(slot) = leaf_slot {
                    self.emit(UnlinkedInstructionKind::GetRecordSlot {
                        dst: current,
                        record: leaf_record,
                        field: leaf_field.clone(),
                        slot,
                    });
                } else {
                    self.emit(UnlinkedInstructionKind::GetRecordField {
                        dst: current,
                        record: leaf_record,
                        field: leaf_field.clone(),
                    });
                }
                let dst = self.alloc_register()?;
                let instruction = compound_assignment_instruction(op, dst, current, value)
                    .ok_or_else(|| {
                        crate::compiler::CompileError::new(
                            crate::compiler::CompileErrorKind::UnsupportedSyntax(
                                "compound assignment",
                            ),
                        )
                    })?;
                self.emit(instruction);
                dst
            }
        };

        let leaf_slot = shapes
            .last()
            .and_then(|shape| shape.as_ref())
            .and_then(|shape| shape.field_slot(&leaf_field));
        if let Some(slot) = leaf_slot {
            self.emit(UnlinkedInstructionKind::SetRecordSlot {
                record: leaf_record,
                field: leaf_field,
                slot,
                src: assigned,
            });
        } else {
            self.emit(UnlinkedInstructionKind::SetRecordField {
                record: leaf_record,
                field: leaf_field,
                src: assigned,
            });
        }
        for (index, field) in fields
            .iter()
            .take(fields.len().saturating_sub(1))
            .enumerate()
            .rev()
        {
            let slot = shapes[index]
                .as_ref()
                .and_then(|shape| shape.field_slot(field));
            if let Some(slot) = slot {
                self.emit(UnlinkedInstructionKind::SetRecordSlot {
                    record: records[index],
                    field: field.clone(),
                    slot,
                    src: records[index + 1],
                });
            } else {
                self.emit(UnlinkedInstructionKind::SetRecordField {
                    record: records[index],
                    field: field.clone(),
                    src: records[index + 1],
                });
            }
        }
        Ok(assigned)
    }

    fn compile_syntax_record_field_value(
        &mut self,
        source: SourceId,
        expression: &SyntaxExpression,
        expected: Option<RuntimeTypeFact>,
        field_name: String,
        value: Register,
    ) -> CompileResult<Register> {
        let Some(expected) = expected else {
            return Ok(value);
        };
        let span = syntax_expression_span(source, expression);
        let context = TypeContractContext::Field { name: field_name };
        let static_type = self.syntax_static_type_for_expression(Some(source), expression);
        let outcome = check_expected_type(static_type, expected, span, context.clone())?;
        if let ExpectedTypeOutcome::Contextualized(RuntimeTypeFact::Primitive(tag)) = &outcome
            && let Some(literal) = expression_syntax_literal(expression)
            && let Some(constant) = compile_literal_constant_for_type(&literal, *tag)
                .map_err(|error| error.with_span(span))?
        {
            return self.emit_constant(constant);
        }
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(expected) = &outcome
            && let Some((location, name)) = guard_location_and_name(context)
            && let Some(plan) = type_guard_plan_for_runtime_type(expected)
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

    fn compile_syntax_local_assignment(
        &mut self,
        op: AssignOp,
        target: Register,
        value: Register,
        assigned_type: Option<RuntimeTypeFact>,
    ) -> CompileResult<Option<Register>> {
        let assigned = match op {
            AssignOp::Set => {
                self.emit(UnlinkedInstructionKind::Move {
                    dst: target,
                    src: value,
                });
                value
            }
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let dst = self.alloc_register()?;
                let instruction = if assigned_type
                    == Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
                {
                    i64_compound_assignment_instruction(op, dst, target, value)
                } else {
                    None
                }
                .or_else(|| compound_assignment_instruction(op, dst, target, value))
                .ok_or_else(|| {
                    crate::compiler::CompileError::new(
                        crate::compiler::CompileErrorKind::UnsupportedSyntax("compound assignment"),
                    )
                })?;
                self.emit(instruction);
                self.emit(UnlinkedInstructionKind::Move {
                    dst: target,
                    src: dst,
                });
                dst
            }
        };
        Ok(Some(assigned))
    }
    fn compile_syntax_index_assignment(
        &mut self,
        source: SourceId,
        op: AssignOp,
        target: &vela_syntax::ast::SyntaxIndexExpr,
        value: Register,
    ) -> CompileResult<Option<Register>> {
        let Some(receiver_expression) = target.receiver() else {
            return Ok(None);
        };
        let Some(index_expression) = target.index() else {
            return Ok(None);
        };
        let Some(base) = self.compile_syntax_expression(source, &receiver_expression)? else {
            return Ok(None);
        };
        if let Some(Literal::String(key)) = expression_syntax_literal(&index_expression) {
            return self.compile_syntax_string_key_index_assignment(op, base, key, value);
        }
        let Some(index) = self.compile_syntax_expression(source, &index_expression)? else {
            return Ok(None);
        };
        let assigned = match op {
            AssignOp::Set => value,
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let current = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::GetIndex {
                    dst: current,
                    base,
                    index,
                });
                let dst = self.alloc_register()?;
                let instruction = compound_assignment_instruction(op, dst, current, value)
                    .ok_or_else(|| {
                        crate::compiler::CompileError::new(
                            crate::compiler::CompileErrorKind::UnsupportedSyntax(
                                "compound assignment",
                            ),
                        )
                    })?;
                self.emit(instruction);
                dst
            }
        };
        self.emit(UnlinkedInstructionKind::SetIndex {
            base,
            index,
            src: assigned,
        });
        Ok(Some(assigned))
    }

    fn compile_syntax_string_key_index_assignment(
        &mut self,
        op: AssignOp,
        base: Register,
        key: String,
        value: Register,
    ) -> CompileResult<Option<Register>> {
        let key = self.code.push_constant(crate::Constant::String(key));
        let assigned = match op {
            AssignOp::Set => value,
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let current = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::GetStringKeyIndex {
                    dst: current,
                    base,
                    key,
                });
                let dst = self.alloc_register()?;
                let instruction = compound_assignment_instruction(op, dst, current, value)
                    .ok_or_else(|| {
                        crate::compiler::CompileError::new(
                            crate::compiler::CompileErrorKind::UnsupportedSyntax(
                                "compound assignment",
                            ),
                        )
                    })?;
                self.emit(instruction);
                dst
            }
        };
        self.emit(UnlinkedInstructionKind::SetStringKeyIndex {
            base,
            key,
            src: assigned,
        });
        Ok(Some(assigned))
    }

    fn syntax_indexed_record_field_parts(
        &self,
        source: SourceId,
        expression: SyntaxExpression,
    ) -> Option<(SyntaxExpression, SyntaxExpression, Vec<String>)> {
        let field = self.hir_field_for_span(syntax_expression_span(source, &expression))?;
        let receiver_span = self.expression_span(field.receiver)?;
        let receiver = syntax_assignment_expression_at_span(source, &expression, receiver_span)?;
        let field_name = field.name.clone();
        if let Some(index) = receiver.as_index() {
            let collection = index.receiver()?;
            let index = index.index()?;
            return Some((collection, index, vec![field_name]));
        }
        let (collection, index, mut fields) =
            self.syntax_indexed_record_field_parts(source, receiver)?;
        fields.push(field_name);
        Some((collection, index, fields))
    }
}

fn syntax_assignment_expression_at_span(
    source: SourceId,
    expression: &SyntaxExpression,
    span: Span,
) -> Option<SyntaxExpression> {
    if span.source != source {
        return None;
    }
    expression
        .syntax()
        .descendants()
        .filter_map(SyntaxExpression::cast)
        .find(|child| syntax_expression_span(source, child) == span)
}

fn syntax_assignment_value_type(
    op: AssignOp,
    target_type: Option<RuntimeTypeFact>,
    value_type: Option<RuntimeTypeFact>,
) -> Option<RuntimeTypeFact> {
    match op {
        AssignOp::Set => value_type,
        AssignOp::Add | AssignOp::Sub | AssignOp::Mul
            if target_type == Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
                && value_type == Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64)) =>
        {
            Some(RuntimeTypeFact::Primitive(PrimitiveTag::I64))
        }
        AssignOp::Div | AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Rem => None,
    }
}

struct SyntaxNestedRecordFieldAssignmentTarget {
    root: String,
    root_span: Span,
    fields: Vec<String>,
    shape: Option<RecordShape>,
    value_type: Option<RuntimeTypeFact>,
}

struct SyntaxIndexedRecordFieldAssignmentTarget {
    collection: SyntaxExpression,
    index: SyntaxExpression,
    fields: Vec<String>,
    element_shape: Option<RecordShape>,
}

struct SyntaxNestedRecordFieldAssignmentRoot {
    root: Register,
    fields: Vec<String>,
    shape: Option<RecordShape>,
    value_type: Option<RuntimeTypeFact>,
}
