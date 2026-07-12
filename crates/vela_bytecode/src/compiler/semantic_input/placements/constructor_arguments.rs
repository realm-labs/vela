use vela_analysis::semantic_facts::ConstructorTargetFact;
use vela_analysis::validation::{
    CallArgumentPlacementFact, ConstructorFieldSlotFact, ConstructorInputKindFact,
    ConstructorPlacementFact, ConstructorSlotValueFact,
};
use vela_hir::body::{HirArgument, HirRecordField};
use vela_hir::ids::HirExprId;
use vela_mir::{MirBuildError, MirSourceOrigin};

use super::{CompileError, CompileResult, FunctionId, GenerationBuilder, input_error};

impl GenerationBuilder<'_, '_> {
    pub(super) fn checked_record_constructor_placement(
        &self,
        executable: FunctionId,
        expression: HirExprId,
        fields: &[HirRecordField],
        origin: MirSourceOrigin,
    ) -> CompileResult<ConstructorPlacementFact> {
        let placement = self.checked_constructor_placement(executable, expression, origin)?;
        if placement.input_kind != ConstructorInputKindFact::RecordFields {
            return Err(constructor_placement_error(
                origin,
                "analysis constructor input kind is not record fields",
            ));
        }
        if placement.source_order.len() != fields.len() {
            return Err(constructor_placement_error(
                origin,
                "analysis constructor source order does not cover every HIR field",
            ));
        }
        for (index, (source, field)) in placement.source_order.iter().zip(fields).enumerate() {
            if source.source_index != index
                || source.name.as_deref() != Some(field.name.as_str())
                || source.value != field.value
                || source.span != field.name_origin.span
            {
                return Err(constructor_placement_error(
                    origin,
                    "analysis constructor source order disagrees with HIR record fields",
                ));
            }
        }
        self.require_constructor_source_values(&placement, origin)?;
        Ok(placement)
    }

    pub(super) fn checked_tuple_constructor_placement(
        &self,
        executable: FunctionId,
        expression: HirExprId,
        arguments: &[HirArgument],
        call_placement: &CallArgumentPlacementFact,
        expected_target: &ConstructorTargetFact,
        origin: MirSourceOrigin,
    ) -> CompileResult<ConstructorPlacementFact> {
        let placement = self.checked_constructor_placement(executable, expression, origin)?;
        if placement.input_kind != ConstructorInputKindFact::TupleArguments {
            return Err(constructor_placement_error(
                origin,
                "analysis constructor input kind is not tuple arguments",
            ));
        }
        if &placement.target != expected_target {
            return Err(constructor_placement_error(
                origin,
                "analysis constructor placement target disagrees with the call target",
            ));
        }
        if placement.source_order.len() != arguments.len()
            || placement.source_order.len() != call_placement.source_order.len()
        {
            return Err(constructor_placement_error(
                origin,
                "analysis tuple constructor source order does not cover every call argument",
            ));
        }
        for (index, ((source, argument), call_source)) in placement
            .source_order
            .iter()
            .zip(arguments)
            .zip(&call_placement.source_order)
            .enumerate()
        {
            if source.source_index != index
                || source.name != argument.name
                || source.value != argument.value
                || source.span != argument.origin.span
                || source.source_index != call_source.source_index
                || source.name != call_source.name
                || source.value != call_source.value
                || source.span != call_source.span
            {
                return Err(constructor_placement_error(
                    origin,
                    "analysis tuple constructor placement disagrees with call placement",
                ));
            }
        }
        self.require_constructor_source_values(&placement, origin)?;
        Ok(placement)
    }

    pub(super) fn constructor_slots(
        &self,
        placement: &ConstructorPlacementFact,
        expected: usize,
        origin: MirSourceOrigin,
    ) -> CompileResult<Vec<ConstructorFieldSlotFact>> {
        let slots = placement.declaration_slots.as_ref().ok_or_else(|| {
            constructor_placement_error(
                origin,
                "analysis constructor placement is missing declaration slots",
            )
        })?;
        if slots.len() != expected {
            return Err(constructor_placement_error(
                origin,
                "analysis constructor slots do not cover the declaration shape",
            ));
        }
        Ok(slots.clone())
    }

