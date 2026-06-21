use vela_common::{Diagnostic, Span};
use vela_hir::binding::BindingResolution;
use vela_hir::ids::HirLocalId;
use vela_host::resolved::HostMutationOp;
use vela_syntax::ast::{AssignOp, Expr, ExprKind, SyntaxExpressionKind};

use crate::{Register, UnlinkedInstructionKind};

use super::body_payloads::{
    CompilerBodyPayload, CompilerExpressionPayload, CompilerIfPayload, CompilerMatchArmPayload,
};
use super::expression_payload_kinds::expression_payload_kind_matches;
use super::expressions::literal_string;
use super::host_paths::{HostIndexAccessKind, HostPath};
use super::operators::{compound_assignment_instruction, i64_compound_assignment_instruction};
use super::record_shapes::RecordShape;
use super::script_types::ScriptTypeFact;
use super::value_types::{RuntimeTypeFact, TypeContractContext};
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

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
pub(in crate::compiler) struct AssignmentValuePayloads<'payload, 'ast> {
    block_body: Option<&'payload CompilerBodyPayload<'ast>>,
    if_expr: Option<&'payload CompilerIfPayload<'ast>>,
    match_scrutinee: Option<&'payload CompilerExpressionPayload<'ast>>,
    match_arms: Option<&'payload [CompilerMatchArmPayload<'ast>]>,
}

