use super::*;
use crate::compiler::host_paths::{DynamicHostPathPart, HostPath, HostPathPart, HostPathRoot};
use vela_mir::{
    CompileCallArguments, CompileCalleeTarget, CompileHostPathSegment, CompileHostPathTarget,
    CompileParameter, CompileParameterDefault, CompilePlacedCallArgument, CompilePlacedCallValue,
    CompileSignature, MethodExecutableTarget,
};

pub(super) struct HirMethodArguments<'a> {
    pub(super) call: HirExprId,
    pub(super) target: HirMethodCallee,
    pub(super) method: &'a str,
    pub(super) receiver_type: Option<&'a RuntimeTypeFact>,
    pub(super) receiver_shape: Option<&'a ValueShape>,
    pub(super) signature: CompileSignature,
    pub(super) params: &'a [ParamHint],
    pub(super) preserve_missing_defaults: bool,
}

struct HirMethodArgument<'a> {
    method: &'a str,
    receiver_type: Option<&'a RuntimeTypeFact>,
    receiver_shape: Option<&'a ValueShape>,
    neutral_param: Option<&'a CompileParameter>,
    legacy_param: Option<&'a ParamHint>,
    index: usize,
    expression: HirExprId,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum HirMethodCallee {
    Script(MethodExecutableTarget),
    Value {
        owner: vela_def::TypeId,
        method: vela_def::MethodId,
    },
}

impl Compiler<'_, '_> {
    pub(super) fn hir_direct_lambda_body(&self, expression: HirExprId) -> Option<HirBodyId> {
        match self.hir_expression_record(expression).ok()?.1 {
            HirExprKind::Lambda { body } => Some(body),
            HirExprKind::Paren {
                expression: Some(inner),
            } => self.hir_direct_lambda_body(inner),
            _ => None,
        }
    }

    pub(super) fn require_direct_host_path_target(
        &self,
        call: HirExprId,
        receiver: HirExprId,
        direct: &HostPath,
        call_target: &CompileHostPathTarget,
    ) -> CompileResult<()> {
        let placed = self.placed_host_path_target(receiver)?;
        if placed != *call_target {
            return Err(self.compile_target_input_error(
                call,
                "host call path disagrees with the receiver host-path placement",
            ));
        }
        let HostPathRoot::LocalPath {
            expression: root, ..
        } = &direct.root;
        if *root != placed.root
            || self.host_path_root_type(direct.root.clone()) != placed.root_type.runtime
            || direct.segments.len() != placed.segments.len()
            || direct
                .segments
                .iter()
                .zip(&placed.segments)
                .any(|(direct, placed)| !direct_host_segment_matches(direct, placed))
        {
            return Err(self.compile_target_input_error(
                call,
                "direct host path disagrees with its compile-target placement",
            ));
        }
        Ok(())
    }

    pub(in crate::compiler) fn compile_placed_call_sources(
        &mut self,
        call: HirExprId,
        evaluation_order: &[HirExprId],
        slots: &[CompilePlacedCallArgument],
        mut compile: impl FnMut(&mut Self, usize, HirExprId) -> CompileResult<(Register, bool)>,
    ) -> CompileResult<Vec<(Register, bool)>> {
        let mut compiled = vec![None; evaluation_order.len()];
        for (source_index, expression) in evaluation_order.iter().copied().enumerate() {
            let (parameter, value) = slots
                .iter()
                .find_map(|slot| match slot.value {
                    CompilePlacedCallValue::Explicit {
                        source_index: candidate,
                        value,
                    } if usize::try_from(candidate) == Ok(source_index) => {
                        Some((slot.parameter, value))
                    }
                    CompilePlacedCallValue::Explicit { .. }
                    | CompilePlacedCallValue::MissingDefault => None,
                })
                .ok_or_else(|| {
                    self.compile_target_input_error(
                        call,
                        "placed call source is not referenced by a parameter slot",
                    )
                })?;
            if value != expression {
                return Err(self.compile_target_input_error(
                    call,
                    "placed call source value disagrees with evaluation order",
                ));
            }
            let parameter = usize::try_from(parameter).map_err(|_| {
                self.compile_target_input_error(call, "placed call parameter exceeds usize")
            })?;
            compiled[source_index] = Some(compile(self, parameter, expression)?);
        }
        compiled
            .into_iter()
            .map(|value| {
                value.ok_or_else(|| {
                    self.compile_target_input_error(
                        call,
                        "placed call source was not compiled exactly once",
                    )
                })
            })
            .collect()
    }