    pub(super) fn constructor_evaluation_order(
        &self,
        placement: &ConstructorPlacementFact,
        origin: MirSourceOrigin,
    ) -> CompileResult<Vec<HirExprId>> {
        self.require_constructor_source_values(placement, origin)
    }

    pub(super) fn constructor_explicit_value(
        &self,
        placement: &ConstructorPlacementFact,
        source_index: usize,
        value: Option<HirExprId>,
        origin: MirSourceOrigin,
    ) -> CompileResult<HirExprId> {
        let source = placement.source_order.get(source_index).ok_or_else(|| {
            constructor_placement_error(
                origin,
                "analysis constructor slot references an unknown source value",
            )
        })?;
        let source_value = source.value.ok_or_else(|| {
            constructor_placement_error(
                MirSourceOrigin {
                    span: source.span,
                    ..origin
                },
                "validated constructor source is missing its HIR expression",
            )
        })?;
        if value != Some(source_value) {
            return Err(constructor_placement_error(
                origin,
                "analysis constructor slot value disagrees with source evaluation order",
            ));
        }
        Ok(source_value)
    }

    fn checked_constructor_placement(
        &self,
        executable: FunctionId,
        expression: HirExprId,
        origin: MirSourceOrigin,
    ) -> CompileResult<ConstructorPlacementFact> {
        self.executable_analysis(executable)?
            .constructor_placement(expression)
            .cloned()
            .ok_or_else(|| {
                constructor_placement_error(origin, "missing analysis constructor placement")
            })
    }

    fn require_constructor_source_values(
        &self,
        placement: &ConstructorPlacementFact,
        origin: MirSourceOrigin,
    ) -> CompileResult<Vec<HirExprId>> {
        placement
            .source_order
            .iter()
            .map(|source| {
                source.value.ok_or_else(|| {
                    constructor_placement_error(
                        MirSourceOrigin {
                            span: source.span,
                            ..origin
                        },
                        "validated constructor source is missing its HIR expression",
                    )
                })
            })
            .collect()
    }
}

pub(super) fn require_constructor_slot_identity(
    slot: &ConstructorFieldSlotFact,
    declaration_index: usize,
    field_name: &str,
    parameter_name: &str,
    origin: MirSourceOrigin,
) -> CompileResult<()> {
    if slot.declaration_index != declaration_index
        || slot.field_name != field_name
        || slot.parameter_name != parameter_name
    {
        return Err(constructor_placement_error(
            origin,
            "analysis constructor slot disagrees with the declaration shape",
        ));
    }
    Ok(())
}

pub(super) fn unavailable_constructor_default(
    value: &ConstructorSlotValueFact,
    origin: MirSourceOrigin,
) -> CompileError {
    let message = match value {
        ConstructorSlotValueFact::SourceDefaultUnavailable { body: Some(body) } => {
            format!("constructor source default {body:?} is unavailable")
        }
        ConstructorSlotValueFact::SourceDefaultUnavailable { body: None } => {
            "constructor source default is unavailable".to_owned()
        }
        ConstructorSlotValueFact::RegisteredDefaultUnavailable => {
            "registered constructor default is unavailable at compile time".to_owned()
        }
        ConstructorSlotValueFact::Explicit { .. }
        | ConstructorSlotValueFact::SourceDefault { .. } => {
            "constructor default availability is inconsistent".to_owned()
        }
    };
    constructor_placement_error(origin, message)
}

fn constructor_placement_error(
    origin: MirSourceOrigin,
    message: impl Into<String>,
) -> CompileError {
    input_error(MirBuildError::InconsistentInput {
        origin,
        message: message.into(),
    })
}
