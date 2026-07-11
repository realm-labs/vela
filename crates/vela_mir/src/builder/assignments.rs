use vela_analysis::semantic_facts::OperatorTargetFact;
use vela_analysis::type_fact::TypeFact;
use vela_hir::binding::BindingResolution;
use vela_hir::body::{HirAssignOp, HirExprKind, HirField, HirIndex, HirLiteral};
use vela_hir::ids::HirExprId;

use crate::{
    CompileFieldTarget, CompileMemberTarget, MirBinaryOp, MirBuildError, MirDynamicBinaryOp,
    MirEffect, MirFieldTarget, MirImmediate, MirIndexKey, MirIndexOperation, MirNumericBinaryOp,
    MirOperand, MirPlace, MirSafepoint, MirSourceOrigin, MirStatement, MirStatementKind,
};

use super::core::{FunctionBuilder, value_type};

#[derive(Clone, Debug)]
enum PreparedAssignmentTarget {
    Local(crate::MirLocalId),
    Index(PreparedIndexTarget),
    Field(PreparedFieldTarget),
}

#[derive(Clone, Debug)]
struct PreparedIndexTarget {
    expression: HirExprId,
    receiver: MirOperand,
    index: MirIndexKey,
}

#[derive(Clone, Debug)]
struct PreparedFieldTarget {
    fields: Vec<PreparedFieldStep>,
    receivers: Vec<MirOperand>,
    indexed_root: Option<PreparedIndexTarget>,
}

#[derive(Clone, Debug)]
struct PreparedFieldStep {
    expression: HirExprId,
    target: MirFieldTarget,
}

struct AssignmentValueInput {
    expression: HirExprId,
    operation: HirAssignOp,
    mode: AssignmentOperatorMode,
    current: MirOperand,
    value_expression: HirExprId,
    value: MirOperand,
    origin: MirSourceOrigin,
}

struct AssignmentRhs {
    expression: HirExprId,
    operation: HirAssignOp,
    mode: AssignmentOperatorMode,
    value_expression: HirExprId,
    value: MirOperand,
    origin: MirSourceOrigin,
}

impl AssignmentRhs {
    fn with_current(self, current: MirOperand) -> AssignmentValueInput {
        AssignmentValueInput {
            expression: self.expression,
            operation: self.operation,
            mode: self.mode,
            current,
            value_expression: self.value_expression,
            value: self.value,
            origin: self.origin,
        }
    }
}

enum Prepared<T> {
    Ready(T),
    Diverged,
}

