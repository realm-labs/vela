use vela_common::{Diagnostic, Span};
use vela_hir::binding::BindingResolution;
use vela_hir::ids::HirLocalId;
use vela_host::resolved::HostMutationOp;
use vela_syntax::ast::{AssignOp, Expr, ExprKind, SyntaxExpressionKind};

use crate::{Register, UnlinkedInstructionKind};

mod helpers;

use super::assignment_payloads::{
    validate_assignment_target_payload, validate_assignment_value_payload,
};
use super::body_payloads::CompilerExpressionPayload;
use super::expression_checks::payload_syntax_overlaps_expr;
#[cfg(not(test))]
use super::expression_facts::ExpressionFacts;
#[cfg(test)]
use super::expression_facts::expression_facts;
use super::expressions::literal_string_with_payload;
use super::host_paths::{HostIndexAccessKind, HostPath};
use super::operators::i64_compound_assignment_instruction;
use super::record_shapes::RecordShape;
use super::script_types::ScriptTypeFact;
use super::value_types::{RuntimeTypeFact, TypeContractContext};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};
use helpers::{
    compound_assignment_instruction_or_error, expressions_are_i64,
    indexed_record_field_parts_with_payload, record_field_expr_parts_with_payload,
    record_path_parts,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalAssignmentTarget {
    target_span: Span,
    name: String,
    local: Option<HirLocalId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordFieldAssignmentTarget {
    root: Register,
    fields: Vec<String>,
    shape: Option<RecordShape>,
    slot: Option<usize>,
    value_type: Option<RuntimeTypeFact>,
}

struct IndexedRecordFieldAssignmentTarget<'expr> {
    collection: &'expr Expr,
    index: &'expr Expr,
    collection_payload: Option<CompilerExpressionPayload<'expr>>,
    index_payload: Option<CompilerExpressionPayload<'expr>>,
    fields: Vec<String>,
    element_shape: Option<RecordShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordFieldExprParts<'expr> {
    root: &'expr Expr,
    fields: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalAssignmentFacts {
    script: Option<ScriptTypeFact>,
    value_type: Option<RuntimeTypeFact>,
    value_shape: Option<super::record_shapes::ValueShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NestedRecordFieldAssignmentTarget {
    root: Register,
    fields: Vec<String>,
    shape: Option<RecordShape>,
    value_type: Option<RuntimeTypeFact>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordFieldAssignmentRoot<'field> {
    root: Register,
    field: &'field str,
    slot: Option<usize>,
    value_type: Option<RuntimeTypeFact>,
}

#[derive(Clone, Copy)]
pub(in crate::compiler) struct AssignmentValueSyntax<'payload, 'ast> {
    kind: Option<SyntaxExpressionKind>,
    op: Option<AssignOp>,
    expression: Option<&'payload CompilerExpressionPayload<'ast>>,
}

impl<'payload, 'ast> AssignmentValueSyntax<'payload, 'ast> {
    #[cfg(test)]
    pub(in crate::compiler) fn new(
        kind: Option<SyntaxExpressionKind>,
        op: Option<AssignOp>,
        expression: Option<&'payload CompilerExpressionPayload<'ast>>,
    ) -> Self {
        Self {
            kind,
            op,
            expression,
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::compiler) struct AssignmentTargetSyntax<'payload, 'ast> {
    expression: Option<&'payload CompilerExpressionPayload<'ast>>,
}

impl<'payload, 'ast> AssignmentTargetSyntax<'payload, 'ast> {
    #[cfg(test)]
    pub(in crate::compiler) fn new(
        expression: Option<&'payload CompilerExpressionPayload<'ast>>,
    ) -> Self {
        Self { expression }
    }

    fn field_base_payload(&self) -> Option<CompilerExpressionPayload<'ast>> {
        self.expression
            .and_then(CompilerExpressionPayload::field_base_payload)
    }

    fn index_operand_payloads(
        &self,
    ) -> Option<(
        CompilerExpressionPayload<'ast>,
        CompilerExpressionPayload<'ast>,
    )> {
        self.expression
            .and_then(CompilerExpressionPayload::index_operand_payloads)
    }

    fn indexed_record_operand_payloads(
        &self,
    ) -> Option<(
        CompilerExpressionPayload<'ast>,
        CompilerExpressionPayload<'ast>,
    )> {
        let mut payload = self.field_base_payload()?;
        loop {
            if let Some(operands) = payload.index_operand_payloads() {
                return Some(operands);
            }
            payload = payload.field_base_payload()?;
        }
    }

    fn record_field_root_payload(&self) -> Option<CompilerExpressionPayload<'ast>> {
        let payload = self.field_base_payload()?;
        record_field_root_payload(payload)
    }
}

fn record_field_root_payload<'ast>(
    payload: CompilerExpressionPayload<'ast>,
) -> Option<CompilerExpressionPayload<'ast>> {
    match payload.syntax_kind() {
        Some(SyntaxExpressionKind::Field) => {
            record_field_root_payload(payload.field_base_payload()?)
        }
        Some(_) => Some(payload),
        None => Some(payload),
    }
}

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_assignment_with_payloads(
        &mut self,
        expr: &Expr,
        target_syntax: AssignmentTargetSyntax<'_, '_>,
        value_syntax: AssignmentValueSyntax<'_, '_>,
    ) -> CompileResult<Register> {
        let ExprKind::Assign { op, target, value } = &expr.kind else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "assignment statement",
            )));
        };
        let op = value_syntax.op.unwrap_or(*op);
        #[cfg(test)]
        let target_facts = expression_facts(target);
        #[cfg(not(test))]
        let target_facts = ExpressionFacts::span_only(target.span);
        validate_assignment_target_payload(target_facts, target_syntax.expression)?;
        if value_syntax.expression.is_some() && value_syntax.kind.is_none() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST assignment value",
            )));
        }
        validate_assignment_value_payload(value.span, value_syntax.expression)?;
        if let Some(local_target) = self.local_assignment_target(target_syntax.expression) {
            let target_value_type =
                self.value_type_for_expression_payload(target_syntax.expression);
            let assigned_value_type = match op {
                AssignOp::Set => self.value_type_for_expression_payload(value_syntax.expression),
                AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Rem
                    if expressions_are_i64(
                        target_value_type.clone(),
                        self.value_type_for_expression_payload(value_syntax.expression),
                    ) =>
                {
                    Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64))
                }
                AssignOp::Div => None,
                AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Rem => None,
            };
            let script_fact = (op == AssignOp::Set)
                .then(|| self.script_fact_for_expression_payload(value_syntax.expression))
                .flatten();
            let value_shape = (op == AssignOp::Set)
                .then(|| self.value_shape_for_expression_payload(value_syntax.expression))
                .flatten();
            let facts = LocalAssignmentFacts {
                script: script_fact,
                value_type: assigned_value_type,
                value_shape,
            };
            let assigned =
                self.compile_local_assignment(op, local_target, value, facts, value_syntax)?;
            return Ok(assigned);
        }
        self.reject_read_only_host_assignment(target, target_syntax)?;
        if let ExprKind::Index { base, index } = &target.kind {
            let operand_payloads = target_syntax.index_operand_payloads();
            let base_payload = operand_payloads.as_ref().map(|(base, _)| base);
            let index_payload = operand_payloads.as_ref().map(|(_, index)| index);
            let access = match op {
                AssignOp::Set => HostIndexAccessKind::Write,
                AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                    HostIndexAccessKind::Mutate
                }
            };
            self.reject_invalid_host_index_access_with_payload(
                target,
                base,
                index,
                access,
                base_payload,
                index_payload,
            )?;
            if self.host_field_path(target).is_none() {
                return self.compile_index_assignment(
                    op,
                    base,
                    index,
                    value,
                    target_syntax,
                    value_syntax,
                );
            }
        }
        if let Some(target) = self.indexed_record_field_assignment_target(target, target_syntax) {
            return self.compile_indexed_record_field_assignment(op, target, value, value_syntax);
        }
        if let Some(target) = self.record_field_assignment_target(target, target_syntax)? {
            return self.compile_record_field_assignment(op, target, value, value_syntax);
        }
        self.compile_host_assignment(op, target, value, target_syntax, value_syntax)
    }

    fn local_assignment_target(
        &self,
        target_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> Option<LocalAssignmentTarget> {
        let payload = target_payload?;
        if !matches!(
            payload.syntax_kind(),
            Some(SyntaxExpressionKind::Path) | None
        ) {
            return None;
        }
        let path = payload.syntax_path_segments()?;
        let [name] = path.as_slice() else {
            return None;
        };
        let target_span = payload.syntax_span()?;
        let name = name.clone();
        let local = match self.bindings.resolution_at_span(target_span) {
            Some(BindingResolution::Local(local)) => Some(*local),
            _ if self.locals.contains_key(&name) => None,
            _ => return None,
        };
        Some(LocalAssignmentTarget {
            target_span,
            name,
            local,
        })
    }

    fn compile_local_assignment(
        &mut self,
        op: AssignOp,
        local_target: LocalAssignmentTarget,
        value: &Expr,
        facts: LocalAssignmentFacts,
        value_syntax: AssignmentValueSyntax<'_, '_>,
    ) -> CompileResult<Register> {
        let LocalAssignmentTarget {
            target_span,
            name,
            local,
        } = local_target;
        let target = self.local_register_at_span(target_span, &name)?;
        if let Some(local) = local {
            self.hir_locals.insert(local, target);
            self.script_types
                .set_local_fact(local, name.clone(), facts.script);
            self.value_types
                .set_local(local, name.clone(), facts.value_type.clone());
            self.value_shapes
                .set_local(local, name.clone(), facts.value_shape);
        } else {
            self.script_types.set_name_fact(name.clone(), facts.script);
            self.value_types
                .set_name(name.clone(), facts.value_type.clone());
            self.value_shapes.set_name(name.clone(), facts.value_shape);
        }
        let assigned = match op {
            AssignOp::Set => {
                let src = self.compile_assignment_value(value, None, value_syntax)?;
                self.emit(UnlinkedInstructionKind::Move { dst: target, src });
                src
            }
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let rhs = self.compile_assignment_value(value, None, value_syntax)?;
                let dst = self.alloc_register()?;
                let instruction = if facts.value_type
                    == Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64))
                {
                    i64_compound_assignment_instruction(op, dst, target, rhs)
                } else {
                    None
                }
                .unwrap_or(compound_assignment_instruction_or_error(
                    op, dst, target, rhs,
                )?);
                self.emit(instruction);
                self.emit(UnlinkedInstructionKind::Move {
                    dst: target,
                    src: dst,
                });
                dst
            }
        };
        Ok(assigned)
    }

    fn compile_index_assignment(
        &mut self,
        op: AssignOp,
        base: &Expr,
        index: &Expr,
        value: &Expr,
        target_syntax: AssignmentTargetSyntax<'_, '_>,
        value_syntax: AssignmentValueSyntax<'_, '_>,
    ) -> CompileResult<Register> {
        let operand_payloads = target_syntax.index_operand_payloads();
        let (base_payload, index_payload) = operand_payloads
            .as_ref()
            .map_or((None, None), |(base, index)| (Some(base), Some(index)));
        let base = self.compile_expr_with_payload(base, base_payload)?;
        if let Some(key) = literal_string_with_payload(index_payload) {
            return self.compile_string_key_index_assignment(op, base, &key, value, value_syntax);
        }
        let index = self.compile_expr_with_payload(index, index_payload)?;
        let assigned = match op {
            AssignOp::Set => self.compile_assignment_value(value, None, value_syntax)?,
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let current = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::GetIndex {
                    dst: current,
                    base,
                    index,
                });
                let rhs = self.compile_assignment_value(value, None, value_syntax)?;
                let dst = self.alloc_register()?;
                self.emit(compound_assignment_instruction_or_error(
                    op, dst, current, rhs,
                )?);
                dst
            }
        };
        self.emit(UnlinkedInstructionKind::SetIndex {
            base,
            index,
            src: assigned,
        });
        Ok(assigned)
    }

    fn compile_string_key_index_assignment(
        &mut self,
        op: AssignOp,
        base: Register,
        key: &str,
        value: &Expr,
        value_syntax: AssignmentValueSyntax<'_, '_>,
    ) -> CompileResult<Register> {
        let key = self
            .code
            .push_constant(crate::Constant::String(key.to_owned()));
        let assigned = match op {
            AssignOp::Set => self.compile_assignment_value(value, None, value_syntax)?,
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let current = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::GetStringKeyIndex {
                    dst: current,
                    base,
                    key,
                });
                let rhs = self.compile_assignment_value(value, None, value_syntax)?;
                let dst = self.alloc_register()?;
                self.emit(compound_assignment_instruction_or_error(
                    op, dst, current, rhs,
                )?);
                dst
            }
        };
        self.emit(UnlinkedInstructionKind::SetStringKeyIndex {
            base,
            key,
            src: assigned,
        });
        Ok(assigned)
    }

    fn indexed_record_field_assignment_target<'expr>(
        &self,
        target: &'expr Expr,
        syntax: AssignmentTargetSyntax<'_, 'expr>,
    ) -> Option<IndexedRecordFieldAssignmentTarget<'expr>> {
        if self.host_field_path(target).is_some() {
            return None;
        }
        let (collection, index, fields) =
            indexed_record_field_parts_with_payload(target, syntax.expression.cloned())?;
        let operand_payloads = syntax.indexed_record_operand_payloads();
        let (collection_payload, index_payload) =
            operand_payloads.map_or((None, None), |payloads| {
                let (collection, index) = payloads;
                (Some(collection), Some(index))
            });
        let element_shape = self.record_shape_for_index_collection(collection_payload.as_ref());
        Some(IndexedRecordFieldAssignmentTarget {
            collection,
            index,
            collection_payload,
            index_payload,
            fields,
            element_shape,
        })
    }

    fn record_field_assignment_target(
        &mut self,
        target: &Expr,
        syntax: AssignmentTargetSyntax<'_, '_>,
    ) -> CompileResult<Option<RecordFieldAssignmentTarget>> {
        match &target.kind {
            ExprKind::Path(path) => {
                let path = if let Some(payload) = syntax.expression {
                    payload.syntax_path_segments().ok_or_else(|| {
                        CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "missing CST assignment target path",
                        ))
                    })?
                } else {
                    path.to_owned()
                };
                let Some((record, fields)) = record_path_parts(&path) else {
                    return Ok(None);
                };
                if self.host_field_path(target).is_some() {
                    return Ok(None);
                }
                let root_type = self.script_type_for_path_root(target.span, record);
                let shape = self
                    .record_shape_for_path_root(target.span, record)
                    .or_else(|| {
                        root_type
                            .as_deref()
                            .and_then(|type_name| self.record_shape_for_type(type_name))
                    });
                let slot = match fields.as_slice() {
                    [field] => self
                        .script_record_field_slot_for_path_root(target.span, record, field.as_str())
                        .or_else(|| {
                            self.record_shape_for_path_root(target.span, record)?
                                .field_slot(field)
                        }),
                    _ => None,
                };
                let value_type = self.schema_record_field_value_type(root_type.as_deref(), &fields);
                Ok(Some(RecordFieldAssignmentTarget {
                    root: self.local_register_at_span(target.span, record)?,
                    fields,
                    shape,
                    slot,
                    value_type,
                }))
            }
            ExprKind::Field { base: _, name } => {
                if self.host_field_path(target).is_some() {
                    return Ok(None);
                }
                let Some(parts) =
                    record_field_expr_parts_with_payload(target, syntax.expression.cloned())
                else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "record field assignment target",
                    )));
                };
                let root_payload = syntax.record_field_root_payload();
                let root_type = self.script_type_for_expression_payload(root_payload.as_ref());
                let shape = root_type
                    .as_deref()
                    .and_then(|type_name| self.record_shape_for_type(type_name))
                    .or_else(|| self.record_shape_for_expression_payload(root_payload.as_ref()));
                let slot = (parts.fields.len() == 1)
                    .then(|| {
                        let field = parts.fields.first().map_or(name.as_str(), String::as_str);
                        root_type
                            .as_deref()
                            .and_then(|type_name| {
                                self.script_record_field_slot_for_type(type_name, field)
                            })
                            .or_else(|| shape.as_ref().and_then(|shape| shape.field_slot(field)))
                    })
                    .flatten();
                let value_type =
                    self.schema_record_field_value_type(root_type.as_deref(), &parts.fields);
                let root = self.compile_expr_with_payload(parts.root, root_payload.as_ref())?;
                Ok(Some(RecordFieldAssignmentTarget {
                    root,
                    fields: parts.fields,
                    shape,
                    slot,
                    value_type,
                }))
            }
            _ => Ok(None),
        }
    }

    fn compile_record_field_assignment(
        &mut self,
        op: AssignOp,
        target: RecordFieldAssignmentTarget,
        value: &Expr,
        value_syntax: AssignmentValueSyntax<'_, '_>,
    ) -> CompileResult<Register> {
        if target.fields.len() > 1 {
            return self.compile_nested_record_field_assignment(
                op,
                NestedRecordFieldAssignmentTarget {
                    root: target.root,
                    fields: target.fields,
                    shape: target.shape,
                    value_type: target.value_type,
                },
                value,
                value_syntax,
            );
        }
        let [field] = target.fields.as_slice() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "record field assignment target",
            )));
        };
        self.compile_record_field_assignment_at_root(
            op,
            RecordFieldAssignmentRoot {
                root: target.root,
                field,
                slot: target.slot,
                value_type: target.value_type,
            },
            value,
            value_syntax,
        )
    }

    fn compile_record_field_assignment_at_root(
        &mut self,
        op: AssignOp,
        target: RecordFieldAssignmentRoot<'_>,
        value: &Expr,
        value_syntax: AssignmentValueSyntax<'_, '_>,
    ) -> CompileResult<Register> {
        let RecordFieldAssignmentRoot {
            root,
            field,
            slot,
            value_type,
        } = target;
        let assigned = match op {
            AssignOp::Set => self.compile_assignment_value(
                value,
                value_type.map(|expected| {
                    (
                        expected,
                        TypeContractContext::Field {
                            name: field.to_owned(),
                        },
                    )
                }),
                value_syntax,
            )?,
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let current = self.alloc_register()?;
                if let Some(slot) = slot {
                    self.emit(UnlinkedInstructionKind::GetRecordSlot {
                        dst: current,
                        record: root,
                        field: field.to_owned(),
                        slot,
                    });
                } else {
                    self.emit(UnlinkedInstructionKind::GetRecordField {
                        dst: current,
                        record: root,
                        field: field.to_owned(),
                    });
                }
                let rhs = self.compile_assignment_value(value, None, value_syntax)?;
                let dst = self.alloc_register()?;
                self.emit(compound_assignment_instruction_or_error(
                    op, dst, current, rhs,
                )?);
                dst
            }
        };
        if let Some(slot) = slot {
            self.emit(UnlinkedInstructionKind::SetRecordSlot {
                record: root,
                field: field.to_owned(),
                slot,
                src: assigned,
            });
        } else {
            self.emit(UnlinkedInstructionKind::SetRecordField {
                record: root,
                field: field.to_owned(),
                src: assigned,
            });
        }
        Ok(assigned)
    }

    fn compile_indexed_record_field_assignment(
        &mut self,
        op: AssignOp,
        target: IndexedRecordFieldAssignmentTarget<'_>,
        value: &Expr,
        value_syntax: AssignmentValueSyntax<'_, '_>,
    ) -> CompileResult<Register> {
        let collection =
            self.compile_expr_with_payload(target.collection, target.collection_payload.as_ref())?;
        let index = self.compile_expr_with_payload(target.index, target.index_payload.as_ref())?;
        let record = self.alloc_register()?;
        self.emit(UnlinkedInstructionKind::GetIndex {
            dst: record,
            base: collection,
            index,
        });

        let assigned = if target.fields.len() > 1 {
            self.compile_nested_record_field_assignment(
                op,
                NestedRecordFieldAssignmentTarget {
                    root: record,
                    fields: target.fields,
                    shape: target.element_shape,
                    value_type: None,
                },
                value,
                value_syntax,
            )?
        } else {
            let [field] = target.fields.as_slice() else {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "record field assignment target",
                )));
            };
            let slot = target
                .element_shape
                .as_ref()
                .and_then(|shape| shape.field_slot(field));
            self.compile_record_field_assignment_at_root(
                op,
                RecordFieldAssignmentRoot {
                    root: record,
                    field,
                    slot,
                    value_type: None,
                },
                value,
                value_syntax,
            )?
        };

        self.emit(UnlinkedInstructionKind::SetIndex {
            base: collection,
            index,
            src: record,
        });
        Ok(assigned)
    }

    fn compile_nested_record_field_assignment(
        &mut self,
        op: AssignOp,
        target: NestedRecordFieldAssignmentTarget,
        value: &Expr,
        value_syntax: AssignmentValueSyntax<'_, '_>,
    ) -> CompileResult<Register> {
        let NestedRecordFieldAssignmentTarget {
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
            AssignOp::Set => self.compile_assignment_value(
                value,
                value_type.map(|expected| {
                    (
                        expected,
                        TypeContractContext::Field {
                            name: leaf_field.clone(),
                        },
                    )
                }),
                value_syntax,
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
                let rhs = self.compile_assignment_value(value, None, value_syntax)?;
                let dst = self.alloc_register()?;
                self.emit(compound_assignment_instruction_or_error(
                    op, dst, current, rhs,
                )?);
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

    fn compile_host_assignment(
        &mut self,
        op: AssignOp,
        target: &Expr,
        value: &Expr,
        target_syntax: AssignmentTargetSyntax<'_, '_>,
        value_syntax: AssignmentValueSyntax<'_, '_>,
    ) -> CompileResult<Register> {
        let path = self.compile_host_assignment_target(target, target_syntax.expression)?;
        let root = self.compile_host_path_root(&path.root)?;
        let src = self.compile_assignment_value(value, None, value_syntax)?;
        match op {
            AssignOp::Set => self.emit_host_write(root, path, src, target.span)?,
            AssignOp::Add => {
                self.emit_host_mutate(root, path, HostMutationOp::Add, src, target.span)?
            }
            AssignOp::Sub => {
                self.emit_host_mutate(root, path, HostMutationOp::Sub, src, target.span)?
            }
            AssignOp::Mul => {
                self.emit_host_mutate(root, path, HostMutationOp::Mul, src, target.span)?
            }
            AssignOp::Div => {
                self.emit_host_mutate(root, path, HostMutationOp::Div, src, target.span)?
            }
            AssignOp::Rem => {
                self.emit_host_mutate(root, path, HostMutationOp::Rem, src, target.span)?
            }
        }
        Ok(src)
    }

    fn compile_assignment_value(
        &mut self,
        value: &Expr,
        expected: Option<(RuntimeTypeFact, TypeContractContext)>,
        syntax: AssignmentValueSyntax<'_, '_>,
    ) -> CompileResult<Register> {
        if let Some(payload) = syntax.expression
            && !payload_syntax_overlaps_expr(payload, value)
        {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "mismatched CST assignment value",
            )));
        }
        if let Some((expected, context)) = expected {
            return self.compile_expr_with_expected_type_and_payload(
                value,
                expected,
                context,
                syntax.expression,
            );
        }
        if syntax.expression.is_some() && syntax.kind.is_none() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "missing CST assignment value",
            )));
        }
        if let Some(kind) = syntax.kind {
            if matches!(
                kind,
                SyntaxExpressionKind::Block
                    | SyntaxExpressionKind::If
                    | SyntaxExpressionKind::Match
            ) {
                return self.compile_assignment_value_with_syntax_kind(
                    value,
                    kind,
                    syntax.expression,
                );
            }
            return self.compile_expr_with_payload(value, syntax.expression);
        }
        self.compile_expr(value)
    }

    fn compile_assignment_value_with_syntax_kind(
        &mut self,
        value: &Expr,
        kind: SyntaxExpressionKind,
        expression_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<Register> {
        match kind {
            SyntaxExpressionKind::Block => {
                let dst = self.alloc_register()?;
                let Some(expression_payload) = expression_payload else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST assignment value block body payload",
                    )));
                };
                let Some(source) = expression_payload.source() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST assignment value block body payload",
                    )));
                };
                let Some(expression) = expression_payload.syntax_expression() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST assignment value block body payload",
                    )));
                };
                let Some(_) = self.compile_syntax_block_expr_to(source, expression, dst)? else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST assignment value block body payload",
                    )));
                };
                Ok(dst)
            }
            SyntaxExpressionKind::If => {
                let dst = self.alloc_register()?;
                let Some(expression_payload) = expression_payload else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST assignment value if payload",
                    )));
                };
                let Some(source) = expression_payload.source() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST assignment value if payload",
                    )));
                };
                let Some(if_expr) = expression_payload
                    .syntax_expression()
                    .and_then(vela_syntax::ast::SyntaxExpression::as_if)
                else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST assignment value if payload",
                    )));
                };
                let Some(_) = self.compile_syntax_if_value_to(source, &if_expr, dst)? else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "missing CST assignment value if payload",
                    )));
                };
                Ok(dst)
            }
            SyntaxExpressionKind::Match => {
                let dst = self.alloc_register()?;
                if let Some(expression_payload) = expression_payload
                    && let Some(_) =
                        self.compile_syntax_match_payload_value_to(expression_payload, dst)?
                {
                    return Ok(dst);
                }
                Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "missing CST assignment value match payload",
                )))
            }
            _ => self.compile_expr(value),
        }
    }

    fn compile_host_assignment_target<'expr>(
        &mut self,
        target: &'expr Expr,
        target_payload: Option<&CompilerExpressionPayload<'expr>>,
    ) -> CompileResult<HostPath<'expr>> {
        let Some(path) = self.host_field_path_with_payload(target, target_payload) else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "assignment target",
            )));
        };
        if path.segments.is_empty() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "host path",
            )));
        }
        Ok(path)
    }

    fn reject_read_only_host_assignment(
        &self,
        target: &Expr,
        syntax: AssignmentTargetSyntax<'_, '_>,
    ) -> CompileResult<()> {
        let Some((receiver_type, field)) = self.host_assignment_receiver_and_field(syntax) else {
            return Ok(());
        };
        let Some(access) = self.host_field_info(Some(receiver_type.as_str()), field.as_str())
        else {
            return Ok(());
        };
        if access.writable {
            return Ok(());
        }
        Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
            vec![
                Diagnostic::error(format!(
                    "field `{receiver_type}.{field}` is read-only for script writes"
                ))
                .with_code("analysis::field_not_writable")
                .with_span(target.span)
                .with_label(target.span, "assignment targets a read-only field")
                .with_label(
                    target.span,
                    "write through an exposed method or a writable field instead",
                ),
            ],
        )))
    }

    fn host_assignment_receiver_and_field(
        &self,
        syntax: AssignmentTargetSyntax<'_, '_>,
    ) -> Option<(String, String)> {
        let payload = syntax.expression?;
        match payload.syntax_kind() {
            Some(SyntaxExpressionKind::Field) | None => {
                let base_payload = payload.field_base_payload()?;
                let field = payload.syntax_field_name()?;
                let receiver_type = self.script_type_for_payload(&base_payload)?;
                Some((receiver_type, field))
            }
            Some(SyntaxExpressionKind::Path) => {
                let path = payload.syntax_path_segments()?;
                let (field, receiver_path) = path.split_last()?;
                let [receiver] = receiver_path else {
                    return None;
                };
                Some((self.script_types.name(receiver)?, field.clone()))
            }
            Some(_) => None,
        }
    }
}
