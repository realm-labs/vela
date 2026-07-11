use vela_common::PrimitiveTag;

use crate::{
    MirBuildError, MirConstantProvenance, MirEffect, MirEvaluatedConstant, MirImmediate,
    MirOperand, MirPlace, MirRvalue, MirSafepoint, MirSourceOrigin, MirStatement, MirStatementKind,
    MirTypeContract, MirValueType,
};

use super::core::FunctionBuilder;

impl FunctionBuilder<'_> {
    /// Lower a value that was fully evaluated by the compile-time evaluator.
    ///
    /// Scalar values receive an explicit pure definition at every use.
    /// Heap-backed values are materialized at every runtime use so allocation,
    /// identity, GC, and budget behavior stay explicit in MIR.
    pub(super) fn lower_evaluated_constant(
        &mut self,
        value: MirEvaluatedConstant,
        value_type: MirValueType,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        match value {
            MirEvaluatedConstant::Unit => self.define_immediate_constant(
                MirImmediate::Unit,
                MirConstantProvenance::EvaluatedConstant,
                origin,
            ),
            MirEvaluatedConstant::Bool(value) => self.define_immediate_constant(
                MirImmediate::Bool(value),
                MirConstantProvenance::EvaluatedConstant,
                origin,
            ),
            MirEvaluatedConstant::Char(value) => self.define_immediate_constant(
                MirImmediate::Char(value),
                MirConstantProvenance::EvaluatedConstant,
                origin,
            ),
            MirEvaluatedConstant::Scalar(value) => self.define_immediate_constant(
                MirImmediate::Scalar(value),
                MirConstantProvenance::EvaluatedConstant,
                origin,
            ),
            heap_value => {
                let destination = self.function.add_temp(value_type, origin);
                let safepoint = self.function.add_safepoint(MirSafepoint::new(origin));
                self.function.append_statement(
                    self.current_block,
                    MirStatement::new(
                        origin,
                        Some(MirPlace::temp(destination)),
                        MirStatementKind::MaterializeConstant(heap_value),
                        MirEffect::allocation(),
                        Some(safepoint),
                    ),
                )?;
                Ok(MirOperand::Temp(destination))
            }
        }
    }

    pub(super) fn define_immediate_constant(
        &mut self,
        value: MirImmediate,
        provenance: MirConstantProvenance,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let destination = self.function.add_temp(value.value_type(), origin);
        self.function.append_statement(
            self.current_block,
            MirStatement::assign(
                origin,
                MirPlace::temp(destination),
                MirRvalue::Constant { value, provenance },
            ),
        )?;
        Ok(MirOperand::Temp(destination))
    }
}

pub(super) fn value_type_for_contract(contract: &MirTypeContract) -> MirValueType {
    match contract {
        MirTypeContract::Primitive(primitive) => MirValueType::Primitive(*primitive),
        MirTypeContract::Range => MirValueType::Range,
        MirTypeContract::Iterator(_) => MirValueType::Iterator,
        MirTypeContract::Tuple(elements) => {
            MirValueType::Tuple(u32::try_from(elements.len()).unwrap_or(u32::MAX))
        }
        MirTypeContract::Callable { .. } => MirValueType::Callable,
        MirTypeContract::Shape { type_id, shape } => MirValueType::ScriptType {
            type_id: *type_id,
            shape: *shape,
        },
        MirTypeContract::Variant { type_id, .. } => MirValueType::Enum(*type_id),
        MirTypeContract::Host(target) => MirValueType::Host(*target),
        MirTypeContract::Any
        | MirTypeContract::Array(_)
        | MirTypeContract::Map { .. }
        | MirTypeContract::Set(_)
        | MirTypeContract::Option(_)
        | MirTypeContract::Result { .. }
        | MirTypeContract::Definition(_) => MirValueType::Dynamic,
    }
}

pub(super) fn value_type_for_evaluated_constant(value: &MirEvaluatedConstant) -> MirValueType {
    match value {
        MirEvaluatedConstant::Unit => MirValueType::Unit,
        MirEvaluatedConstant::Bool(_) => MirValueType::Primitive(PrimitiveTag::Bool),
        MirEvaluatedConstant::Char(_) => MirValueType::Primitive(PrimitiveTag::Char),
        MirEvaluatedConstant::Scalar(value) => MirValueType::Primitive(value.primitive_tag()),
        MirEvaluatedConstant::String(_) => MirValueType::Primitive(PrimitiveTag::String),
        MirEvaluatedConstant::Bytes(_) => MirValueType::Primitive(PrimitiveTag::Bytes),
        MirEvaluatedConstant::Array(_) | MirEvaluatedConstant::Map(_) => MirValueType::Dynamic,
    }
}