impl FunctionBuilder<'_> {
    /// Lower a script-local, script-field, or script-index assignment.
    ///
    /// Host paths are deliberately rejected here with a routing error. They
    /// must be lowered by the HostAccess builder as explicit host operations,
    /// never as ordinary MIR places or script heap writes.
    pub(super) fn lower_assignment(
        &mut self,
        expression: HirExprId,
        operation: Option<HirAssignOp>,
        target: Option<HirExprId>,
        value: Option<HirExprId>,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let operation = operation
            .ok_or_else(|| self.inconsistent(origin, "assignment expression has no operator"))?;
        let target = target
            .ok_or_else(|| self.inconsistent(origin, "assignment expression has no target"))?;
        let value =
            value.ok_or_else(|| self.inconsistent(origin, "assignment expression has no value"))?;
        let operator_mode = self.assignment_operator_mode(expression, operation, origin)?;

        if self.input.targets().host_path(expression).is_some() {
            return Err(self.host_assignment_route_error(origin));
        }

        let target = match self.prepare_assignment_target(target, origin)? {
            Prepared::Ready(target) => target,
            Prepared::Diverged => return Ok(unit()),
        };
        let value_operand = self.lower_expression(value)?;
        if self.current_is_terminated()? {
            return Ok(unit());
        }
        let value_operand =
            self.capture_operand(value_operand, self.assignment_expression_origin(value)?)?;
        self.finish_assignment(
            target,
            AssignmentRhs {
                expression,
                operation,
                mode: operator_mode,
                value_expression: value,
                value: value_operand,
                origin,
            },
        )
    }

    /// Lower an ordinary script field or tuple projection read.
    pub(super) fn lower_field(
        &mut self,
        expression: HirExprId,
        field: &HirField,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        if self.input.targets().host_path(expression).is_some() {
            return Err(self.host_read_route_error(origin));
        }
        let receiver = self.lower_expression(field.receiver)?;
        if self.current_is_terminated()? {
            return Ok(unit());
        }
        match self.member_target(expression, origin)? {
            PreparedMemberTarget::Field(target) => self.append_value_statement(
                expression,
                origin,
                MirStatementKind::ReadField { receiver, target },
                MirEffect::may_trap(),
            ),
            PreparedMemberTarget::Tuple(index) => self.append_value_statement(
                expression,
                origin,
                MirStatementKind::TupleField {
                    tuple: receiver,
                    index,
                },
                MirEffect::may_trap(),
            ),
        }
    }

    /// Lower an ordinary script collection index read.
    pub(super) fn lower_index(
        &mut self,
        expression: HirExprId,
        index: &HirIndex,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        if self.input.targets().host_path(expression).is_some() {
            return Err(self.host_read_route_error(origin));
        }
        let receiver = self.lower_expression(index.receiver)?;
        if self.current_is_terminated()? {
            return Ok(unit());
        }
        let receiver =
            self.capture_operand(receiver, self.assignment_expression_origin(index.receiver)?)?;
        let index = match self.prepare_index_key(index.index)? {
            Prepared::Ready(index) => index,
            Prepared::Diverged => return Ok(unit()),
        };
        self.append_index_read(
            &PreparedIndexTarget {
                expression,
                receiver,
                index,
            },
            origin,
        )
    }

    fn prepare_assignment_target(
        &mut self,
        mut target: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<Prepared<PreparedAssignmentTarget>, MirBuildError> {
        loop {
            match self.expression_kind(target, origin)? {
                HirExprKind::Paren {
                    expression: Some(inner),
                } => target = inner,
                HirExprKind::Paren { expression: None } => {
                    return Err(self.inconsistent(origin, "assignment target is an empty paren"));
                }
                _ => break,
            }
        }

        match self.expression_kind(target, origin)? {
            HirExprKind::Path(_) => self
                .assignment_local(target, origin)
                .map(|local| Prepared::Ready(PreparedAssignmentTarget::Local(local))),
            HirExprKind::Index(index) => self
                .prepare_index_target(&index, origin)
                .map(|target| target.map(PreparedAssignmentTarget::Index)),
            HirExprKind::Field(_) => self
                .prepare_field_target(target, origin)
                .map(|target| target.map(PreparedAssignmentTarget::Field)),
            _ => Err(self.inconsistent(
                origin,
                "validated assignment target is not a local, field, index, or host path",
            )),
        }
    }

    fn prepare_index_target(
        &mut self,
        index: &HirIndex,
        origin: MirSourceOrigin,
    ) -> Result<Prepared<PreparedIndexTarget>, MirBuildError> {
        if self.input.targets().host_path(index.expression).is_some() {
            return Err(self.host_assignment_route_error(origin));
        }
        let expression = index.expression;
        let receiver = self.lower_expression(index.receiver)?;
        if self.current_is_terminated()? {
            return Ok(Prepared::Diverged);
        }
        let receiver =
            self.capture_operand(receiver, self.assignment_expression_origin(index.receiver)?)?;
        let key = match self.prepare_index_key(index.index)? {
            Prepared::Ready(index) => index,
            Prepared::Diverged => return Ok(Prepared::Diverged),
        };
        Ok(Prepared::Ready(PreparedIndexTarget {
            expression,
            receiver,
            index: key,
        }))
    }

    fn prepare_index_key(
        &mut self,
        expression: HirExprId,
    ) -> Result<Prepared<MirIndexKey>, MirBuildError> {
        let origin = self.assignment_expression_origin(expression)?;
        if let HirExprKind::Literal(HirLiteral::String(value)) =
            self.expression_kind(expression, origin)?
        {
            return Ok(Prepared::Ready(MirIndexKey::ConstantString(value)));
        }
        let value = self.lower_expression(expression)?;
        if self.current_is_terminated()? {
            return Ok(Prepared::Diverged);
        }
        let value = self.capture_operand(value, origin)?;
        Ok(Prepared::Ready(MirIndexKey::Value(value)))
    }

    fn prepare_field_target(
        &mut self,
        target: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<Prepared<PreparedFieldTarget>, MirBuildError> {
        let mut field_expressions = Vec::new();
        let mut base = target;
        loop {
            match self.expression_kind(base, origin)? {
                HirExprKind::Field(field) => {
                    field_expressions.push(field.expression);
                    base = field.receiver;
                }
                HirExprKind::Paren {
                    expression: Some(inner),
                } => base = inner,
                _ => break,
            }
        }
        field_expressions.reverse();
        if field_expressions.is_empty() {
            return Err(self.inconsistent(origin, "field assignment has no field steps"));
        }

        let (root, indexed_root) = match self.expression_kind(base, origin)? {
            HirExprKind::Index(index) => {
                let target = match self.prepare_index_target(&index, origin)? {
                    Prepared::Ready(target) => target,
                    Prepared::Diverged => return Ok(Prepared::Diverged),
                };
                let root = self.append_index_read(&target, origin)?;
                (root, Some(target))
            }
            _ => {
                let root = self.lower_expression(base)?;
                if self.current_is_terminated()? {
                    return Ok(Prepared::Diverged);
                }
                let root = self.capture_operand(root, self.assignment_expression_origin(base)?)?;
                (root, None)
            }
        };

        let mut fields = Vec::with_capacity(field_expressions.len());
        for expression in field_expressions {
            let target = match self.member_target(expression, origin)? {
                PreparedMemberTarget::Field(target) => target,
                PreparedMemberTarget::Tuple(index) => {
                    return Err(self.inconsistent(
                        origin,
                        format!(
                            "MIR v1 has no tuple element write/rebuild form for projection {index} in assignment target {expression:?}"
                        ),
                    ));
                }
            };
            fields.push(PreparedFieldStep { expression, target });
        }

        let mut receivers = vec![root];
        for field in fields.iter().take(fields.len().saturating_sub(1)) {
            let receiver = receivers
                .last()
                .cloned()
                .ok_or_else(|| self.inconsistent(origin, "field assignment lost its root"))?;
            let field_origin = self.assignment_expression_origin(field.expression)?;
            let value = self.append_value_statement(
                field.expression,
                field_origin,
                MirStatementKind::ReadField {
                    receiver,
                    target: field.target.clone(),
                },
                MirEffect::may_trap(),
            )?;
            receivers.push(value);
        }

        Ok(Prepared::Ready(PreparedFieldTarget {
            fields,
            receivers,
            indexed_root,
        }))
    }

    fn finish_assignment(
        &mut self,
        target: PreparedAssignmentTarget,
        rhs: AssignmentRhs,
    ) -> Result<MirOperand, MirBuildError> {
        match target {
            PreparedAssignmentTarget::Local(local) => {
                let origin = rhs.origin;
                let assigned = self.assigned_value(rhs.with_current(MirOperand::Local(local)))?;
                self.function.append_statement(
                    self.current_block,
                    MirStatement::assign(
                        origin,
                        MirPlace::local(local),
                        crate::MirRvalue::Use(assigned.clone()),
                    ),
                )?;
                Ok(assigned)
            }
            PreparedAssignmentTarget::Index(target) => {
                let origin = rhs.origin;
                let current = if rhs.operation == HirAssignOp::Set {
                    unit()
                } else {
                    self.append_index_read(&target, origin)?
                };
                let assigned = self.assigned_value(rhs.with_current(current))?;
                self.append_index_write(&target, assigned.clone(), origin)?;
                Ok(assigned)
            }
            PreparedAssignmentTarget::Field(target) => self.finish_field_assignment(target, rhs),
        }
    }

    fn finish_field_assignment(
        &mut self,
        target: PreparedFieldTarget,
        rhs: AssignmentRhs,
    ) -> Result<MirOperand, MirBuildError> {
        let origin = rhs.origin;
        let leaf = target
            .fields
            .last()
            .ok_or_else(|| self.inconsistent(origin, "field assignment lost its leaf"))?;
        let leaf_receiver =
            target.receivers.last().cloned().ok_or_else(|| {
                self.inconsistent(origin, "field assignment lost its leaf receiver")
            })?;
        let current = if rhs.operation == HirAssignOp::Set {
            unit()
        } else {
            self.append_value_statement(
                leaf.expression,
                self.assignment_expression_origin(leaf.expression)?,
                MirStatementKind::ReadField {
                    receiver: leaf_receiver.clone(),
                    target: leaf.target.clone(),
                },
                MirEffect::may_trap(),
            )?
        };
        let assigned = self.assigned_value(rhs.with_current(current))?;
        self.append_field_write(leaf_receiver, leaf.target.clone(), assigned.clone(), origin)?;

        for index in (0..target.fields.len().saturating_sub(1)).rev() {
            self.append_field_write(
                target.receivers[index].clone(),
                target.fields[index].target.clone(),
                target.receivers[index + 1].clone(),
                origin,
            )?;
        }
        if let Some(indexed_root) = target.indexed_root {
            let root =
                target.receivers.first().cloned().ok_or_else(|| {
                    self.inconsistent(origin, "indexed field assignment lost root")
                })?;
            self.append_index_write(&indexed_root, root, origin)?;
        }
        Ok(assigned)
    }

    fn assigned_value(&mut self, input: AssignmentValueInput) -> Result<MirOperand, MirBuildError> {
        let AssignmentValueInput {
            expression,
            operation,
            mode,
            current,
            value_expression,
            value,
            origin,
        } = input;
        if operation == HirAssignOp::Set {
            return Ok(value);
        }
        let kind = match mode {
            AssignmentOperatorMode::Dynamic => MirStatementKind::DynamicBinary {
                operation: dynamic_assignment(operation, origin, self)?,
                left: current,
                right: value,
            },
            AssignmentOperatorMode::Resolved => {
                let analysis = self.input.analysis();
                let target_fact = assignment_target(self.body, expression)
                    .and_then(|target| analysis.expression(target));
                let value_fact = analysis.expression(value_expression);
                match (target_fact, value_fact) {
                    (Some(target), Some(value_fact))
                        if numeric_primitive_pair(target, value_fact).is_some() =>
                    {
                        let kind = numeric_primitive_pair(target, value_fact)
                            .expect("matching numeric facts were checked");
                        MirStatementKind::Binary {
                            operation: MirBinaryOp::Numeric {
                                operation: numeric_assignment(operation, origin, self)?,
                                kind,
                            },
                            left: current,
                            right: value,
                        }
                    }
                    (Some(_), Some(_)) => MirStatementKind::DynamicBinary {
                        operation: dynamic_assignment(operation, origin, self)?,
                        left: current,
                        right: value,
                    },
                    _ => {
                        return Err(self.inconsistent(
                            origin,
                            "compound assignment is missing an analysis type fact",
                        ));
                    }
                }
            }
        };
        self.append_value_statement(expression, origin, kind, MirEffect::may_trap())
    }

    fn assignment_operator_mode(
        &self,
        expression: HirExprId,
        operation: HirAssignOp,
        origin: MirSourceOrigin,
    ) -> Result<AssignmentOperatorMode, MirBuildError> {
        match self.input.analysis().operator_target(expression) {
            Some(OperatorTargetFact::Assignment(target)) if target == operation => {
                Ok(AssignmentOperatorMode::Resolved)
            }
            Some(OperatorTargetFact::Assignment(_)) => Err(self.inconsistent(
                origin,
                "analysis assignment target disagrees with the HIR operator",
            )),
            Some(OperatorTargetFact::Dynamic) => Ok(AssignmentOperatorMode::Dynamic),
            Some(OperatorTargetFact::Unresolved) => {
                Err(self.inconsistent(origin, "unresolved assignment operator reached MIR"))
            }
            Some(OperatorTargetFact::Unary(_) | OperatorTargetFact::Binary(_)) => Err(self
                .inconsistent(
                    origin,
                    "analysis operator target has the wrong assignment family",
                )),
            None => Err(self.inconsistent(
                origin,
                "assignment expression has no analysis operator target",
            )),
        }
    }

    fn assignment_local(
        &self,
        expression: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<crate::MirLocalId, MirBuildError> {
        let bindings = self
            .input
            .graph()
            .bindings_for_body(self.body.id)
            .ok_or_else(|| self.inconsistent(origin, "HIR body has no binding map"))?;
        let Some(BindingResolution::Local(local)) = bindings.resolution(expression) else {
            return Err(
                self.inconsistent(origin, "assignment path does not resolve to a script local")
            );
        };
        self.local(*local, origin)
    }

    fn member_target(
        &self,
        expression: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<PreparedMemberTarget, MirBuildError> {
        let target = self.input.targets().member(expression).ok_or_else(|| {
            self.inconsistent(
                origin,
                format!("field expression {expression:?} has no compile member target"),
            )
        })?;
        match target {
            CompileMemberTarget::ScriptField(target) => {
                Ok(PreparedMemberTarget::Field(match target {
                    CompileFieldTarget::RecordSlot {
                        type_id,
                        shape,
                        field,
                    } => MirFieldTarget::RecordSlot {
                        type_id: *type_id,
                        shape: *shape,
                        field: *field,
                    },
                    CompileFieldTarget::VariantSlot {
                        type_id,
                        variant,
                        field,
                    } => MirFieldTarget::VariantSlot {
                        type_id: *type_id,
                        variant: *variant,
                        field: *field,
                    },
                    CompileFieldTarget::Dynamic { name } => {
                        MirFieldTarget::Dynamic { name: name.clone() }
                    }
                }))
            }
            CompileMemberTarget::TupleIndex(index) => Ok(PreparedMemberTarget::Tuple(*index)),
            CompileMemberTarget::Dynamic { name } => {
                Ok(PreparedMemberTarget::Field(MirFieldTarget::Dynamic {
                    name: name.clone(),
                }))
            }
            CompileMemberTarget::HostField(_) => Err(self.host_read_route_error(origin)),
            CompileMemberTarget::ScriptMethod { .. } | CompileMemberTarget::ValueMethod { .. } => {
                Err(self.inconsistent(
                    origin,
                    "method target reached field-value lowering outside a call",
                ))
            }
        }
    }

    fn append_index_read(
        &mut self,
        target: &PreparedIndexTarget,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(unit());
        }
        self.append_value_statement(
            target.expression,
            origin,
            MirStatementKind::Index(MirIndexOperation::Read {
                receiver: target.receiver.clone(),
                index: target.index.clone(),
            }),
            MirEffect::may_trap(),
        )
    }

    fn append_index_write(
        &mut self,
        target: &PreparedIndexTarget,
        value: MirOperand,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(());
        }
        let safepoint = self.function.add_safepoint(MirSafepoint::new(origin));
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                None,
                MirStatementKind::Index(MirIndexOperation::Write {
                    receiver: target.receiver.clone(),
                    index: target.index.clone(),
                    value,
                }),
                MirEffect::allocation(),
                Some(safepoint),
            ),
        )?;
        Ok(())
    }

    fn append_field_write(
        &mut self,
        receiver: MirOperand,
        target: MirFieldTarget,
        value: MirOperand,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(());
        }
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                None,
                MirStatementKind::WriteField {
                    receiver,
                    target,
                    value,
                },
                MirEffect::may_trap(),
                None,
            ),
        )?;
        Ok(())
    }

    fn append_value_statement(
        &mut self,
        expression: HirExprId,
        origin: MirSourceOrigin,
        kind: MirStatementKind,
        effect: MirEffect,
    ) -> Result<MirOperand, MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(unit());
        }
        let analysis = self.input.analysis();
        let fact = analysis.expression(expression).ok_or_else(|| {
            self.inconsistent(
                origin,
                format!("expression {expression:?} has no analysis type fact"),
            )
        })?;
        let destination = self.function.add_temp(value_type(Some(fact)), origin);
        let safepoint = effect
            .requires_safepoint()
            .then(|| self.function.add_safepoint(MirSafepoint::new(origin)));
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(destination)),
                kind,
                effect,
                safepoint,
            ),
        )?;
        Ok(MirOperand::Temp(destination))
    }

    fn expression_kind(
        &self,
        expression: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<HirExprKind, MirBuildError> {
        self.body
            .expression(expression)
            .map(|expression| expression.kind.clone())
            .ok_or_else(|| {
                self.inconsistent(origin, format!("missing HIR expression {expression:?}"))
            })
    }

    fn assignment_expression_origin(
        &self,
        expression: HirExprId,
    ) -> Result<MirSourceOrigin, MirBuildError> {
        let record = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(
                MirSourceOrigin::body(self.body.id, self.body.origin.span),
                format!("missing HIR expression {expression:?}"),
            )
        })?;
        Ok(MirSourceOrigin::expression(
            self.body.id,
            expression,
            record.origin.span,
        ))
    }

    fn host_assignment_route_error(&self, origin: MirSourceOrigin) -> MirBuildError {
        self.inconsistent(
            origin,
            "compile host-path assignment must be lowered by builder::host as an explicit HostAccess operation",
        )
    }

    fn host_read_route_error(&self, origin: MirSourceOrigin) -> MirBuildError {
        self.inconsistent(
            origin,
            "compile host-path read must be lowered by builder::host as an explicit HostAccess operation",
        )
    }
}

