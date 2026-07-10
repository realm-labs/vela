use super::call_placements::{HirMethodArguments, HirMethodCallee};
use super::*;
use vela_mir::{CompileCallArguments, CompileCalleeTarget, CompilePlacedCallValue};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_hir_call(
        &mut self,
        span: Span,
        call: &vela_hir::body::HirCall,
    ) -> CompileResult<Register> {
        if let Some(result) = self.try_compile_placed_hir_host_call(span, call)? {
            return Ok(result);
        }
        if let Some(field) = self.hir_field_for_expression(call.callee).cloned() {
            let receiver_fact = self.script_fact_for_hir_expression(field.receiver);
            let receiver_shape = self.value_shape_for_hir_expression(field.receiver);
            let value_receiver_type = self
                .hir_value_type(field.receiver)
                .or_else(|| receiver_shape.as_ref().and_then(ValueShape::value_type));
            let value_methods_known = value_receiver_type
                .as_ref()
                .is_some_and(|value| self.value_methods_known_for_type(value));
            let ordering_key_shape = call
                .arguments
                .first()
                .and_then(|argument| argument.value)
                .and_then(|argument| {
                    self.hir_callback_return_shape(receiver_shape.as_ref(), &field.name, argument)
                });
            self.reject_static_hir_array_ordering_method_without_ord(
                &field.name,
                value_receiver_type.as_ref(),
                receiver_shape.as_ref(),
                ordering_key_shape.as_ref(),
                span,
            )?;
            let legacy_script_method = receiver_fact.as_ref().and_then(|fact| {
                self.script_method_id_for_type(&fact.type_name, &field.name)
                    .map(|method| (fact.type_name.clone(), method))
            });
            let legacy_value_method = value_receiver_type
                .as_ref()
                .and_then(|value| self.value_method_target_for_type(value, &field.name));
            let target = self.placed_call_target(call.expression)?;
            let receiver = self.compile_hir_expression(field.receiver)?;
            let dst = self.alloc_register()?;
            return match target.callee {
                CompileCalleeTarget::ScriptMethod {
                    target: method_target,
                    debug_name,
                } => {
                    if debug_name != field.name {
                        return Err(self.compile_target_input_error(
                            call.expression,
                            "script method target name disagrees with HIR",
                        ));
                    }
                    if legacy_value_method.is_some()
                        || legacy_script_method
                            .as_ref()
                            .is_some_and(|(type_name, method)| {
                                *method != method_target.method
                                    || self
                                        .facts
                                        .semantic_input
                                        .targets()
                                        .type_descriptor(method_target.owner)
                                        .is_none_or(|owner| {
                                            owner.canonical_name != *type_name
                                                && !owner
                                                    .canonical_name
                                                    .ends_with(&format!("::{type_name}"))
                                        })
                            })
                    {
                        return Err(self.compile_target_input_error(
                            call.expression,
                            "script method target disagrees with the direct stable selection",
                        ));
                    }
                    let descriptor = self
                        .facts
                        .semantic_input
                        .targets()
                        .method_descriptor(method_target.owner, method_target.method)
                        .cloned()
                        .ok_or_else(|| {
                            self.compile_target_input_error(
                                call.expression,
                                "script method target has no neutral descriptor",
                            )
                        })?;
                    if descriptor.member_name != field.name
                        || !matches!(
                            descriptor.class,
                            vela_mir::CompileMethodClass::Script { executable, .. }
                                if executable == method_target
                        )
                    {
                        return Err(self.compile_target_input_error(
                            call.expression,
                            "script method descriptor disagrees with the placed executable",
                        ));
                    }
                    let params = self
                        .facts
                        .script_method_ids
                        .iter()
                        .find_map(|(key, method)| {
                            (*method == method_target.method)
                                .then(|| self.facts.script_method_signatures.get(key).cloned())
                                .flatten()
                        })
                        .unwrap_or_default()
                        .into_iter()
                        .skip(1)
                        .collect::<Vec<_>>();
                    let args = self.compile_hir_method_arguments(HirMethodArguments {
                        call: call.expression,
                        target: HirMethodCallee::Script(method_target),
                        method: &field.name,
                        receiver_type: value_receiver_type.as_ref(),
                        receiver_shape: receiver_shape.as_ref(),
                        signature: descriptor.signature,
                        params: &params,
                        preserve_missing_defaults: true,
                    })?;
                    self.emit_spanned(
                        UnlinkedInstructionKind::CallMethodId {
                            dst,
                            receiver,
                            method: field.name,
                            method_id: method_target.method,
                            args,
                        },
                        span,
                    );
                    Ok(dst)
                }
                CompileCalleeTarget::ValueMethod {
                    owner,
                    method,
                    debug_name,
                } => {
                    if debug_name != field.name {
                        return Err(self.compile_target_input_error(
                            call.expression,
                            "value method target name disagrees with HIR",
                        ));
                    }
                    if legacy_script_method.is_some()
                        || legacy_value_method.is_some_and(|(legacy_owner, legacy_method)| {
                            legacy_owner != owner || legacy_method != method
                        })
                    {
                        return Err(self.compile_target_input_error(
                            call.expression,
                            "value method target disagrees with the direct stable selection",
                        ));
                    }
                    let descriptor = self
                        .facts
                        .semantic_input
                        .targets()
                        .method_descriptor(owner, method)
                        .cloned()
                        .ok_or_else(|| {
                            self.compile_target_input_error(
                                call.expression,
                                "value method target has no neutral descriptor",
                            )
                        })?;
                    if descriptor.member_name != field.name
                        || !matches!(
                            descriptor.class,
                            vela_mir::CompileMethodClass::Value
                                | vela_mir::CompileMethodClass::Registry
                        )
                    {
                        return Err(self.compile_target_input_error(
                            call.expression,
                            "value method descriptor disagrees with the placed target",
                        ));
                    }
                    let params = self
                        .facts
                        .registry
                        .and_then(|registry| registry.method_params(method))
                        .map(|params| registry_param_hints(params, span))
                        .unwrap_or_default();
                    let args = self.compile_hir_method_arguments(HirMethodArguments {
                        call: call.expression,
                        target: HirMethodCallee::Value { owner, method },
                        method: &field.name,
                        receiver_type: value_receiver_type.as_ref(),
                        receiver_shape: receiver_shape.as_ref(),
                        signature: descriptor.signature,
                        params: &params,
                        preserve_missing_defaults: false,
                    })?;
                    self.emit_spanned(
                        UnlinkedInstructionKind::CallMethodId {
                            dst,
                            receiver,
                            method: field.name,
                            method_id: method,
                            args,
                        },
                        span,
                    );
                    Ok(dst)
                }
                CompileCalleeTarget::DynamicMethod(method_target) => {
                    if legacy_script_method.is_some()
                        || legacy_value_method.is_some()
                        || receiver_fact.is_some()
                        || value_methods_known
                    {
                        return Err(self.compile_target_input_error(
                            call.expression,
                            "dynamic method target disagrees with direct stable receiver facts",
                        ));
                    }
                    if method_target.member != field.name {
                        return Err(self.compile_target_input_error(
                            call.expression,
                            "dynamic method placement name disagrees with HIR",
                        ));
                    }
                    let CompileCallArguments::Dynamic(arguments) = target.arguments else {
                        return Err(self.compile_target_input_error(
                            call.expression,
                            "dynamic method call has non-dynamic argument placement",
                        ));
                    };
                    let mut args = Vec::with_capacity(arguments.len());
                    for argument in arguments {
                        args.push(DynamicCallArgument {
                            name: argument.name,
                            value: self.compile_hir_expression(argument.value)?,
                        });
                    }
                    self.emit_spanned(
                        UnlinkedInstructionKind::CallDynamicMethod {
                            dst,
                            receiver,
                            method: field.name,
                            args,
                        },
                        span,
                    );
                    Ok(dst)
                }
                _ => Err(self.compile_target_input_error(
                    call.expression,
                    "method HIR owns a non-method compile-target family",
                )),
            };
        }

        let dst = self.alloc_register()?;
        if let Some((declaration, name)) = self.script_function_call(call.expression) {
            let params = self
                .facts
                .script_function_signatures
                .get(&declaration)
                .cloned()
                .ok_or_else(|| hir_unsupported("script call", span))?;
            let (target, args, mode) =
                self.compile_hir_script_arguments(call.expression, declaration, &params)?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallFunction {
                    dst,
                    target,
                    name,
                    mode,
                    args,
                },
                span,
            );
            return Ok(dst);
        }
        if self.local_call_callee(call.expression).is_some()
            || self.hir_callee_path(call.expression).is_none()
        {
            let target = self.placed_call_target(call.expression)?;
            let callee_matches = match target.callee {
                CompileCalleeTarget::Local(local) => {
                    self.local_call_callee(call.expression) == Some(local)
                }
                CompileCalleeTarget::Lambda(body) => {
                    self.hir_direct_lambda_body(call.callee) == Some(body)
                }
                CompileCalleeTarget::DynamicCallable => {
                    self.local_call_callee(call.expression).is_none()
                }
                _ => false,
            };
            if !callee_matches {
                return Err(self.compile_target_input_error(
                    call.expression,
                    "callable-value placement disagrees with the HIR callee",
                ));
            }
            let callee = self.compile_hir_expression(call.callee)?;
            let values = match target.arguments {
                CompileCallArguments::Positional(values) => values,
                CompileCallArguments::Dynamic(arguments) => arguments
                    .into_iter()
                    .map(|argument| argument.value)
                    .collect(),
                CompileCallArguments::Script { .. }
                | CompileCallArguments::ExternalNamed { .. } => {
                    return Err(self.compile_target_input_error(
                        call.expression,
                        "callable value owns an incompatible argument placement",
                    ));
                }
            };
            let args = values
                .into_iter()
                .map(|value| self.compile_hir_expression(value))
                .collect::<CompileResult<Vec<_>>>()?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallClosure { dst, callee, args },
                span,
            );
            return Ok(dst);
        }
        if let Some(fact) = self.script_fact_for_hir_call(call.expression)
            && let Some(variant) = fact.enum_variant
        {
            let fields = self.compile_hir_tuple_variant_fields(
                call.expression,
                &fact.type_name,
                &variant,
                &call.arguments,
                span,
            )?;
            self.emit(UnlinkedInstructionKind::MakeEnum {
                dst,
                enum_name: fact.type_name,
                variant,
                fields,
            });
            return Ok(dst);
        }
        let path = self
            .hir_callee_path(call.expression)
            .ok_or_else(|| hir_unsupported("call target", span))?
            .to_vec();
        let name = path.join("::");
        if name == "set::from_array" {
            let target = self.placed_call_target(call.expression)?;
            if !matches!(target.callee, CompileCalleeTarget::SetFromArray { .. }) {
                return Err(self.compile_target_input_error(
                    call.expression,
                    "set::from_array placement has a different callee family",
                ));
            }
            let CompileCallArguments::Positional(values) = target.arguments else {
                return Err(self.compile_target_input_error(
                    call.expression,
                    "set::from_array owns non-positional arguments",
                ));
            };
            let args = values
                .into_iter()
                .map(|value| self.compile_hir_expression(value))
                .collect::<CompileResult<Vec<_>>>()?;
            let [src] = args.as_slice() else {
                return Err(hir_unsupported("set::from_array", span));
            };
            self.emit_spanned(
                UnlinkedInstructionKind::MakeSetFromArray { dst, src: *src },
                span,
            );
            return Ok(dst);
        }
        let (native, args) = self.compile_hir_native_arguments(call.expression, &name, span)?;
        self.emit_spanned(
            UnlinkedInstructionKind::CallNative {
                dst: Some(dst),
                name,
                native,
                cache_site: None,
                args,
            },
            span,
        );
        Ok(dst)
    }

    pub(in crate::compiler) fn compile_hir_native_arguments(
        &mut self,
        call: HirExprId,
        function: &str,
        call_span: Span,
    ) -> CompileResult<(crate::FunctionId, Vec<Register>)> {
        let target = self.placed_call_target(call)?;
        let (target_function, debug_name) = match target.callee {
            CompileCalleeTarget::NativeFunction {
                function,
                debug_name,
            }
            | CompileCalleeTarget::StdlibFunction {
                function,
                debug_name,
            }
            | CompileCalleeTarget::Reflection {
                function,
                debug_name,
                ..
            } => (function, debug_name),
            _ => {
                return Err(self.compile_target_input_error(
                    call,
                    "native call target disagrees with the direct callee selection",
                ));
            }
        };
        if debug_name != function {
            return Err(self.compile_target_input_error(
                call,
                "native call target name disagrees with the HIR callee",
            ));
        }
        if self
            .facts
            .registry
            .and_then(|registry| registry.resolve_native_function_name(function))
            .is_some_and(|legacy| legacy != target_function)
        {
            return Err(self.compile_target_input_error(
                call,
                "native call FunctionId disagrees with the direct registry selection",
            ));
        }
        let descriptor = self
            .facts
            .semantic_input
            .targets()
            .function_descriptor(target_function)
            .cloned()
            .ok_or_else(|| {
                self.compile_target_input_error(
                    call,
                    "native call target has no neutral descriptor",
                )
            })?;
        if descriptor.id != target_function || descriptor.debug_name != function {
            return Err(self.compile_target_input_error(
                call,
                "native call descriptor disagrees with the placed target",
            ));
        }
        let params = self
            .facts
            .registry
            .and_then(|registry| registry.function_params(target_function))
            .map(|params| registry_param_hints(params, call_span))
            .unwrap_or_default();
        if !params.is_empty()
            && (params.len() != descriptor.signature.parameters.len()
                || params
                    .iter()
                    .zip(&descriptor.signature.parameters)
                    .any(|(legacy, neutral)| legacy.name != neutral.name))
        {
            return Err(self.compile_target_input_error(
                call,
                "registry function parameters disagree with the compile-target signature",
            ));
        }
        match target.arguments {
            CompileCallArguments::Positional(evaluation_order) => {
                let mut registers = Vec::with_capacity(evaluation_order.len());
                for (index, expression) in evaluation_order.into_iter().enumerate() {
                    let register = if let Some(param) = params.get(index) {
                        self.compile_hir_argument_for_expected_param(
                            function,
                            index,
                            expression,
                            param,
                            &[],
                            false,
                        )?
                        .0
                    } else {
                        self.compile_hir_expression(expression)?
                    };
                    registers.push(register);
                }
                Ok((target_function, registers))
            }
            CompileCallArguments::ExternalNamed {
                evaluation_order,
                parameter_slots,
            } => {
                if descriptor.signature.parameters.len() != parameter_slots.len() {
                    return Err(self.compile_target_input_error(
                        call,
                        "named native placement disagrees with the compile-target signature",
                    ));
                }
                let source_registers = self.compile_placed_call_sources(
                    call,
                    &evaluation_order,
                    &parameter_slots,
                    |compiler, parameter, expression| match params.get(parameter) {
                        Some(param) => compiler.compile_hir_argument_for_expected_param(
                            function,
                            parameter,
                            expression,
                            param,
                            &[],
                            false,
                        ),
                        None => compiler
                            .compile_hir_expression(expression)
                            .map(|register| (register, false)),
                    },
                )?;
                self.validate_external_missing_slots(
                    call,
                    &descriptor.signature,
                    &parameter_slots,
                )?;
                self.project_external_registers(call, &parameter_slots, &source_registers)
                    .map(|arguments| (target_function, arguments))
            }
            CompileCallArguments::Script { .. } | CompileCallArguments::Dynamic(_) => Err(self
                .compile_target_input_error(
                    call,
                    "native call owns an incompatible argument placement",
                )),
        }
    }

    pub(in crate::compiler) fn compile_hir_host_method_arguments(
        &mut self,
        call: HirExprId,
        method_name: &str,
        legacy_owner: Option<vela_def::TypeId>,
        legacy_method: Option<vela_common::HostMethodId>,
        call_span: Span,
    ) -> CompileResult<(vela_common::HostMethodId, Vec<Register>)> {
        let target = self.placed_call_target(call)?;
        let CompileCalleeTarget::HostMethod(host) = target.callee else {
            return Err(self.compile_target_input_error(
                call,
                "host method placement has a different callee family",
            ));
        };
        if legacy_owner.is_some_and(|owner| host.owner.semantic != owner)
            || legacy_method.is_some_and(|method| host.runtime != method)
        {
            return Err(self.compile_target_input_error(
                call,
                "host method placement runtime ID disagrees with direct selection",
            ));
        }
        let descriptor = self
            .facts
            .semantic_input
            .targets()
            .method_descriptor(host.owner.semantic, host.semantic)
            .cloned()
            .ok_or_else(|| {
                self.compile_target_input_error(
                    call,
                    "host method target has no neutral descriptor",
                )
            })?;
        if descriptor.member_name != method_name
            || descriptor.signature != host.signature
            || !matches!(
                descriptor.class,
                vela_mir::CompileMethodClass::Host { runtime } if runtime == host.runtime
            )
        {
            return Err(self.compile_target_input_error(
                call,
                "host method descriptor disagrees with the placed target",
            ));
        }
        let params = self
            .facts
            .registry
            .and_then(|registry| registry.host_method_params_by_runtime_id(host.runtime.get()))
            .map(|params| registry_param_hints(params, call_span))
            .unwrap_or_default();
        if !params.is_empty()
            && (params.len() != host.signature.parameters.len()
                || params
                    .iter()
                    .zip(&host.signature.parameters)
                    .any(|(legacy, neutral)| legacy.name != neutral.name))
        {
            return Err(self.compile_target_input_error(
                call,
                "registry host parameters disagree with the compile-target signature",
            ));
        }
        match target.arguments {
            CompileCallArguments::Positional(evaluation_order) => evaluation_order
                .into_iter()
                .map(|expression| self.compile_hir_expression(expression))
                .collect::<CompileResult<Vec<_>>>()
                .map(|arguments| (host.runtime, arguments)),
            CompileCallArguments::ExternalNamed {
                evaluation_order,
                parameter_slots,
            } => {
                if host.signature.parameters.len() != parameter_slots.len() {
                    return Err(self.compile_target_input_error(
                        call,
                        "named host method placement disagrees with the compile-target signature",
                    ));
                }
                let source_registers = self.compile_placed_call_sources(
                    call,
                    &evaluation_order,
                    &parameter_slots,
                    |compiler, parameter, expression| match params.get(parameter) {
                        Some(param) => compiler.compile_hir_argument_for_expected_param(
                            "host method",
                            parameter,
                            expression,
                            param,
                            &[],
                            false,
                        ),
                        None => compiler
                            .compile_hir_expression(expression)
                            .map(|register| (register, false)),
                    },
                )?;
                self.validate_external_missing_slots(call, &host.signature, &parameter_slots)?;
                self.project_external_registers(call, &parameter_slots, &source_registers)
                    .map(|arguments| (host.runtime, arguments))
            }
            CompileCallArguments::Script { .. } | CompileCallArguments::Dynamic(_) => Err(self
                .compile_target_input_error(
                    call,
                    "host method owns an incompatible argument placement",
                )),
        }
    }

    pub(in crate::compiler) fn compile_hir_script_arguments(
        &mut self,
        call: HirExprId,
        declaration: vela_hir::ids::HirDeclId,
        params: &[ParamHint],
    ) -> CompileResult<(vela_def::FunctionId, Vec<CallArgument>, ScriptCallMode)> {
        let target = self.placed_call_target(call)?;
        let CompileCalleeTarget::ScriptFunction { function, .. } = target.callee else {
            return Err(self.compile_target_input_error(
                call,
                "resolved script function call has a different compile-target family",
            ));
        };
        if self
            .facts
            .semantic_input
            .targets()
            .function_for_declaration(declaration)
            != Some(function)
        {
            return Err(self.compile_target_input_error(
                call,
                "script function call target disagrees with its HIR declaration",
            ));
        }
        let descriptor = self
            .facts
            .semantic_input
            .targets()
            .function_descriptor(function)
            .cloned()
            .ok_or_else(|| {
                self.compile_target_input_error(
                    call,
                    "script function target has no neutral descriptor",
                )
            })?;
        if descriptor.class != vela_mir::CompileFunctionClass::Script
            || descriptor.signature.parameters.len() != params.len()
            || descriptor
                .signature
                .parameters
                .iter()
                .zip(params)
                .any(|(neutral, hir)| neutral.name != hir.name)
        {
            return Err(self.compile_target_input_error(
                call,
                "script function descriptor disagrees with its HIR signature",
            ));
        }
        let CompileCallArguments::Script {
            evaluation_order,
            parameter_slots,
        } = target.arguments
        else {
            return Err(self.compile_target_input_error(
                call,
                "script call does not own placed script arguments",
            ));
        };
        if parameter_slots.len() != descriptor.signature.parameters.len() {
            return Err(self.compile_target_input_error(
                call,
                "script placement disagrees with the compile-target signature",
            ));
        }
        let source_registers = self.compile_placed_call_sources(
            call,
            &evaluation_order,
            &parameter_slots,
            |compiler, parameter, expression| {
                let param = params.get(parameter).ok_or_else(|| {
                    compiler.compile_target_input_error(
                        call,
                        "script call parameter slot exceeds its HIR signature",
                    )
                })?;
                compiler.compile_hir_argument_for_expected_param(
                    "script function",
                    parameter,
                    expression,
                    param,
                    &[],
                    true,
                )
            },
        )?;
        let mut mode = ScriptCallMode::Unchecked;
        let mut arguments = Vec::with_capacity(params.len());
        for (index, (slot, param)) in parameter_slots.iter().zip(params).enumerate() {
            if usize::try_from(slot.parameter) != Ok(index) {
                return Err(self.compile_target_input_error(
                    call,
                    "script call parameter slots are not contiguous",
                ));
            }
            match slot.value {
                CompilePlacedCallValue::Explicit { source_index, .. } => {
                    let source_index = usize::try_from(source_index).map_err(|_| {
                        self.compile_target_input_error(
                            call,
                            "script call source index exceeds usize",
                        )
                    })?;
                    let (register, requires_guard) =
                        source_registers.get(source_index).copied().ok_or_else(|| {
                            self.compile_target_input_error(
                                call,
                                "script call source index is out of bounds",
                            )
                        })?;
                    if requires_guard {
                        mode = ScriptCallMode::Checked;
                    }
                    arguments.push(CallArgument::Register(register));
                }
                CompilePlacedCallValue::MissingDefault => {
                    if matches!(
                        descriptor.signature.parameters[index].default,
                        vela_mir::CompileParameterDefault::Required
                    ) {
                        return Err(self.compile_target_input_error(
                            call,
                            "required script parameter is represented as missing",
                        ));
                    }
                    if param.type_hint.is_some() {
                        mode = ScriptCallMode::Checked;
                    }
                    arguments.push(CallArgument::Missing);
                }
            }
        }
        Ok((function, arguments, mode))
    }

    pub(in crate::compiler) fn compile_hir_argument_for_expected_param(
        &mut self,
        function: &str,
        index: usize,
        expression: HirExprId,
        param: &ParamHint,
        callback_shapes: &[Option<ValueShape>],
        script_function: bool,
    ) -> CompileResult<(Register, bool)> {
        let Some(expected) = param
            .type_hint
            .as_ref()
            .and_then(crate::compiler::value_types::type_hint_value_type)
        else {
            return self
                .compile_hir_expression(expression)
                .map(|value| (value, false));
        };
        let context = if script_function {
            TypeContractContext::FunctionParameter {
                name: param.name.clone(),
            }
        } else {
            TypeContractContext::NativeParameter {
                function: function.to_owned(),
                name: param.name.clone(),
                index: u16::try_from(index).unwrap_or(u16::MAX),
            }
        };
        self.compile_hir_expression_for_expected_type(
            expression,
            expected,
            context,
            callback_shapes,
        )
    }

    pub(in crate::compiler) fn compile_hir_expression_for_expected_type(
        &mut self,
        expression: HirExprId,
        expected: RuntimeTypeFact,
        context: TypeContractContext,
        callback_shapes: &[Option<ValueShape>],
    ) -> CompileResult<(Register, bool)> {
        let (span, kind) = self.hir_expression_record(expression)?;
        let expected_is_function = expected
            == RuntimeTypeFact::Standard(
                crate::compiler::value_types::StandardRuntimeType::Function,
            );
        let static_type = match self.hir_static_type(expression) {
            StaticExprType::Dynamic => self
                .value_shape_for_hir_expression(expression)
                .and_then(|shape| shape.value_type())
                .map(StaticExprType::Exact)
                .unwrap_or(StaticExprType::Dynamic),
            known => known,
        };
        let outcome = check_expected_type(static_type, expected, span, context.clone())?;
        if let ExpectedTypeOutcome::Contextualized(RuntimeTypeFact::Primitive(tag)) = &outcome
            && let Some(constant) = self.compile_hir_contextual_numeric_literal(expression, *tag)?
        {
            return self.emit_constant(constant).map(|value| (value, false));
        }
        if expected_is_function && matches!(kind, HirExprKind::Lambda { .. }) {
            return self
                .compile_hir_lambda(expression, callback_shapes)
                .map(|value| (value, false));
        }
        let register = self.compile_hir_expression(expression)?;
        let requires_guard = matches!(outcome, ExpectedTypeOutcome::RequiresRuntimeGuard(_));
        if let ExpectedTypeOutcome::RequiresRuntimeGuard(expected) = &outcome
            && let Some((location, name)) = guard_location_and_name(context)
            && let Some(plan) = crate::compiler::type_guard_plan_for_runtime_type(expected)
        {
            self.emit_spanned(
                UnlinkedInstructionKind::GuardType {
                    src: register,
                    guard: UnlinkedTypeGuard::new(
                        plan,
                        UnlinkedGuardContext::new(GuardKind::Contract, location, name),
                    ),
                },
                span,
            );
        }
        Ok((register, requires_guard))
    }

    pub(in crate::compiler) fn hir_callback_param_shapes(
        &self,
        receiver_shape: Option<&ValueShape>,
        method: &str,
        expression: HirExprId,
    ) -> Option<Vec<Option<ValueShape>>> {
        let HirExprKind::Lambda { body } = self.hir_expression_record(expression).ok()?.1 else {
            return None;
        };
        let count = self
            .hir_bodies
            .iter()
            .find(|candidate| candidate.id == body)?
            .params
            .len();
        callback_param_shapes(receiver_shape?, method, count)
    }

    pub(in crate::compiler) fn hir_callback_return_shape(
        &self,
        receiver_shape: Option<&ValueShape>,
        method: &str,
        expression: HirExprId,
    ) -> Option<ValueShape> {
        let HirExprKind::Lambda { body } = self.hir_expression_record(expression).ok()?.1 else {
            return None;
        };
        let body = self
            .hir_bodies
            .iter()
            .find(|candidate| candidate.id == body)?;
        let hints = callback_param_shapes(receiver_shape?, method, body.params.len())?;
        let locals = body
            .params
            .iter()
            .zip(hints)
            .filter_map(|(param, shape)| shape.map(|shape| (param.local, shape)))
            .collect::<BTreeMap<_, _>>();
        let expression = match body.root {
            HirBodyRoot::Expr(expression) => expression,
            HirBodyRoot::Block(block) => self.hir_block_tail_expression(block)?,
            HirBodyRoot::Empty => return None,
        };
        self.hir_shape_with_locals(expression, &locals)
    }

    pub(in crate::compiler) fn hir_shape_with_locals(
        &self,
        expression: HirExprId,
        locals: &BTreeMap<vela_hir::ids::HirLocalId, ValueShape>,
    ) -> Option<ValueShape> {
        match self.hir_expression_record(expression).ok()?.1 {
            HirExprKind::Path(_) => self
                .local_for_expression(expression)
                .and_then(|local| locals.get(&local).cloned())
                .or_else(|| self.value_shape_for_hir_expression(expression)),
            HirExprKind::Field(field) => self
                .hir_shape_with_locals(field.receiver, locals)?
                .as_record()?
                .field_value_shape(&field.name)
                .cloned(),
            HirExprKind::Paren {
                expression: Some(inner),
            } => self.hir_shape_with_locals(inner, locals),
            HirExprKind::Call(call) => {
                if let Some(field) = self.hir_field_for_expression(call.callee) {
                    let receiver = self.hir_shape_with_locals(field.receiver, locals)?;
                    let first = call
                        .arguments
                        .first()
                        .and_then(|argument| argument.value)
                        .and_then(|value| self.hir_shape_with_locals(value, locals));
                    return self.hir_method_result_shape(receiver, &field.name, first, None);
                }
                self.value_shape_for_hir_expression(call.expression)
            }
            _ => self.value_shape_for_hir_expression(expression),
        }
    }

    pub(in crate::compiler) fn compile_hir_tuple_variant_fields(
        &mut self,
        call: HirExprId,
        type_name: &str,
        variant: &str,
        arguments: &[vela_hir::body::HirArgument],
        _call_span: Span,
    ) -> CompileResult<Vec<(String, Register)>> {
        let target = self.placed_constructor_target(call)?;
        let vela_mir::CompileConstructorTarget::Variant {
            type_id,
            variant: variant_id,
            evaluation_order,
            fields,
        } = target
        else {
            return Err(self.compile_target_input_error(
                call,
                "tuple variant HIR call has a non-variant constructor placement",
            ));
        };
        self.require_constructor_type_name(call, type_id, type_name)?;
        let (variant_owner, variant_name) = self
            .facts
            .semantic_input
            .targets()
            .variant_descriptor(variant_id)
            .map(|descriptor| (descriptor.owner, descriptor.name.clone()))
            .ok_or_else(|| {
                self.compile_target_input_error(call, "tuple variant descriptor is missing")
            })?;
        if variant_owner != type_id || variant_name != variant {
            return Err(self.compile_target_input_error(
                call,
                "tuple variant placement disagrees with the HIR callee",
            ));
        }
        if evaluation_order.len() != arguments.len()
            || evaluation_order
                .iter()
                .zip(arguments)
                .any(|(source, argument)| argument.value != Some(*source))
        {
            return Err(self.compile_target_input_error(
                call,
                "tuple constructor evaluation order disagrees with HIR arguments",
            ));
        }
        let placed = fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                if usize::try_from(field.parameter) != Ok(index) {
                    return Err(self.compile_target_input_error(
                        call,
                        "tuple constructor fields are not contiguous",
                    ));
                }
                let field_name = self
                    .facts
                    .semantic_input
                    .targets()
                    .field_descriptor(field.field)
                    .map(|descriptor| descriptor.name.clone())
                    .ok_or_else(|| {
                        self.compile_target_input_error(
                            call,
                            "tuple constructor field descriptor is missing",
                        )
                    })?;
                Ok((field_name, field.parameter_name.clone(), field.value))
            })
            .collect::<CompileResult<Vec<_>>>()?;
        let shape = self.enum_constructor_shape(type_name, variant);
        let mut source_registers = vec![None; evaluation_order.len()];
        for (source_index, source) in evaluation_order.iter().copied().enumerate() {
            let (field_name, parameter_name, value) = placed
                .iter()
                .find_map(|(field_name, parameter_name, value)| match value {
                    vela_mir::CompileConstructorValue::Explicit {
                        source_index: candidate,
                        value,
                    } if usize::try_from(*candidate) == Ok(source_index) => {
                        Some((field_name, parameter_name, *value))
                    }
                    vela_mir::CompileConstructorValue::Explicit { .. }
                    | vela_mir::CompileConstructorValue::EvaluatedDefault(_) => None,
                })
                .ok_or_else(|| {
                    self.compile_target_input_error(
                        call,
                        "tuple constructor source has no field slot",
                    )
                })?;
            if source != value
                || arguments[source_index]
                    .name
                    .as_deref()
                    .is_some_and(|name| name != parameter_name)
            {
                return Err(self.compile_target_input_error(
                    call,
                    "tuple constructor source disagrees with its field slot",
                ));
            }
            let expected = shape
                .as_ref()
                .and_then(|shape| shape.field_value_type(field_name));
            source_registers[source_index] =
                Some(self.compile_hir_constructor_field_value(source, expected, field_name)?);
        }

        placed
            .into_iter()
            .map(|(field_name, _, value)| {
                let register = match value {
                    vela_mir::CompileConstructorValue::Explicit { source_index, .. } => {
                        let source_index = usize::try_from(source_index).map_err(|_| {
                            self.compile_target_input_error(
                                call,
                                "tuple constructor source index exceeds usize",
                            )
                        })?;
                        source_registers
                            .get(source_index)
                            .copied()
                            .flatten()
                            .ok_or_else(|| {
                                self.compile_target_input_error(
                                    call,
                                    "tuple constructor source index is out of bounds",
                                )
                            })?
                    }
                    vela_mir::CompileConstructorValue::EvaluatedDefault(body) => {
                        let value = self
                            .facts
                            .semantic_input
                            .targets()
                            .evaluated_schema_default(body)
                            .cloned()
                            .ok_or_else(|| {
                                self.compile_target_input_error(
                                    call,
                                    format!("tuple constructor default {body:?} is missing"),
                                )
                            })?;
                        self.emit_constant(super::values::constant_from_mir(value))?
                    }
                };
                Ok((field_name, register))
            })
            .collect()
    }
}
