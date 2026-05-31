use vela_common::Span;
use vela_hir::{BindingResolution, HirLocalId};
use vela_syntax::{AssignOp, Expr, ExprKind};

use crate::{InstructionKind, Register};

use super::host_paths::{HostPath, HostPathPart, host_field_path};
use super::operators::compound_assignment_instruction;
use super::script_types::ScriptTypeFact;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler};

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordFieldAssignmentTarget {
    record: Register,
    field: String,
    slot: Option<usize>,
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
            let assigned =
                self.compile_local_assignment(*op, target.span, name, local, value, script_fact)?;
            return Ok(assigned);
        }
        if matches!(&target.kind, ExprKind::Index { .. })
            && host_field_path(&self.facts.options, target).is_none()
            && let ExprKind::Index { base, index } = &target.kind
        {
            return self.compile_index_assignment(*op, base, index, value);
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
        script_fact: Option<ScriptTypeFact>,
    ) -> CompileResult<Register> {
        let target = self.local_register_at_span(target_span, &name)?;
        if let Some(local) = local {
            self.hir_locals.insert(local, target);
            self.script_types
                .set_local_fact(local, name.clone(), script_fact);
        } else {
            self.script_types.set_name_fact(name.clone(), script_fact);
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

    fn record_field_assignment_target(
        &mut self,
        target: &Expr,
    ) -> CompileResult<Option<RecordFieldAssignmentTarget>> {
        match &target.kind {
            ExprKind::Path(path) => {
                let [record, field] = path.as_slice() else {
                    return Ok(None);
                };
                let slot = self.script_record_field_slot_for_path_root(target.span, record, field);
                if slot.is_none() && self.facts.options.host_fields.contains_key(field) {
                    return Ok(None);
                }
                Ok(Some(RecordFieldAssignmentTarget {
                    record: self.local_register_at_span(target.span, record)?,
                    field: field.clone(),
                    slot,
                }))
            }
            ExprKind::Field { base, name } => {
                let slot = self.script_record_field_slot_for_receiver(base, name);
                if slot.is_none() && self.facts.options.host_fields.contains_key(name) {
                    return Ok(None);
                }
                let ExprKind::Path(path) = &base.kind else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "record field assignment target",
                    )));
                };
                let [record] = path.as_slice() else {
                    return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                        "record field assignment target",
                    )));
                };
                Ok(Some(RecordFieldAssignmentTarget {
                    record: self.local_register_at_span(base.span, record)?,
                    field: name.clone(),
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
        let assigned = match op {
            AssignOp::Set => self.compile_expr(value)?,
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                let current = self.alloc_register()?;
                if let Some(slot) = target.slot {
                    self.emit(InstructionKind::GetRecordSlot {
                        dst: current,
                        record: target.record,
                        field: target.field.clone(),
                        slot,
                    });
                } else {
                    self.emit(InstructionKind::GetRecordField {
                        dst: current,
                        record: target.record,
                        field: target.field.clone(),
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
        if let Some(slot) = target.slot {
            self.emit(InstructionKind::SetRecordSlot {
                record: target.record,
                field: target.field,
                slot,
                src: assigned,
            });
        } else {
            self.emit(InstructionKind::SetRecordField {
                record: target.record,
                field: target.field,
                src: assigned,
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
