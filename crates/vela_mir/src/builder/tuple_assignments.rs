use vela_analysis::type_fact::TypeFact;
use vela_hir::body::HirExprKind;
use vela_hir::ids::HirExprId;

use crate::{
    MirAggregate, MirBuildError, MirEffect, MirImmediate, MirOperand, MirPlace, MirSafepoint,
    MirSourceOrigin, MirStatement, MirStatementKind, MirValueType,
};

use super::core::{FunctionBuilder, value_type};

#[derive(Clone, Debug)]
pub(super) struct PreparedTupleProjection {
    expression: HirExprId,
    index: u32,
    arity: u32,
    elements: Vec<TypeFact>,
    origin: MirSourceOrigin,
}

impl FunctionBuilder<'_> {
    pub(super) fn prepare_tuple_assignment_projection(
        &self,
        expression: HirExprId,
        index: u32,
        origin: MirSourceOrigin,
    ) -> Result<PreparedTupleProjection, MirBuildError> {
        let record = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(
                origin,
                format!("missing tuple projection expression {expression:?}"),
            )
        })?;
        let HirExprKind::Field(field) = &record.kind else {
            return Err(self.inconsistent(
                origin,
                format!("tuple projection target {expression:?} is not a HIR field"),
            ));
        };
        let hir_index = field.name.parse::<u32>().map_err(|_| {
            self.inconsistent(
                origin,
                format!(
                    "tuple projection member {:?} is not a numeric index",
                    field.name
                ),
            )
        })?;
        if hir_index != index {
            return Err(self.inconsistent(
                origin,
                format!(
                    "tuple projection compile target index {index} disagrees with HIR member {hir_index}"
                ),
            ));
        }
        let elements = match self.input.analysis().expression(field.receiver) {
            Some(TypeFact::Tuple { elements }) => elements.clone(),
            Some(fact) => {
                return Err(self.inconsistent(
                    origin,
                    format!(
                        "tuple projection receiver {:?} requires an exact tuple analysis fact, got {fact:?}",
                        field.receiver
                    ),
                ));
            }
            None => {
                return Err(self.inconsistent(
                    origin,
                    format!(
                        "tuple projection receiver {:?} has no analysis type fact",
                        field.receiver
                    ),
                ));
            }
        };
        let arity = u32::try_from(elements.len())
            .map_err(|_| self.inconsistent(origin, "tuple assignment arity exceeds u32"))?;
        if index >= arity {
            return Err(self.inconsistent(
                origin,
                format!("tuple projection index {index} is out of range for arity {arity}"),
            ));
        }
        Ok(PreparedTupleProjection {
            expression,
            index,
            arity,
            elements,
            origin,
        })
    }

    pub(super) fn append_tuple_assignment_read(
        &mut self,
        tuple: MirOperand,
        projection: &PreparedTupleProjection,
    ) -> Result<MirOperand, MirBuildError> {
        let fact = projection
            .elements
            .get(usize::try_from(projection.index).map_err(|_| {
                self.inconsistent(projection.origin, "tuple projection index exceeds usize")
            })?)
            .ok_or_else(|| {
                self.inconsistent(
                    projection.origin,
                    format!(
                        "tuple projection {:?} lost element {} of arity {}",
                        projection.expression, projection.index, projection.arity
                    ),
                )
            })?;
        self.append_tuple_element_read(tuple, projection.index, fact, projection.origin)
    }

    pub(super) fn rebuild_tuple_assignment(
        &mut self,
        tuple: MirOperand,
        replacement: MirOperand,
        projection: &PreparedTupleProjection,
    ) -> Result<MirOperand, MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(MirOperand::Immediate(MirImmediate::Unit));
        }
        let mut values = Vec::with_capacity(projection.elements.len());
        for (index, fact) in projection.elements.iter().enumerate() {
            let index = u32::try_from(index).map_err(|_| {
                self.inconsistent(projection.origin, "tuple rebuild index exceeds u32")
            })?;
            if index == projection.index {
                values.push(replacement.clone());
            } else {
                values.push(self.append_tuple_element_read(
                    tuple.clone(),
                    index,
                    fact,
                    projection.origin,
                )?);
            }
        }
        let destination = self
            .function
            .add_temp(MirValueType::Tuple(projection.arity), projection.origin);
        let safepoint = self
            .function
            .add_safepoint(MirSafepoint::new(projection.origin));
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                projection.origin,
                Some(MirPlace::temp(destination)),
                MirStatementKind::Allocate(MirAggregate::Tuple(values)),
                MirEffect::allocation(),
                Some(safepoint),
            ),
        )?;
        Ok(MirOperand::Temp(destination))
    }

    fn append_tuple_element_read(
        &mut self,
        tuple: MirOperand,
        index: u32,
        fact: &TypeFact,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let destination = self.function.add_temp(value_type(Some(fact)), origin);
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(destination)),
                MirStatementKind::TupleField { tuple, index },
                MirEffect::may_trap(),
                None,
            ),
        )?;
        Ok(MirOperand::Temp(destination))
    }
}
