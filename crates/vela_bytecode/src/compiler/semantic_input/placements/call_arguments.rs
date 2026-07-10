use vela_analysis::validation::{
    CallArgumentPlacementFact, CallParameterSlotFact, CallParameterSlotValueFact,
    CallPlacementModeFact,
};
use vela_hir::body::HirCall;
use vela_hir::ids::{HirExprId, ModuleId};
use vela_hir::type_hint::ParamHint;
use vela_mir::{
    CompileDynamicCallArgument, CompilePlacedCallArgument, MirBuildError, MirSourceOrigin,
};

use super::*;

impl GenerationBuilder<'_, '_> {
    pub(super) fn checked_call_placement(
        &self,
        executable: FunctionId,
        call: &HirCall,
        origin: MirSourceOrigin,
    ) -> CompileResult<CallArgumentPlacementFact> {
        let placement = self
            .executable_analysis(executable)?
            .call_argument_placement(call.expression)
            .cloned()
            .ok_or_else(|| placement_error(origin, "missing analysis call argument placement"))?;
        if placement.source_order.len() != call.arguments.len() {
            return Err(placement_error(
                origin,
                "analysis call source order does not cover every HIR argument",
            ));
        }
        for (index, (source, argument)) in placement
            .source_order
            .iter()
            .zip(&call.arguments)
            .enumerate()
        {
            if source.source_index != index
                || source.name != argument.name
                || source.value != argument.value
                || source.span != argument.origin.span
            {
                return Err(placement_error(
                    origin,
                    "analysis call source order disagrees with HIR arguments",
                ));
            }
            if source.value.is_none() {
                return Err(placement_error(
                    MirSourceOrigin {
                        span: source.span,
                        ..origin
                    },
                    "validated call argument is missing its HIR expression",
                ));
            }
        }
        Ok(placement)
    }

    pub(super) fn positional_argument_values(
        &self,
        placement: &CallArgumentPlacementFact,
        mode: CallPlacementModeFact,
        origin: MirSourceOrigin,
    ) -> CompileResult<Vec<HirExprId>> {
        require_mode(placement, mode, origin)?;
        source_values(placement, origin)
    }

    pub(super) fn dynamic_argument_values(
        &self,
        placement: &CallArgumentPlacementFact,
        origin: MirSourceOrigin,
    ) -> CompileResult<Vec<CompileDynamicCallArgument>> {
        require_mode(placement, CallPlacementModeFact::Dynamic, origin)?;
        placement
            .source_order
            .iter()
            .map(|argument| {
                Ok(CompileDynamicCallArgument {
                    name: argument.name.clone(),
                    value: source_value(argument.value, origin)?,
                })
            })
            .collect()
    }

    pub(super) fn script_argument_slots(
        &mut self,
        executable: FunctionId,
        placement: &CallArgumentPlacementFact,
        params: &[ParamHint],
        module: ModuleId,
        origin: MirSourceOrigin,
    ) -> CompileResult<(Vec<HirExprId>, Vec<CompilePlacedCallArgument>)> {
        let slots = self.strict_parameter_slots(placement, params, origin)?;
        let parameter_slots = slots
            .iter()
            .zip(params)
            .enumerate()
            .map(|(index, (slot, parameter))| {
                require_slot_identity(
                    slot.parameter_index,
                    &slot.name,
                    index,
                    &parameter.name,
                    origin,
                )?;
                let value = match &slot.value {
                    CallParameterSlotValueFact::Explicit {
                        source_index,
                        value,
                    } => {
                        let value = checked_slot_value(placement, *source_index, *value, origin)?;
                        if let Some(hint) = &parameter.type_hint
                            && let Some(contract) = self.type_contract_for_hint(module, hint)
                        {
                            self.boundaries.push(ContractBoundary::function_parameter(
                                executable,
                                value,
                                contract,
                                parameter.name.clone(),
                            ));
                        }
                        CompilePlacedCallArgument::placed(
                            checked_u32(index, origin, "script call parameter")?,
                            checked_u32(*source_index, origin, "script call source argument")?,
                            value,
                        )
                    }
                    CallParameterSlotValueFact::MissingDefault => {
                        if parameter.default_value_span.is_none()
                            && parameter.default_body.is_none()
                        {
                            return Err(placement_error(
                                origin,
                                "analysis omitted a required script parameter",
                            ));
                        }
                        CompilePlacedCallArgument::missing(checked_u32(
                            index,
                            origin,
                            "script call parameter",
                        )?)
                    }
                };
                Ok(value)
            })
            .collect::<CompileResult<Vec<_>>>()?;
        Ok((source_values(placement, origin)?, parameter_slots))
    }

    pub(super) fn strict_parameter_slots(
        &self,
        placement: &CallArgumentPlacementFact,
        params: &[ParamHint],
        origin: MirSourceOrigin,
    ) -> CompileResult<Vec<CallParameterSlotFact>> {
        require_mode(placement, CallPlacementModeFact::Strict, origin)?;
        let slots = parameter_slots(placement, params.len(), origin)?;
        for (index, (slot, parameter)) in slots.iter().zip(params).enumerate() {
            require_slot_identity(
                slot.parameter_index,
                &slot.name,
                index,
                &parameter.name,
                origin,
            )?;
        }
        Ok(slots.to_vec())
    }

    pub(super) fn source_argument_values(
        &self,
        placement: &CallArgumentPlacementFact,
        origin: MirSourceOrigin,
    ) -> CompileResult<Vec<HirExprId>> {
        source_values(placement, origin)
    }