#[derive(Clone, Copy)]
enum AssignmentOperatorMode {
    Resolved,
    Dynamic,
}

enum PreparedMemberTarget {
    Field(MirFieldTarget),
    Tuple(u32),
}

impl<T> Prepared<T> {
    fn map<U>(self, map: impl FnOnce(T) -> U) -> Prepared<U> {
        match self {
            Self::Ready(value) => Prepared::Ready(map(value)),
            Self::Diverged => Prepared::Diverged,
        }
    }
}

fn assignment_target(body: &vela_hir::body::HirBody, expression: HirExprId) -> Option<HirExprId> {
    let HirExprKind::Assign { target, .. } = &body.expression(expression)?.kind else {
        return None;
    };
    *target
}

fn numeric_primitive_pair(left: &TypeFact, right: &TypeFact) -> Option<vela_common::NumericTag> {
    let (TypeFact::Primitive(left), TypeFact::Primitive(right)) = (left, right) else {
        return None;
    };
    (left == right).then(|| left.numeric_tag()).flatten()
}

fn numeric_assignment(
    operation: HirAssignOp,
    origin: MirSourceOrigin,
    builder: &FunctionBuilder<'_>,
) -> Result<MirNumericBinaryOp, MirBuildError> {
    match operation {
        HirAssignOp::Add => Ok(MirNumericBinaryOp::Add),
        HirAssignOp::Sub => Ok(MirNumericBinaryOp::Subtract),
        HirAssignOp::Mul => Ok(MirNumericBinaryOp::Multiply),
        HirAssignOp::Div => Ok(MirNumericBinaryOp::Divide),
        HirAssignOp::Rem => Ok(MirNumericBinaryOp::Remainder),
        HirAssignOp::Set => Err(builder.inconsistent(
            origin,
            "set assignment reached compound numeric operation lowering",
        )),
    }
}

fn dynamic_assignment(
    operation: HirAssignOp,
    origin: MirSourceOrigin,
    builder: &FunctionBuilder<'_>,
) -> Result<MirDynamicBinaryOp, MirBuildError> {
    match operation {
        HirAssignOp::Add => Ok(MirDynamicBinaryOp::Add),
        HirAssignOp::Sub => Ok(MirDynamicBinaryOp::Subtract),
        HirAssignOp::Mul => Ok(MirDynamicBinaryOp::Multiply),
        HirAssignOp::Div => Ok(MirDynamicBinaryOp::Divide),
        HirAssignOp::Rem => Ok(MirDynamicBinaryOp::Remainder),
        HirAssignOp::Set => Err(builder.inconsistent(
            origin,
            "set assignment reached compound dynamic operation lowering",
        )),
    }
}

const fn unit() -> MirOperand {
    MirOperand::Immediate(MirImmediate::Unit)
}