    pub(in crate::compiler) fn project_external_registers(
        &self,
        call: HirExprId,
        slots: &[CompilePlacedCallArgument],
        source_registers: &[(Register, bool)],
    ) -> CompileResult<Vec<Register>> {
        let mut arguments = Vec::with_capacity(slots.len());
        for (index, slot) in slots.iter().enumerate() {
            if usize::try_from(slot.parameter) != Ok(index) {
                return Err(self.compile_target_input_error(
                    call,
                    "external parameter slots are not contiguous",
                ));
            }
            let CompilePlacedCallValue::Explicit { source_index, .. } = slot.value else {
                continue;
            };
            let source_index = usize::try_from(source_index).map_err(|_| {
                self.compile_target_input_error(call, "external source index exceeds usize")
            })?;
            let register = source_registers
                .get(source_index)
                .map(|(register, _)| *register)
                .ok_or_else(|| {
                    self.compile_target_input_error(call, "external source index is out of bounds")
                })?;
            arguments.push(register);
        }
        Ok(arguments)
    }

    pub(super) fn validate_external_missing_slots(
        &self,
        call: HirExprId,
        signature: &CompileSignature,
        slots: &[CompilePlacedCallArgument],
    ) -> CompileResult<()> {
        if signature.parameters.len() != slots.len() {
            return Err(self.compile_target_input_error(
                call,
                "external placement disagrees with the compile-target signature",
            ));
        }
        for (index, (parameter, slot)) in signature.parameters.iter().zip(slots).enumerate() {
            if usize::try_from(slot.parameter) != Ok(index) {
                return Err(self.compile_target_input_error(
                    call,
                    "external parameter slots are not contiguous",
                ));
            }
            if matches!(slot.value, CompilePlacedCallValue::MissingDefault)
                && matches!(parameter.default, CompileParameterDefault::Required)
            {
                return Err(self.compile_target_input_error(
                    call,
                    "required external parameter is represented as missing",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn compile_hir_method_arguments(
        &mut self,
        request: HirMethodArguments<'_>,
    ) -> CompileResult<Vec<CallArgument>> {
        let HirMethodArguments {
            call,
            target: expected_target,
            method,
            receiver_type,
            receiver_shape,
            signature,
            params,
            preserve_missing_defaults,
        } = request;
        let target = self.placed_call_target(call)?;
        let callee_matches = match (expected_target, &target.callee) {
            (
                HirMethodCallee::Script(expected),
                CompileCalleeTarget::ScriptMethod { target, .. },
            ) => *target == expected,
            (
                HirMethodCallee::Value {
                    owner: expected_owner,
                    method: expected_method,
                },
                CompileCalleeTarget::ValueMethod { owner, method, .. },
            ) => *owner == expected_owner && *method == expected_method,
            (HirMethodCallee::Script(_), _) | (HirMethodCallee::Value { .. }, _) => false,
        };
        if !callee_matches {
            return Err(self.compile_target_input_error(
                call,
                format!(
                    "method call target {:?} disagrees with direct selection {expected_target:?}",
                    target.callee
                ),
            ));
        }
        let argument_kind_matches = matches!(
            (expected_target, &target.arguments),
            (
                HirMethodCallee::Script(_),
                CompileCallArguments::Script { .. }
            ) | (
                HirMethodCallee::Value { .. },
                CompileCallArguments::Positional(_) | CompileCallArguments::ExternalNamed { .. }
            )
        );
        if !argument_kind_matches {
            return Err(self.compile_target_input_error(
                call,
                "method argument placement disagrees with its compile-target family",
            ));
        }

        if !params.is_empty()
            && (params.len() != signature.parameters.len()
                || params
                    .iter()
                    .zip(&signature.parameters)
                    .any(|(legacy, neutral)| legacy.name != neutral.name))
        {
            return Err(self.compile_target_input_error(
                call,
                "legacy method parameter hints disagree with the compile-target signature",
            ));
        }

        match target.arguments {
            CompileCallArguments::Positional(evaluation_order) => evaluation_order
                .into_iter()
                .enumerate()
                .map(|(index, expression)| {
                    self.compile_hir_method_argument(HirMethodArgument {
                        method,
                        receiver_type,
                        receiver_shape,
                        neutral_param: signature.parameters.get(index),
                        legacy_param: params.get(index),
                        index,
                        expression,
                    })
                    .map(CallArgument::Register)
                })
                .collect(),
            CompileCallArguments::Script {
                evaluation_order,
                parameter_slots,
            }
            | CompileCallArguments::ExternalNamed {
                evaluation_order,
                parameter_slots,
            } => {
                if parameter_slots.len() != signature.parameters.len() {
                    return Err(self.compile_target_input_error(
                        call,
                        "method placement disagrees with the compile-target signature",
                    ));
                }
                let source_registers = self.compile_placed_call_sources(
                    call,
                    &evaluation_order,
                    &parameter_slots,
                    |compiler, parameter, expression| {
                        compiler
                            .compile_hir_method_argument(HirMethodArgument {
                                method,
                                receiver_type,
                                receiver_shape,
                                neutral_param: signature.parameters.get(parameter),
                                legacy_param: params.get(parameter),
                                index: parameter,
                                expression,
                            })
                            .map(|register| (register, false))
                    },
                )?;
                let mut arguments = Vec::with_capacity(parameter_slots.len());
                for (index, slot) in parameter_slots.iter().enumerate() {
                    if usize::try_from(slot.parameter) != Ok(index) {
                        return Err(self.compile_target_input_error(
                            call,
                            "method parameter slots are not contiguous",
                        ));
                    }
                    match slot.value {
                        CompilePlacedCallValue::Explicit { source_index, .. } => {
                            let source_index = usize::try_from(source_index).map_err(|_| {
                                self.compile_target_input_error(
                                    call,
                                    "method source index exceeds usize",
                                )
                            })?;
                            let register = source_registers
                                .get(source_index)
                                .map(|(register, _)| *register)
                                .ok_or_else(|| {
                                    self.compile_target_input_error(
                                        call,
                                        "method source index is out of bounds",
                                    )
                                })?;
                            arguments.push(CallArgument::Register(register));
                        }
                        CompilePlacedCallValue::MissingDefault => {
                            let parameter = signature.parameters.get(index).ok_or_else(|| {
                                self.compile_target_input_error(
                                    call,
                                    "missing method slot exceeds the compile-target signature",
                                )
                            })?;
                            if matches!(parameter.default, CompileParameterDefault::Required) {
                                return Err(self.compile_target_input_error(
                                    call,
                                    "required method parameter is represented as missing",
                                ));
                            }
                            if preserve_missing_defaults {
                                arguments.push(CallArgument::Missing);
                            }
                        }
                    }
                }
                Ok(arguments)
            }
            CompileCallArguments::Dynamic(_) => Err(self.compile_target_input_error(
                call,
                "static method call owns dynamic argument placement",
            )),
        }
    }

    fn compile_hir_method_argument(
        &mut self,
        request: HirMethodArgument<'_>,
    ) -> CompileResult<Register> {
        let HirMethodArgument {
            method,
            receiver_type,
            receiver_shape,
            neutral_param,
            legacy_param,
            index,
            expression,
        } = request;
        let Some(neutral_param) = neutral_param else {
            return self.compile_hir_expression(expression);
        };
        let callback_shapes = self.hir_callback_param_shapes(receiver_shape, method, expression);
        let expected = typed_container_mutation_arg_contract(
            receiver_type,
            method,
            &neutral_param.name,
            index,
        );
        if let Some(expected) = expected {
            return self
                .compile_hir_expression_for_expected_type(
                    expression,
                    expected,
                    TypeContractContext::NativeParameter {
                        function: method.to_owned(),
                        name: mutation_arg_debug_name(method, &neutral_param.name, index),
                        index: u16::try_from(index).unwrap_or(u16::MAX),
                    },
                    callback_shapes.as_deref().unwrap_or(&[]),
                )
                .map(|(register, _)| register);
        }
        let Some(param) = legacy_param else {
            return self.compile_hir_expression(expression);
        };
        self.compile_hir_argument_for_expected_param(
            method,
            index,
            expression,
            param,
            callback_shapes.as_deref().unwrap_or(&[]),
            false,
        )
        .map(|(register, _)| register)
    }
}

fn direct_host_segment_matches(direct: &HostPathPart, placed: &CompileHostPathSegment) -> bool {
    match (direct, placed) {
        (HostPathPart::Field(direct), CompileHostPathSegment::Field(placed)) => {
            *direct == placed.runtime
        }
        (HostPathPart::VariantField(direct), CompileHostPathSegment::VariantField(placed)) => {
            *direct == placed.runtime
        }
        (
            HostPathPart::DynamicValue {
                expression: direct,
                dynamic_kind: DynamicHostPathPart::Index,
            },
            CompileHostPathSegment::DynamicIndex {
                expression: placed, ..
            },
        )
        | (
            HostPathPart::DynamicValue {
                expression: direct,
                dynamic_kind: DynamicHostPathPart::Key,
            },
            CompileHostPathSegment::DynamicKey {
                expression: placed, ..
            },
        ) => direct == placed,
        (
            HostPathPart::DynamicValue {
                dynamic_kind: DynamicHostPathPart::Index,
                ..
            },
            CompileHostPathSegment::ConstantIndex { .. },
        )
        | (
            HostPathPart::DynamicValue {
                dynamic_kind: DynamicHostPathPart::Key,
                ..
            },
            CompileHostPathSegment::ConstantKey { .. },
        ) => true,
        (HostPathPart::Field(_), _)
        | (HostPathPart::VariantField(_), _)
        | (HostPathPart::DynamicValue { .. }, _) => false,
    }
}