impl<'payload, 'ast> AssignmentValuePayloads<'payload, 'ast> {
    pub(in crate::compiler) fn new(
        block_body: Option<&'payload CompilerBodyPayload<'ast>>,
        if_expr: Option<&'payload CompilerIfPayload<'ast>>,
        match_scrutinee: Option<&'payload CompilerExpressionPayload<'ast>>,
        match_arms: Option<&'payload [CompilerMatchArmPayload<'ast>]>,
    ) -> Self {
        Self {
            block_body,
            if_expr,
            match_scrutinee,
            match_arms,
        }
    }

    fn none() -> Self {
        Self {
            block_body: None,
            if_expr: None,
            match_scrutinee: None,
            match_arms: None,
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::compiler) struct AssignmentValueSyntax<'payload, 'ast> {
    kind: Option<SyntaxExpressionKind>,
    expression: Option<&'payload CompilerExpressionPayload<'ast>>,
    payloads: AssignmentValuePayloads<'payload, 'ast>,
}

impl<'payload, 'ast> AssignmentValueSyntax<'payload, 'ast> {
    pub(in crate::compiler) fn new(
        kind: Option<SyntaxExpressionKind>,
        expression: Option<&'payload CompilerExpressionPayload<'ast>>,
        payloads: AssignmentValuePayloads<'payload, 'ast>,
    ) -> Self {
        Self {
            kind,
            expression,
            payloads,
        }
    }

    fn none() -> Self {
        Self {
            kind: None,
            expression: None,
            payloads: AssignmentValuePayloads::none(),
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::compiler) struct AssignmentTargetSyntax<'payload, 'ast> {
    expression: Option<&'payload CompilerExpressionPayload<'ast>>,
}

impl<'payload, 'ast> AssignmentTargetSyntax<'payload, 'ast> {
    pub(in crate::compiler) fn new(
        expression: Option<&'payload CompilerExpressionPayload<'ast>>,
    ) -> Self {
        Self { expression }
    }

    fn none() -> Self {
        Self { expression: None }
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
    match &payload.fallback().kind {
        ExprKind::Field { .. } => record_field_root_payload(payload.field_base_payload()?),
        _ => Some(payload),
    }
}

impl Compiler<'_, '_> {
    pub(super) fn compile_assignment(&mut self, expr: &Expr) -> CompileResult<Register> {
        self.compile_assignment_with_payloads(
            expr,
            AssignmentTargetSyntax::none(),
            AssignmentValueSyntax::none(),
        )
    }

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
        if let Some(local_target) = self.local_assignment_target(target) {
            let target_value_type =
                self.value_type_for_expr_with_payload(target, target_syntax.expression);
            let assigned_value_type = match op {
                AssignOp::Set => {
                    self.value_type_for_expr_with_payload(value, value_syntax.expression)
                }
                AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Rem
                    if expressions_are_i64(
                        target_value_type.clone(),
                        self.value_type_for_expr_with_payload(value, value_syntax.expression),
                    ) =>
                {
                    Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64))
                }
                AssignOp::Div => None,
                AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Rem => None,
            };
            let script_fact = (*op == AssignOp::Set)
                .then(|| self.script_fact_for_expr_with_payload(value, value_syntax.expression))
                .flatten();
            let value_shape = (*op == AssignOp::Set)
                .then(|| self.value_shape_for_expr_with_payload(value, value_syntax.expression))
                .flatten();
            let facts = LocalAssignmentFacts {
                script: script_fact,
                value_type: assigned_value_type,
                value_shape,
            };
            let assigned =
                self.compile_local_assignment(*op, local_target, value, facts, value_syntax)?;
            return Ok(assigned);
        }
        self.reject_read_only_host_assignment(target)?;
        if let ExprKind::Index { base, index } = &target.kind {
            let operand_payloads = target_syntax.index_operand_payloads();
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
                index_payload,
            )?;
            if self.host_field_path(target).is_none() {
                return self.compile_index_assignment(
                    *op,
                    base,
                    index,
                    value,
                    target_syntax,
                    value_syntax,
                );
            }
        }
        if let Some(target) = self.indexed_record_field_assignment_target(target, target_syntax) {
            return self.compile_indexed_record_field_assignment(*op, target, value, value_syntax);
        }
        if let Some(target) = self.record_field_assignment_target(target, target_syntax)? {
            return self.compile_record_field_assignment(*op, target, value, value_syntax);
        }
        self.compile_host_assignment(*op, target, value, target_syntax, value_syntax)
    }

    fn local_assignment_target(&self, target: &Expr) -> Option<LocalAssignmentTarget> {
        let ExprKind::Path(path) = &target.kind else {
            return None;
        };
        let [name] = path.as_slice() else {
            return None;
        };
        let local = match self.bindings.resolution_at_span(target.span) {
            Some(BindingResolution::Local(local)) => Some(*local),
            _ if self.locals.contains_key(name) => None,
            _ => return None,
        };
        Some(LocalAssignmentTarget {
            target_span: target.span,
            name: name.clone(),
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
        if let Some(key) = literal_string(index) {
            return self.compile_string_key_index_assignment(op, base, key, value, value_syntax);
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
        Some(IndexedRecordFieldAssignmentTarget {
            collection,
            index,
            collection_payload,
            index_payload,
            fields,
            element_shape: self.record_shape_for_index_collection(collection),
        })
    }

    fn record_field_assignment_target(
        &mut self,
        target: &Expr,
        syntax: AssignmentTargetSyntax<'_, '_>,
    ) -> CompileResult<Option<RecordFieldAssignmentTarget>> {
        match &target.kind {
            ExprKind::Path(path) => {
                let path = syntax
                    .expression
                    .and_then(CompilerExpressionPayload::path_segments)
                    .unwrap_or_else(|| path.to_owned());
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
            ExprKind::Field { base, name } => {
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
                let root_type = self.script_type_for_expr(parts.root);
                let shape = self.record_shape_for_expr(parts.root).or_else(|| {
                    root_type
                        .as_deref()
                        .and_then(|type_name| self.record_shape_for_type(type_name))
                });
                let slot = (parts.fields.len() == 1)
                    .then(|| {
                        let name = parts.fields.first().map_or(name.as_str(), String::as_str);
                        self.script_record_field_slot_for_receiver(base, name)
                            .or_else(|| self.record_field_shape_slot_for_receiver(base, name))
                    })
                    .flatten();
                let value_type =
                    self.schema_record_field_value_type(root_type.as_deref(), &parts.fields);
                let root_payload = syntax.record_field_root_payload();
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

    fn schema_record_field_value_type(
        &self,
        root_type: Option<&str>,
        fields: &[String],
    ) -> Option<RuntimeTypeFact> {
        let mut current_type = root_type?.to_owned();
        let (leaf, parents) = fields.split_last()?;
        for field in parents {
            current_type = self
                .facts
                .script_field_slots
                .record_field_fact(&current_type, field)?
                .type_name;
        }
        self.facts
            .script_field_slots
            .record_field_value_type(&current_type, leaf)
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
        if let Some((expected, context)) = expected {
            return self.compile_expr_with_expected_type_and_payload(
                value,
                expected,
                context,
                syntax.expression,
            );
        }
        if let Some(kind) = syntax.kind
            && expression_payload_kind_matches(kind, value)
        {
            if matches!(
                kind,
                SyntaxExpressionKind::Block
                    | SyntaxExpressionKind::If
                    | SyntaxExpressionKind::Match
            ) {
                return self.compile_assignment_value_with_syntax_kind(
                    value,
                    kind,
                    syntax.payloads,
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
        syntax_payloads: AssignmentValuePayloads<'_, '_>,
    ) -> CompileResult<Register> {
        match kind {
            SyntaxExpressionKind::Block => {
                let ExprKind::Block(block) = &value.kind else {
                    unreachable!("validated CST block assignment value kind");
                };
                let dst = self.alloc_register()?;
                if let Some(body_payload) = syntax_payloads.block_body {
                    self.compile_block_payload_value_to(body_payload, dst)?;
                } else {
                    self.compile_block_value_to(block, dst)?;
                }
                Ok(dst)
            }
            SyntaxExpressionKind::If => {
                let ExprKind::If(if_expr) = &value.kind else {
                    unreachable!("validated CST if assignment value kind");
                };
                let dst = self.alloc_register()?;
                self.compile_if_value_with_payloads(if_expr, dst, syntax_payloads.if_expr)?;
                Ok(dst)
            }
            SyntaxExpressionKind::Match => {
                let ExprKind::Match(match_expr) = &value.kind else {
                    unreachable!("validated CST match assignment value kind");
                };
                let dst = self.alloc_register()?;
                self.compile_match_value_with_payloads(
                    match_expr,
                    dst,
                    syntax_payloads.match_scrutinee,
                    syntax_payloads.match_arms,
                )?;
                Ok(dst)
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

    fn reject_read_only_host_assignment(&self, target: &Expr) -> CompileResult<()> {
        let Some((receiver_type, field)) = self.host_assignment_receiver_and_field(target) else {
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

    fn host_assignment_receiver_and_field(&self, target: &Expr) -> Option<(String, String)> {
        match &target.kind {
            ExprKind::Field { base, name } => {
                Some((self.script_type_for_expr(base)?, name.clone()))
            }
            ExprKind::Path(path) => {
                let (field, receiver_path) = path.split_last()?;
                let [receiver] = receiver_path else {
                    return None;
                };
                Some((self.script_types.name(receiver)?, field.clone()))
            }
            _ => None,
        }
    }
}

fn expressions_are_i64(left: Option<RuntimeTypeFact>, right: Option<RuntimeTypeFact>) -> bool {
    matches!(
        (left, right),
        (
            Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64)),
            Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64))
        )
    )
}

fn record_path_parts(path: &[String]) -> Option<(&str, Vec<String>)> {
    if path.len() < 2 {
        return None;
    }
    record_field_base_parts(path)
}

fn record_field_base_parts(path: &[String]) -> Option<(&str, Vec<String>)> {
    let root = path.first()?;
    Some((root.as_str(), path[1..].to_vec()))
}

fn record_field_expr_parts_with_payload<'expr>(
    expr: &'expr Expr,
    payload: Option<CompilerExpressionPayload<'expr>>,
) -> Option<RecordFieldExprParts<'expr>> {
    match &expr.kind {
        ExprKind::Field { base, name } => {
            let base_payload = payload
                .as_ref()
                .and_then(CompilerExpressionPayload::field_base_payload);
            let mut parts = record_field_expr_parts_with_payload(base, base_payload)
                .unwrap_or_else(|| RecordFieldExprParts {
                    root: base,
                    fields: Vec::new(),
                });
            let name = payload
                .as_ref()
                .and_then(CompilerExpressionPayload::field_name)
                .unwrap_or_else(|| name.clone());
            parts.fields.push(name);
            Some(parts)
        }
        _ => None,
    }
}

fn indexed_record_field_parts_with_payload<'expr>(
    target: &'expr Expr,
    payload: Option<CompilerExpressionPayload<'expr>>,
) -> Option<(&'expr Expr, &'expr Expr, Vec<String>)> {
    let ExprKind::Field { base, name } = &target.kind else {
        return None;
    };
    let base_payload = payload
        .as_ref()
        .and_then(CompilerExpressionPayload::field_base_payload);
    let (collection, index, mut fields) =
        indexed_record_field_base_parts_with_payload(base, base_payload)?;
    let name = payload
        .as_ref()
        .and_then(CompilerExpressionPayload::field_name)
        .unwrap_or_else(|| name.clone());
    fields.push(name);
    Some((collection, index, fields))
}

fn indexed_record_field_base_parts_with_payload<'expr>(
    expr: &'expr Expr,
    payload: Option<CompilerExpressionPayload<'expr>>,
) -> Option<(&'expr Expr, &'expr Expr, Vec<String>)> {
    match &expr.kind {
        ExprKind::Index { base, index } if is_local_index_collection(base) => {
            Some((base, index, Vec::new()))
        }
        ExprKind::Field { base, name } => {
            let base_payload = payload
                .as_ref()
                .and_then(CompilerExpressionPayload::field_base_payload);
            let (collection, index, mut fields) =
                indexed_record_field_base_parts_with_payload(base, base_payload)?;
            let name = payload
                .as_ref()
                .and_then(CompilerExpressionPayload::field_name)
                .unwrap_or_else(|| name.clone());
            fields.push(name);
            Some((collection, index, fields))
        }
        _ => None,
    }
}

fn is_local_index_collection(expr: &Expr) -> bool {
    matches!(&expr.kind, ExprKind::Path(path) if path.len() == 1)
}

fn compound_assignment_instruction_or_error(
    op: AssignOp,
    dst: Register,
    lhs: Register,
    rhs: Register,
) -> CompileResult<UnlinkedInstructionKind> {
    compound_assignment_instruction(op, dst, lhs, rhs).ok_or_else(|| {
        CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "compound assignment operator",
        ))
    })
}
