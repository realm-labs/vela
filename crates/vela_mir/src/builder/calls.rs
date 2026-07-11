use vela_hir::binding::BindingResolution;
use vela_hir::body::{HirCall, HirExprKind};
use vela_hir::ids::{HirBodyId, HirExprId, HirLocalId};

use crate::{
    CompileCallArguments, CompileCalleeTarget, CompileFunctionClass, CompileMethodClass,
    CompileParameterDefault, CompilePlacedCallArgument, CompilePlacedCallValue, CompileSignature,
    MirBuildError, MirCall, MirDynamicArgument, MirEffect, MirOperand, MirPlace, MirSafepoint,
    MirScriptArgument, MirScriptParameterGuardMode, MirSourceOrigin, MirStatement,
    MirStatementKind,
};

use super::core::{FunctionBuilder, value_type};

impl FunctionBuilder<'_> {
    /// Lower one call from the immutable compile-target generation.
    ///
    /// Runtime callees and method receivers are evaluated before source
    /// arguments. Source arguments are evaluated exactly once in source order,
    /// stabilized when necessary, and only then projected into parameter order.
    pub(super) fn lower_call(
        &mut self,
        expression: HirExprId,
        call: &HirCall,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        if call.expression != expression {
            return Err(self.inconsistent(
                origin,
                "call record expression identity disagrees with its HIR arena key",
            ));
        }
        let target = self
            .input
            .targets()
            .call(expression)
            .cloned()
            .ok_or_else(|| self.inconsistent(origin, "call expression has no compile target"))?;
        self.validate_source_arguments(call, &target.arguments, origin)?;

        if matches!(
            target.callee,
            CompileCalleeTarget::HostMethod(_)
                | CompileCalleeTarget::HostRemove { .. }
                | CompileCalleeTarget::HostPush { .. }
        ) {
            return self.lower_host_call(expression, call, &target, origin);
        }
        if matches!(target.callee, CompileCalleeTarget::Reflection { .. }) {
            return self.lower_reflection_call(expression, call, &target, origin);
        }

        let (call, effect) = match target.callee {
            CompileCalleeTarget::ScriptFunction {
                function,
                debug_name,
            } => {
                let descriptor = self
                    .input
                    .targets()
                    .function_descriptor(function)
                    .cloned()
                    .ok_or_else(|| {
                        self.inconsistent(origin, "script call target has no function descriptor")
                    })?;
                if descriptor.class != CompileFunctionClass::Script {
                    return Err(self.inconsistent(
                        origin,
                        "script call target descriptor has the wrong function class",
                    ));
                }
                let (arguments, parameter_guards) =
                    self.lower_script_arguments(&target.arguments, &descriptor.signature, origin)?;
                let effect = MirEffect::script_call().union(descriptor.signature.effect);
                (
                    MirCall::ScriptFunction {
                        function,
                        debug_name,
                        signature: descriptor.signature,
                        arguments,
                        parameter_guards,
                    },
                    effect,
                )
            }
            CompileCalleeTarget::ScriptMethod {
                target: method,
                debug_name,
            } => {
                let field = self.method_callee(call, &debug_name, origin)?;
                let receiver = self.lower_captured(field.receiver)?;
                let descriptor = self
                    .input
                    .targets()
                    .method_descriptor(method.owner, method.method)
                    .cloned()
                    .ok_or_else(|| {
                        self.inconsistent(origin, "script method call has no method descriptor")
                    })?;
                if !matches!(
                    descriptor.class,
                    CompileMethodClass::Script { executable, .. } if executable == method
                ) {
                    return Err(self.inconsistent(
                        origin,
                        "script method descriptor disagrees with its executable target",
                    ));
                }
                let (arguments, _) =
                    self.lower_script_arguments(&target.arguments, &descriptor.signature, origin)?;
                let effect = MirEffect::script_call().union(descriptor.signature.effect);
                (
                    MirCall::ScriptMethod {
                        target: method,
                        debug_name,
                        receiver,
                        signature: descriptor.signature,
                        arguments,
                    },
                    effect,
                )
            }
            CompileCalleeTarget::Local(local) => {
                if self.direct_local_callee(call.callee)? != Some(local) {
                    return Err(self.inconsistent(
                        origin,
                        "callable-local target disagrees with the HIR callee",
                    ));
                }
                let callee = self.lower_captured(call.callee)?;
                let arguments = self.lower_positional_arguments(&target.arguments, origin)?;
                (
                    MirCall::CallableValue { callee, arguments },
                    MirEffect::dynamic_call(),
                )
            }
            CompileCalleeTarget::Lambda(body) => {
                if self.direct_lambda_body(call.callee)? != Some(body) {
                    return Err(self
                        .inconsistent(origin, "lambda call target disagrees with the HIR callee"));
                }
                let callee = self.lower_captured(call.callee)?;
                let arguments = self.lower_positional_arguments(&target.arguments, origin)?;
                (
                    MirCall::CallableValue { callee, arguments },
                    MirEffect::dynamic_call(),
                )
            }
            CompileCalleeTarget::NativeFunction {
                function,
                debug_name,
            } => {
                let descriptor = self
                    .input
                    .targets()
                    .function_descriptor(function)
                    .cloned()
                    .ok_or_else(|| {
                        self.inconsistent(origin, "native call target has no function descriptor")
                    })?;
                if !matches!(
                    descriptor.class,
                    CompileFunctionClass::Native | CompileFunctionClass::Registry
                ) {
                    return Err(self.inconsistent(
                        origin,
                        "native call target descriptor has the wrong function class",
                    ));
                }
                let arguments = self.lower_external_arguments(
                    &target.arguments,
                    &descriptor.signature,
                    origin,
                )?;
                let effect = MirEffect::external_call().union(descriptor.signature.effect);
                (
                    MirCall::NativeFunction {
                        function,
                        debug_name,
                        signature: descriptor.signature,
                        arguments,
                    },
                    effect,
                )
            }
            CompileCalleeTarget::StdlibFunction {
                function,
                debug_name,
            } => {
                let descriptor = self
                    .input
                    .targets()
                    .function_descriptor(function)
                    .cloned()
                    .ok_or_else(|| {
                        self.inconsistent(origin, "stdlib call target has no function descriptor")
                    })?;
                if descriptor.class != CompileFunctionClass::Stdlib {
                    return Err(self.inconsistent(
                        origin,
                        "stdlib call target descriptor has the wrong function class",
                    ));
                }
                let arguments = self.lower_external_arguments(
                    &target.arguments,
                    &descriptor.signature,
                    origin,
                )?;
                let effect = MirEffect::external_call().union(descriptor.signature.effect);
                (
                    MirCall::StdlibFunction {
                        function,
                        debug_name,
                        signature: descriptor.signature,
                        arguments,
                    },
                    effect,
                )
            }
            CompileCalleeTarget::ValueMethod {
                owner,
                method,
                debug_name,
            } => {
                let field = self.method_callee(call, &debug_name, origin)?;
                let receiver = self.lower_captured(field.receiver)?;
                let descriptor = self
                    .input
                    .targets()
                    .method_descriptor(owner, method)
                    .cloned()
                    .ok_or_else(|| {
                        self.inconsistent(origin, "value method call has no method descriptor")
                    })?;
                if !matches!(
                    descriptor.class,
                    CompileMethodClass::Value | CompileMethodClass::Registry
                ) || descriptor.member_name != debug_name
                {
                    return Err(self.inconsistent(
                        origin,
                        "value method descriptor disagrees with the placed target",
                    ));
                }
                let arguments = self.lower_external_arguments(
                    &target.arguments,
                    &descriptor.signature,
                    origin,
                )?;
                let effect = MirEffect::external_call().union(descriptor.signature.effect);
                (
                    MirCall::ValueMethod {
                        owner,
                        method,
                        debug_name,
                        receiver,
                        signature: descriptor.signature,
                        arguments,
                    },
                    effect,
                )
            }
            CompileCalleeTarget::DynamicCallable => {
                if self.direct_local_callee(call.callee)?.is_some()
                    || self.direct_lambda_body(call.callee)?.is_some()
                {
                    return Err(self.inconsistent(
                        origin,
                        "dynamic callable target disagrees with the direct HIR callee",
                    ));
                }
                let callee = self.lower_captured(call.callee)?;
                let arguments = self.lower_dynamic_arguments(&target.arguments, origin)?;
                (
                    MirCall::DynamicCallable { callee, arguments },
                    MirEffect::dynamic_call(),
                )
            }
            CompileCalleeTarget::DynamicMethod(method) => {
                let field = self.method_callee(call, &method.member, origin)?;
                let receiver = self.lower_captured(field.receiver)?;
                let arguments = self.lower_dynamic_arguments(&target.arguments, origin)?;
                (
                    MirCall::DynamicMethod {
                        target: method,
                        receiver,
                        arguments,
                    },
                    MirEffect::dynamic_call(),
                )
            }
            CompileCalleeTarget::HostMethod(_)
            | CompileCalleeTarget::HostRemove { .. }
            | CompileCalleeTarget::HostPush { .. } => unreachable!("host calls route above"),
            CompileCalleeTarget::Reflection { .. } => {
                unreachable!("reflection calls route above")
            }
            CompileCalleeTarget::SetFromArray { .. } => {
                return Err(self.unsupported(origin, "set-from-array intrinsic"));
            }
        };

        self.append_call(expression, origin, call, effect)
    }

    fn validate_source_arguments(
        &self,
        call: &HirCall,
        arguments: &CompileCallArguments,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let source = call
            .arguments
            .iter()
            .map(|argument| {
                argument
                    .value
                    .map(|value| (argument.name.clone(), value))
                    .ok_or_else(|| {
                        self.inconsistent(origin, "missing call argument reached MIR lowering")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let values = source.iter().map(|(_, value)| *value).collect::<Vec<_>>();
        let valid = match arguments {
            CompileCallArguments::Script {
                evaluation_order, ..
            }
            | CompileCallArguments::ExternalNamed {
                evaluation_order, ..
            } => evaluation_order == &values,
            CompileCallArguments::Positional(arguments) => {
                arguments == &values && source.iter().all(|(name, _)| name.is_none())
            }
            CompileCallArguments::Dynamic(arguments) => {
                arguments.len() == source.len()
                    && arguments.iter().zip(&source).all(|(target, source)| {
                        target.name.as_deref() == source.0.as_deref() && target.value == source.1
                    })
            }
        };
        if valid {
            Ok(())
        } else {
            Err(self.inconsistent(
                origin,
                "compile-target argument order disagrees with the HIR call",
            ))
        }
    }

    fn lower_script_arguments(
        &mut self,
        arguments: &CompileCallArguments,
        signature: &CompileSignature,
        origin: MirSourceOrigin,
    ) -> Result<(Vec<MirScriptArgument>, MirScriptParameterGuardMode), MirBuildError> {
        let CompileCallArguments::Script {
            evaluation_order,
            parameter_slots,
        } = arguments
        else {
            return Err(self.inconsistent(
                origin,
                "script call target has incompatible argument placement",
            ));
        };
        let evaluated = self.lower_source_order(evaluation_order)?;
        if self.current_is_terminated()? {
            return Ok((
                Vec::new(),
                MirScriptParameterGuardMode::CheckCalleeParameterContracts,
            ));
        }
        let mut parameter_guards = MirScriptParameterGuardMode::ProvenAtCallSite;
        let mut projected = Vec::with_capacity(parameter_slots.len());
        for (index, (slot, parameter)) in parameter_slots
            .iter()
            .zip(&signature.parameters)
            .enumerate()
        {
            self.require_parameter_slot(slot, index, origin)?;
            match slot.value {
                CompilePlacedCallValue::Explicit {
                    source_index,
                    value,
                } => {
                    let operand = self.project_source(&evaluated, source_index, origin)?;
                    if self.input.targets().expression_guard(value).is_some() {
                        parameter_guards =
                            MirScriptParameterGuardMode::CheckCalleeParameterContracts;
                    }
                    projected.push(MirScriptArgument::placed(slot.parameter, operand));
                }
                CompilePlacedCallValue::MissingDefault => {
                    if parameter.contract.is_some() {
                        parameter_guards =
                            MirScriptParameterGuardMode::CheckCalleeParameterContracts;
                    }
                    projected.push(MirScriptArgument::missing(slot.parameter));
                }
            }
        }
        if projected.len() != signature.parameters.len() {
            return Err(self.inconsistent(
                origin,
                "script call parameter slots do not cover its signature",
            ));
        }
        Ok((projected, parameter_guards))
    }

    pub(super) fn lower_external_arguments(
        &mut self,
        arguments: &CompileCallArguments,
        signature: &CompileSignature,
        origin: MirSourceOrigin,
    ) -> Result<Vec<MirOperand>, MirBuildError> {
        match arguments {
            CompileCallArguments::Positional(arguments) => self.lower_source_order(arguments),
            CompileCallArguments::ExternalNamed {
                evaluation_order,
                parameter_slots,
            } => {
                let evaluated = self.lower_source_order(evaluation_order)?;
                if self.current_is_terminated()? {
                    return Ok(Vec::new());
                }
                let mut projected = Vec::with_capacity(evaluation_order.len());
                for (index, (slot, parameter)) in parameter_slots
                    .iter()
                    .zip(&signature.parameters)
                    .enumerate()
                {
                    self.require_parameter_slot(slot, index, origin)?;
                    match slot.value {
                        CompilePlacedCallValue::Explicit { source_index, .. } => {
                            projected.push(self.project_source(&evaluated, source_index, origin)?)
                        }
                        CompilePlacedCallValue::MissingDefault => {
                            if parameter.default != CompileParameterDefault::RuntimeProvided {
                                return Err(self.inconsistent(
                                    origin,
                                    "external call omits a parameter without a runtime default",
                                ));
                            }
                        }
                    }
                }
                if parameter_slots.len() != signature.parameters.len() {
                    return Err(self.inconsistent(
                        origin,
                        "external call parameter slots do not cover its signature",
                    ));
                }
                Ok(projected)
            }
            CompileCallArguments::Script { .. } | CompileCallArguments::Dynamic(_) => Err(self
                .inconsistent(
                    origin,
                    "external call target has incompatible argument placement",
                )),
        }
    }

    fn lower_positional_arguments(
        &mut self,
        arguments: &CompileCallArguments,
        origin: MirSourceOrigin,
    ) -> Result<Vec<MirOperand>, MirBuildError> {
        let CompileCallArguments::Positional(arguments) = arguments else {
            return Err(
                self.inconsistent(origin, "callable value has incompatible argument placement")
            );
        };
        self.lower_source_order(arguments)
    }

    fn lower_dynamic_arguments(
        &mut self,
        arguments: &CompileCallArguments,
        origin: MirSourceOrigin,
    ) -> Result<Vec<MirDynamicArgument>, MirBuildError> {
        let CompileCallArguments::Dynamic(arguments) = arguments else {
            return Err(self.inconsistent(
                origin,
                "dynamic call target has incompatible argument placement",
            ));
        };
        if self.current_is_terminated()? {
            return Ok(Vec::new());
        }
        let mut lowered = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let value = self.lower_captured(argument.value)?;
            lowered.push(MirDynamicArgument {
                name: argument.name.clone(),
                value,
            });
            if self.current_is_terminated()? {
                break;
            }
        }
        Ok(lowered)
    }

    fn lower_source_order(
        &mut self,
        expressions: &[HirExprId],
    ) -> Result<Vec<MirOperand>, MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(Vec::new());
        }
        let mut lowered = Vec::with_capacity(expressions.len());
        for expression in expressions {
            lowered.push(self.lower_captured(*expression)?);
            if self.current_is_terminated()? {
                break;
            }
        }
        Ok(lowered)
    }

    fn lower_captured(&mut self, expression: HirExprId) -> Result<MirOperand, MirBuildError> {
        let origin = self.call_expression_origin(expression)?;
        let operand = self.lower_expression(expression)?;
        if self.current_is_terminated()? {
            return Ok(operand);
        }
        self.capture_operand(operand, origin)
    }

    fn project_source(
        &self,
        evaluated: &[MirOperand],
        source_index: u32,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        usize::try_from(source_index)
            .ok()
            .and_then(|source_index| evaluated.get(source_index))
            .cloned()
            .ok_or_else(|| self.inconsistent(origin, "call argument source index is out of bounds"))
    }

    fn require_parameter_slot(
        &self,
        slot: &CompilePlacedCallArgument,
        index: usize,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        if usize::try_from(slot.parameter) == Ok(index) {
            Ok(())
        } else {
            Err(self.inconsistent(origin, "call parameter slots are not contiguous"))
        }
    }

    fn method_callee(
        &self,
        call: &HirCall,
        expected_name: &str,
        origin: MirSourceOrigin,
    ) -> Result<vela_hir::body::HirField, MirBuildError> {
        let record = self.body.expression(call.callee).ok_or_else(|| {
            self.inconsistent(origin, "call references a missing HIR callee expression")
        })?;
        let HirExprKind::Field(field) = &record.kind else {
            return Err(self.inconsistent(origin, "method call target has a non-field HIR callee"));
        };
        if field.name != expected_name {
            return Err(self.inconsistent(
                origin,
                "method call target name disagrees with the HIR callee",
            ));
        }
        Ok(field.clone())
    }

    fn direct_local_callee(
        &self,
        expression: HirExprId,
    ) -> Result<Option<HirLocalId>, MirBuildError> {
        let origin = self.call_expression_origin(expression)?;
        let record = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(origin, "call references a missing HIR callee expression")
        })?;
        match &record.kind {
            HirExprKind::Paren {
                expression: Some(inner),
            } => self.direct_local_callee(*inner),
            HirExprKind::Path(_) => {
                let bindings = self
                    .input
                    .graph()
                    .bindings_for_body(self.body.id)
                    .ok_or_else(|| self.inconsistent(origin, "HIR body has no binding map"))?;
                Ok(match bindings.resolution(expression) {
                    Some(BindingResolution::Local(local)) => Some(*local),
                    Some(
                        BindingResolution::Declaration(_)
                        | BindingResolution::Import(_)
                        | BindingResolution::QualifiedPath(_),
                    )
                    | None => None,
                })
            }
            _ => Ok(None),
        }
    }

    fn direct_lambda_body(
        &self,
        expression: HirExprId,
    ) -> Result<Option<HirBodyId>, MirBuildError> {
        let origin = self.call_expression_origin(expression)?;
        let record = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(origin, "call references a missing HIR callee expression")
        })?;
        match &record.kind {
            HirExprKind::Lambda { body } => Ok(Some(*body)),
            HirExprKind::Paren {
                expression: Some(inner),
            } => self.direct_lambda_body(*inner),
            _ => Ok(None),
        }
    }

    fn call_expression_origin(
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

    fn append_call(
        &mut self,
        expression: HirExprId,
        origin: MirSourceOrigin,
        call: MirCall,
        effect: MirEffect,
    ) -> Result<MirOperand, MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(MirOperand::Immediate(crate::MirImmediate::Unit));
        }
        let analysis = self.input.analysis();
        let result = analysis.expression(expression).ok_or_else(|| {
            self.inconsistent(origin, "call expression has no analysis type fact")
        })?;
        let destination = self.function.add_temp(value_type(Some(result)), origin);
        let safepoint = self.function.add_safepoint(MirSafepoint::new(origin));
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(destination)),
                MirStatementKind::Call(call),
                effect,
                Some(safepoint),
            ),
        )?;
        Ok(MirOperand::Temp(destination))
    }
}

#[cfg(test)]
#[path = "tests/calls.rs"]
mod tests;