    pub(super) fn external_call_target(
        &mut self,
        executable: FunctionId,
        callee: CompileCalleeTarget,
        debug_function: &str,
        params: &[vela_registry::ParamDef],
        placement: &CallArgumentPlacementFact,
        origin: MirSourceOrigin,
    ) -> CompileResult<CompileCallTarget> {
        match placement.mode {
            CallPlacementModeFact::ExternalPositional => {
                let values = source_values(placement, origin)?;
                for (index, (value, parameter)) in values.iter().zip(params).enumerate() {
                    self.push_external_boundary(
                        executable,
                        debug_function,
                        parameter,
                        index,
                        *value,
                        origin,
                    )?;
                }
                Ok(CompileCallTarget::positional(callee, values))
            }
            CallPlacementModeFact::ExternalNamed => {
                let slots = parameter_slots(placement, params.len(), origin)?;
                let mut parameter_slots = Vec::with_capacity(slots.len());
                for (index, (slot, parameter)) in slots.iter().zip(params).enumerate() {
                    require_slot_identity(
                        slot.parameter_index,
                        &slot.name,
                        index,
                        &parameter.name,
                        origin,
                    )?;
                    match &slot.value {
                        CallParameterSlotValueFact::Explicit {
                            source_index,
                            value,
                        } => {
                            let value =
                                checked_slot_value(placement, *source_index, *value, origin)?;
                            self.push_external_boundary(
                                executable,
                                debug_function,
                                parameter,
                                index,
                                value,
                                origin,
                            )?;
                            parameter_slots.push(CompilePlacedCallArgument::placed(
                                checked_u32(index, origin, "external call parameter")?,
                                checked_u32(
                                    *source_index,
                                    origin,
                                    "external call source argument",
                                )?,
                                value,
                            ));
                        }
                        CallParameterSlotValueFact::MissingDefault if parameter.has_default => {
                            parameter_slots.push(CompilePlacedCallArgument::missing(checked_u32(
                                index,
                                origin,
                                "external call parameter",
                            )?));
                        }
                        CallParameterSlotValueFact::MissingDefault => {
                            return Err(placement_error(
                                origin,
                                "analysis omitted a required external parameter",
                            ));
                        }
                    }
                }
                Ok(CompileCallTarget::external_named(
                    callee,
                    source_values(placement, origin)?,
                    parameter_slots,
                ))
            }
            _ => Err(placement_error(
                origin,
                "analysis call placement mode does not match an external call",
            )),
        }
    }

    fn push_external_boundary(
        &mut self,
        executable: FunctionId,
        debug_function: &str,
        parameter: &vela_registry::ParamDef,
        index: usize,
        value: HirExprId,
        origin: MirSourceOrigin,
    ) -> CompileResult<()> {
        if let Some(contract) = parameter
            .type_hint
            .as_ref()
            .and_then(|hint| registry_hint_contract(hint, &self.catalog))
            .and_then(super::super::schema::meaningful_contract)
        {
            self.boundaries.push(ContractBoundary::native_parameter(
                executable,
                value,
                contract,
                debug_function,
                &parameter.name,
                checked_u16(index, origin, "native parameter index")?,
            ));
        }
        Ok(())
    }
}

fn parameter_slots(
    placement: &CallArgumentPlacementFact,
    expected: usize,
    origin: MirSourceOrigin,
) -> CompileResult<&[vela_analysis::validation::CallParameterSlotFact]> {
    let slots = placement.parameter_slots.as_deref().ok_or_else(|| {
        placement_error(origin, "analysis call placement is missing parameter slots")
    })?;
    if slots.len() != expected {
        return Err(placement_error(
            origin,
            "analysis call parameter slots do not cover the callable signature",
        ));
    }
    Ok(slots)
}

fn require_slot_identity(
    actual_index: usize,
    actual_name: &str,
    index: usize,
    name: &str,
    origin: MirSourceOrigin,
) -> CompileResult<()> {
    if actual_index != index || actual_name != name {
        return Err(placement_error(
            origin,
            "analysis call parameter slot disagrees with the callable signature",
        ));
    }
    Ok(())
}

fn checked_slot_value(
    placement: &CallArgumentPlacementFact,
    source_index: usize,
    value: Option<HirExprId>,
    origin: MirSourceOrigin,
) -> CompileResult<HirExprId> {
    let source = placement.source_order.get(source_index).ok_or_else(|| {
        placement_error(
            origin,
            "analysis call slot references an unknown source argument",
        )
    })?;
    let source_value = source_value(source.value, origin)?;
    if value != Some(source_value) {
        return Err(placement_error(
            origin,
            "analysis call slot value disagrees with source evaluation order",
        ));
    }
    Ok(source_value)
}

fn source_values(
    placement: &CallArgumentPlacementFact,
    origin: MirSourceOrigin,
) -> CompileResult<Vec<HirExprId>> {
    placement
        .source_order
        .iter()
        .map(|argument| source_value(argument.value, origin))
        .collect()
}

fn source_value(value: Option<HirExprId>, origin: MirSourceOrigin) -> CompileResult<HirExprId> {
    value.ok_or_else(|| {
        placement_error(
            origin,
            "validated call argument is missing its HIR expression",
        )
    })
}

fn require_mode(
    placement: &CallArgumentPlacementFact,
    expected: CallPlacementModeFact,
    origin: MirSourceOrigin,
) -> CompileResult<()> {
    if placement.mode != expected {
        return Err(placement_error(
            origin,
            "analysis call placement mode does not match the resolved call target",
        ));
    }
    Ok(())
}

fn placement_error(origin: MirSourceOrigin, message: impl Into<String>) -> CompileError {
    input_error(MirBuildError::InconsistentInput {
        origin,
        message: message.into(),
    })
}
