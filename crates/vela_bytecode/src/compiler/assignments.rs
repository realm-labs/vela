use vela_common::Span;
use vela_hir::binding::BindingResolution;
use vela_hir::ids::HirLocalId;
use vela_syntax::ast::{AssignOp, Expr, ExprKind};

use crate::{InstructionKind, Register};

use super::host_paths::{HostPath, HostPathPart, host_field_path};
use super::operators::compound_assignment_instruction;
use super::script_types::ScriptTypeFact;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordFieldAssignmentTarget {
    root: Register,
    fields: Vec<String>,
    slot: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct IndexedRecordFieldAssignmentTarget<'expr> {
    collection: &'expr Expr,
    index: &'expr Expr,
    fields: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalAssignmentFacts {
    script: Option<ScriptTypeFact>,
    value_type: Option<String>,
}

impl Compiler<'_> {
    pub(super) fn compile_assignment(&mut self, expr: &Expr) -> CompileResult<Register> {
        let ExprKind::Assign { op, target, value } = &expr.kind else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "assignment statement",
            )));
        };
        if let Some((name, local)) = self.local_assignment_target(target) {
            let script_fact = (*op == AssignOp::Set)
                .then(|| self.script_fact_for_expr(value))
                .flatten();
            let value_type = (*op == AssignOp::Set)
                .then(|| self.value_type_for_expr(value))
                .flatten();
            let facts = LocalAssignmentFacts {
                script: script_fact,
                value_type,
            };
            let assigned =
                self.compile_local_assignment(*op, target.span, name, local, value, facts)?;
            return Ok(assigned);
        }
        if matches!(&target.kind, ExprKind::Index { .. })
            && host_field_path(&self.facts.options, target).is_none()
            && let ExprKind::Index { base, index } = &target.kind
        {
            return self.compile_index_assignment(*op, base, index, value);
        }
        if let Some(target) = self.indexed_record_field_assignment_target(target) {
            return self.compile_indexed_record_field_assignment(*op, target, value);
        }
        if let Some(target) = self.record_field_assignment_target(target)? {
            return self.compile_record_field_assignment(*op, target, value);
        }
        self.compile_host_assignment(*op, target, value)
    }

    fn local_assignment_target(&self, target: &Expr) -> Option<(String, Option<HirLocalId>)> {
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
        Some((name.clone(), local))
    }

    fn compile_local_assignment(
        &mut self,
        op: AssignOp,
        target_span: Span,
        name: String,
        local: Option<HirLocalId>,
        value: &Expr,
        facts: LocalAssignmentFacts,
    ) -> CompileResult<Register> {
        let target = self.local_register_at_span(target_span, &name)?;
        if let Some(local) = local {
            self.hir_locals.insert(local, target);
            self.script_types
                .set_local_fact(local, name.clone(), facts.script);
            self.value_types
                .set_local(local, name.clone(), facts.value_type);
        } else {
            self.script_types.set_name_fact(name.clone(), facts.script);
            self.value_types.set_name(name.clone(), facts.value_type);
        }
        let assigned = match op {
            AssignOp::Set => {
                let src = self.compile_expr(value)?;
                self.emit(InstructionKind::Move { dst: target, src });
                src
            }
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let rhs = self.compile_expr(value)?;
                let dst = self.alloc_register()?;
                self.emit(compound_assignment_instruction_or_error(
                    op, dst, target, rhs,
                )?);
                self.emit(InstructionKind::Move {
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
    ) -> CompileResult<Register> {
        let base = self.compile_expr(base)?;
        let index = self.compile_expr(index)?;
        let assigned = match op {
            AssignOp::Set => self.compile_expr(value)?,
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let current = self.alloc_register()?;
                self.emit(InstructionKind::GetIndex {
                    dst: current,
                    base,
                    index,
                });
                let rhs = self.compile_expr(value)?;
                let dst = self.alloc_register()?;
                self.emit(compound_assignment_instruction_or_error(
                    op, dst, current, rhs,
                )?);
                dst
            }
        };
        self.emit(InstructionKind::SetIndex {
            base,
            index,
            src: assigned,
        });
        Ok(assigned)
    }

    fn indexed_record_field_assignment_target<'expr>(
        &self,
        target: &'expr Expr,
    ) -> Option<IndexedRecordFieldAssignmentTarget<'expr>> {
        if host_field_path(&self.facts.options, target).is_some() {
            return None;
        }
        let (collection, index, fields) = indexed_record_field_parts(target)?;
        Some(IndexedRecordFieldAssignmentTarget {
            collection,
            index,
            fields,
        })
    }

    fn record_field_assignment_target(
        &mut self,
        target: &Expr,
    ) -> CompileResult<Option<RecordFieldAssignmentTarget>> {
        match &target.kind {
            ExprKind::Path(path) => {
                let Some((record, fields)) = record_path_parts(path) else {
                    return Ok(None);
                };
                if host_field_path(&self.facts.options, target).is_some() {
                    return Ok(None);
                }
                let slot = match fields.as_slice() {
                    [field] => self.script_record_field_slot_for_path_root(
                        target.span,
                        record,
                        field.as_str(),
                    ),
                    _ => None,
                };
                Ok(Some(RecordFieldAssignmentTarget {
                    root: self.local_register_at_span(target.span, record)?,
                    fields,
                    slot,
                }))
            }
            ExprKind::Field { base, name } => {
                if host_field_path(&self.facts.options, target).is_some() {
                    return Ok(None);
                }
                let Some((record, fields)) = record_field_expr_parts(target) else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "record field assignment target",
                    )));
                };
                let slot = (fields.len() == 1)
                    .then(|| self.script_record_field_slot_for_receiver(base, name))
                    .flatten();
                Ok(Some(RecordFieldAssignmentTarget {
                    root: self.local_register_at_span(target.span, record)?,
                    fields,
                    slot,
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
    ) -> CompileResult<Register> {
        if target.fields.len() > 1 {
            return self.compile_nested_record_field_assignment(
                op,
                target.root,
                target.fields,
                value,
            );
        }
        let [field] = target.fields.as_slice() else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "record field assignment target",
            )));
        };
        self.compile_record_field_assignment_at_root(op, target.root, field, target.slot, value)
    }

    fn compile_record_field_assignment_at_root(
        &mut self,
        op: AssignOp,
        root: Register,
        field: &str,
        slot: Option<usize>,
        value: &Expr,
    ) -> CompileResult<Register> {
        let assigned = match op {
            AssignOp::Set => self.compile_expr(value)?,
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let current = self.alloc_register()?;
                if let Some(slot) = slot {
                    self.emit(InstructionKind::GetRecordSlot {
                        dst: current,
                        record: root,
                        field: field.to_owned(),
                        slot,
                    });
                } else {
                    self.emit(InstructionKind::GetRecordField {
                        dst: current,
                        record: root,
                        field: field.to_owned(),
                    });
                }
                let rhs = self.compile_expr(value)?;
                let dst = self.alloc_register()?;
                self.emit(compound_assignment_instruction_or_error(
                    op, dst, current, rhs,
                )?);
                dst
            }
        };
        if let Some(slot) = slot {
            self.emit(InstructionKind::SetRecordSlot {
                record: root,
                field: field.to_owned(),
                slot,
                src: assigned,
            });
        } else {
            self.emit(InstructionKind::SetRecordField {
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
    ) -> CompileResult<Register> {
        let collection = self.compile_expr(target.collection)?;
        let index = self.compile_expr(target.index)?;
        let record = self.alloc_register()?;
        self.emit(InstructionKind::GetIndex {
            dst: record,
            base: collection,
            index,
        });

        let assigned = if target.fields.len() > 1 {
            self.compile_nested_record_field_assignment(op, record, target.fields, value)?
        } else {
            let [field] = target.fields.as_slice() else {
                return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                    "record field assignment target",
                )));
            };
            self.compile_record_field_assignment_at_root(op, record, field, None, value)?
        };

        self.emit(InstructionKind::SetIndex {
            base: collection,
            index,
            src: record,
        });
        Ok(assigned)
    }

    fn compile_nested_record_field_assignment(
        &mut self,
        op: AssignOp,
        root: Register,
        fields: Vec<String>,
        value: &Expr,
    ) -> CompileResult<Register> {
        let mut records = vec![root];
        for field in fields.iter().take(fields.len().saturating_sub(1)) {
            let dst = self.alloc_register()?;
            let record = *records
                .last()
                .expect("nested record assignment always has root");
            self.emit(InstructionKind::GetRecordField {
                dst,
                record,
                field: field.clone(),
            });
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
            AssignOp::Set => self.compile_expr(value)?,
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let current = self.alloc_register()?;
                self.emit(InstructionKind::GetRecordField {
                    dst: current,
                    record: leaf_record,
                    field: leaf_field.clone(),
                });
                let rhs = self.compile_expr(value)?;
                let dst = self.alloc_register()?;
                self.emit(compound_assignment_instruction_or_error(
                    op, dst, current, rhs,
                )?);
                dst
            }
        };

        self.emit(InstructionKind::SetRecordField {
            record: leaf_record,
            field: leaf_field,
            src: assigned,
        });
        for (index, field) in fields
            .iter()
            .take(fields.len().saturating_sub(1))
            .enumerate()
            .rev()
        {
            self.emit(InstructionKind::SetRecordField {
                record: records[index],
                field: field.clone(),
                src: records[index + 1],
            });
        }
        Ok(assigned)
    }

    fn compile_host_assignment(
        &mut self,
        op: AssignOp,
        target: &Expr,
        value: &Expr,
    ) -> CompileResult<Register> {
        let HostPath { root, segments } = self.compile_host_assignment_target(target)?;
        let root = self.compile_host_path_root(target.span, root)?;
        let field = match segments.as_slice() {
            [HostPathPart::Field(field)] => Some(*field),
            _ => None,
        };
        let segments = field
            .is_none()
            .then(|| self.compile_host_path_segments(segments))
            .transpose()?;
        let src = self.compile_expr(value)?;
        match op {
            AssignOp::Set => {
                if let Some(field) = field {
                    self.emit(InstructionKind::SetHostField { root, field, src });
                } else {
                    self.emit(InstructionKind::SetHostPath {
                        root,
                        segments: segments.expect("host path segments"),
                        src,
                    });
                }
            }
            AssignOp::Add => {
                if let Some(field) = field {
                    self.emit(InstructionKind::AddHostField {
                        root,
                        field,
                        rhs: src,
                    });
                } else {
                    self.emit(InstructionKind::AddHostPath {
                        root,
                        segments: segments.expect("host path segments"),
                        rhs: src,
                    });
                }
            }
            AssignOp::Sub => {
                if let Some(field) = field {
                    self.emit(InstructionKind::SubHostField {
                        root,
                        field,
                        rhs: src,
                    });
                } else {
                    self.emit(InstructionKind::SubHostPath {
                        root,
                        segments: segments.expect("host path segments"),
                        rhs: src,
                    });
                }
            }
            AssignOp::Mul => {
                if let Some(field) = field {
                    self.emit(InstructionKind::MulHostField {
                        root,
                        field,
                        rhs: src,
                    });
                } else {
                    self.emit(InstructionKind::MulHostPath {
                        root,
                        segments: segments.expect("host path segments"),
                        rhs: src,
                    });
                }
            }
            AssignOp::Div => {
                if let Some(field) = field {
                    self.emit(InstructionKind::DivHostField {
                        root,
                        field,
                        rhs: src,
                    });
                } else {
                    self.emit(InstructionKind::DivHostPath {
                        root,
                        segments: segments.expect("host path segments"),
                        rhs: src,
                    });
                }
            }
            AssignOp::Rem => {
                if let Some(field) = field {
                    self.emit(InstructionKind::RemHostField {
                        root,
                        field,
                        rhs: src,
                    });
                } else {
                    self.emit(InstructionKind::RemHostPath {
                        root,
                        segments: segments.expect("host path segments"),
                        rhs: src,
                    });
                }
            }
        }
        Ok(src)
    }

    fn compile_host_assignment_target<'expr>(
        &mut self,
        target: &'expr Expr,
    ) -> CompileResult<HostPath<'expr>> {
        let Some(path) = host_field_path(&self.facts.options, target) else {
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

fn record_field_expr_parts(expr: &Expr) -> Option<(&str, Vec<String>)> {
    match &expr.kind {
        ExprKind::Path(path) => {
            let root = path.first()?;
            Some((root.as_str(), path[1..].to_vec()))
        }
        ExprKind::Field { base, name } => {
            let (root, mut fields) = record_field_expr_parts(base)?;
            fields.push(name.clone());
            Some((root, fields))
        }
        _ => None,
    }
}

fn indexed_record_field_parts(target: &Expr) -> Option<(&Expr, &Expr, Vec<String>)> {
    let ExprKind::Field { base, name } = &target.kind else {
        return None;
    };
    let (collection, index, mut fields) = indexed_record_field_base_parts(base)?;
    fields.push(name.clone());
    Some((collection, index, fields))
}

fn indexed_record_field_base_parts(expr: &Expr) -> Option<(&Expr, &Expr, Vec<String>)> {
    match &expr.kind {
        ExprKind::Index { base, index } if is_local_index_collection(base) => {
            Some((base, index, Vec::new()))
        }
        ExprKind::Field { base, name } => {
            let (collection, index, mut fields) = indexed_record_field_base_parts(base)?;
            fields.push(name.clone());
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
) -> CompileResult<InstructionKind> {
    compound_assignment_instruction(op, dst, lhs, rhs).ok_or_else(|| {
        CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "compound assignment operator",
        ))
    })
}
