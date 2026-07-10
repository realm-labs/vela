use super::*;

struct HirMethodArguments<'a> {
    method: &'a str,
    receiver_type: Option<&'a RuntimeTypeFact>,
    receiver_shape: Option<&'a ValueShape>,
    params: &'a [ParamHint],
    arguments: &'a [vela_hir::body::HirArgument],
    call_span: Span,
    preserve_missing_defaults: bool,
}

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_hir_call(
        &mut self,
        span: Span,
        call: &vela_hir::body::HirCall,
    ) -> CompileResult<Register> {
        if let Some(field) = self.hir_field_for_expression(call.callee).cloned() {
            if let Some(resolved) = self.hir_host_path(field.receiver) {
                if field.name == "remove"
                    && call.arguments.is_empty()
                    && matches!(
                        self.hir_expression_record(field.receiver)?.1,
                        HirExprKind::Index(_)
                    )
                {
                    self.reject_invalid_hir_host_index_access(
                        field.receiver,
                        HostIndexAccessKind::Remove,
                        span,
                    )?;
                    let root = self.compile_host_path_root(&resolved.path.root)?;
                    self.emit_host_remove(root, resolved.path, span)?;
                    return self.emit_constant(Constant::Unit);
                }
                if field.name == "push" && !resolved.path.segments.is_empty() {
                    let [argument] = call.arguments.as_slice() else {
                        return Err(hir_unsupported("host path push arity", span));
                    };
                    if argument.name.is_some() {
                        return Err(hir_unsupported("host path push", span));
                    }
                    self.reject_invalid_hir_host_assignment(
                        field.receiver,
                        HirAssignOp::Set,
                        span,
                    )?;
                    let value = argument
                        .value
                        .ok_or_else(|| hir_unsupported("host path push", span))?;
                    let value = self.compile_hir_expression(value)?;
                    let root = self.compile_host_path_root(&resolved.path.root)?;
                    self.emit_host_mutate(root, resolved.path, HostMutationOp::Push, value, span)?;
                    return self.emit_constant(Constant::Unit);
                }
                if let Some(method_id) =
                    self.host_method_id(resolved.type_name.as_deref(), &field.name)
                {
                    let args =
                        self.compile_hir_host_method_arguments(method_id, &call.arguments, span)?;
                    let root = self.compile_host_path_root(&resolved.path.root)?;
                    let dst = self.alloc_register()?;
                    self.emit_host_call(Some(dst), root, resolved.path, method_id, args, span)?;
                    return Ok(dst);
                }
            }
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
            let receiver = self.compile_hir_expression(field.receiver)?;
            let dst = self.alloc_register()?;
            if let Some(method_id) = receiver_fact
                .as_ref()
                .and_then(|fact| self.script_method_id_for_type(&fact.type_name, &field.name))
            {
                let params = self
                    .script_method_params(
                        &receiver_fact.as_ref().expect("method fact").type_name,
                        &field.name,
                    )
                    .unwrap_or_default()
                    .into_iter()
                    .skip(1)
                    .collect::<Vec<_>>();
                let args = self.compile_hir_method_arguments(HirMethodArguments {
                    method: &field.name,
                    receiver_type: value_receiver_type.as_ref(),
                    receiver_shape: receiver_shape.as_ref(),
                    params: &params,
                    arguments: &call.arguments,
                    call_span: span,
                    preserve_missing_defaults: true,
                })?;
                self.emit_spanned(
                    UnlinkedInstructionKind::CallMethodId {
                        dst,
                        receiver,
                        method: field.name,
                        method_id,
                        args,
                    },
                    span,
                );
                return Ok(dst);
            }
            if let Some(method_id) = value_receiver_type
                .as_ref()
                .and_then(|value| self.value_method_id_for_type(value, &field.name))
            {
                let params = self
                    .registry_value_method_params(value_receiver_type.as_ref(), &field.name)
                    .map(|params| registry_param_hints(params, span))
                    .unwrap_or_default();
                let args = self.compile_hir_method_arguments(HirMethodArguments {
                    method: &field.name,
                    receiver_type: value_receiver_type.as_ref(),
                    receiver_shape: receiver_shape.as_ref(),
                    params: &params,
                    arguments: &call.arguments,
                    call_span: span,
                    preserve_missing_defaults: false,
                })?;
                self.emit_spanned(
                    UnlinkedInstructionKind::CallMethodId {
                        dst,
                        receiver,
                        method: field.name,
                        method_id,
                        args,
                    },
                    span,
                );
                return Ok(dst);
            }
            if receiver_fact.is_some() || value_methods_known {
                return Err(unresolved_static_method_error(&field.name, span));
            }
            let mut args = Vec::with_capacity(call.arguments.len());
            for argument in &call.arguments {
                let value = argument
                    .value
                    .ok_or_else(|| hir_unsupported("call argument", argument.origin.span))?;
                args.push(DynamicCallArgument {
                    name: argument.name.clone(),
                    value: self.compile_hir_expression(value)?,
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
            return Ok(dst);
        }

        let dst = self.alloc_register()?;
        if let Some((declaration, name)) = self.script_function_call(call.expression) {
            let params = self
                .facts
                .script_function_signatures
                .get(&declaration)
                .cloned()
                .ok_or_else(|| hir_unsupported("script call", span))?;
            let (args, mode) = self.compile_hir_script_arguments(&params, &call.arguments, span)?;
            self.emit_spanned(
                UnlinkedInstructionKind::CallFunction {
                    dst,
                    target: vela_def::script_function_id(&name),
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
            let callee = self.compile_hir_expression(call.callee)?;
            let args = self.compile_hir_call_arguments(&call.arguments)?;
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
            let args = self.compile_hir_call_arguments(&call.arguments)?;
            let [src] = args.as_slice() else {
                return Err(hir_unsupported("set::from_array", span));
            };
            self.emit_spanned(
                UnlinkedInstructionKind::MakeSetFromArray { dst, src: *src },
                span,
            );
            return Ok(dst);
        }
        let native = self.resolve_native_function_id(&name, span)?;
        let args = self.compile_hir_native_arguments(&name, native, &call.arguments, span)?;
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

    pub(in crate::compiler) fn compile_hir_call_arguments(
        &mut self,
        arguments: &[vela_hir::body::HirArgument],
    ) -> CompileResult<Vec<Register>> {
        arguments
            .iter()
            .map(|argument| {
                argument
                    .value
                    .ok_or_else(|| hir_unsupported("call argument", argument.origin.span))
                    .and_then(|value| self.compile_hir_expression(value))
            })
            .collect()
    }

    pub(in crate::compiler) fn hir_call_arguments(
        &self,
        arguments: &[vela_hir::body::HirArgument],
    ) -> CompileResult<Vec<HirCallArgument>> {
        arguments
            .iter()
            .map(|argument| {
                let value = argument
                    .value
                    .ok_or_else(|| hir_unsupported("call argument", argument.origin.span))?;
                Ok(HirCallArgument {
                    name: argument.name.clone(),
                    span: argument.origin.span,
                    value,
                })
            })
            .collect()
    }

    pub(in crate::compiler) fn compile_hir_native_arguments(
        &mut self,
        function: &str,
        native: crate::FunctionId,
        arguments: &[vela_hir::body::HirArgument],
        call_span: Span,
    ) -> CompileResult<Vec<Register>> {
        let params = self
            .facts
            .registry
            .and_then(|registry| registry.function_params(native))
            .map(|params| registry_param_hints(params, call_span));
        let Some(params) = params else {
            if arguments.iter().any(|argument| argument.name.is_some()) {
                return Err(hir_unsupported("named native arguments", call_span));
            }
            return self.compile_hir_call_arguments(arguments);
        };
        if arguments.iter().all(|argument| argument.name.is_none()) {
            let mut registers = Vec::with_capacity(arguments.len());
            for (index, argument) in arguments.iter().enumerate() {
                let expression = argument
                    .value
                    .ok_or_else(|| hir_unsupported("native argument", argument.origin.span))?;
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
            return Ok(registers);
        }
        let args = self.hir_call_arguments(arguments)?;
        let slots =
            resolve_hir_call_arguments(&params, &args, call_span).map_err(|diagnostics| {
                CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
            })?;
        let mut registers = Vec::new();
        for (index, (slot, param)) in slots.into_iter().zip(params.iter()).enumerate() {
            let Some(arg) = slot else {
                continue;
            };
            let (register, _) = self.compile_hir_argument_for_expected_param(
                function,
                index,
                arg.value,
                param,
                &[],
                false,
            )?;
            registers.push(register);
        }
        Ok(registers)
    }

    pub(in crate::compiler) fn compile_hir_host_method_arguments(
        &mut self,
        method: vela_common::HostMethodId,
        arguments: &[vela_hir::body::HirArgument],
        call_span: Span,
    ) -> CompileResult<Vec<Register>> {
        let params = self
            .facts
            .registry
            .and_then(|registry| registry.host_method_params_by_runtime_id(method.get()))
            .map(|params| registry_param_hints(params, call_span));
        let Some(params) = params else {
            if arguments.iter().any(|argument| argument.name.is_some()) {
                return Err(hir_unsupported("named host method arguments", call_span));
            }
            return self.compile_hir_call_arguments(arguments);
        };
        if arguments.iter().all(|argument| argument.name.is_none()) {
            return self.compile_hir_call_arguments(arguments);
        }
        let args = self.hir_call_arguments(arguments)?;
        let slots =
            resolve_hir_call_arguments(&params, &args, call_span).map_err(|diagnostics| {
                CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
            })?;
        let mut registers = Vec::new();
        for (index, (slot, param)) in slots.into_iter().zip(params.iter()).enumerate() {
            let Some(arg) = slot else {
                continue;
            };
            let (register, _) = self.compile_hir_argument_for_expected_param(
                "host method",
                index,
                arg.value,
                param,
                &[],
                false,
            )?;
            registers.push(register);
        }
        Ok(registers)
    }

    pub(in crate::compiler) fn compile_hir_script_arguments(
        &mut self,
        params: &[ParamHint],
        arguments: &[vela_hir::body::HirArgument],
        call_span: Span,
    ) -> CompileResult<(Vec<CallArgument>, ScriptCallMode)> {
        let args = self.hir_call_arguments(arguments)?;
        let slots =
            resolve_hir_call_arguments(params, &args, call_span).map_err(|diagnostics| {
                CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
            })?;
        let mut mode = ScriptCallMode::Unchecked;
        let mut compiled = Vec::with_capacity(params.len());
        for (index, (slot, param)) in slots.into_iter().zip(params.iter()).enumerate() {
            if let Some(arg) = slot {
                let (register, requires_guard) = self.compile_hir_argument_for_expected_param(
                    "script function",
                    index,
                    arg.value,
                    param,
                    &[],
                    true,
                )?;
                if requires_guard {
                    mode = ScriptCallMode::Checked;
                }
                compiled.push(CallArgument::Register(register));
            } else {
                if param.type_hint.is_some() {
                    mode = ScriptCallMode::Checked;
                }
                compiled.push(CallArgument::Missing);
            }
        }
        Ok((compiled, mode))
    }

    fn compile_hir_method_arguments(
        &mut self,
        request: HirMethodArguments<'_>,
    ) -> CompileResult<Vec<CallArgument>> {
        let HirMethodArguments {
            method,
            receiver_type,
            receiver_shape,
            params,
            arguments,
            call_span,
            preserve_missing_defaults,
        } = request;
        if params.is_empty() && arguments.iter().all(|argument| argument.name.is_none()) {
            return self
                .compile_hir_call_arguments(arguments)
                .map(|args| args.into_iter().map(CallArgument::Register).collect());
        }
        let args = self.hir_call_arguments(arguments)?;
        let slots =
            resolve_hir_call_arguments(params, &args, call_span).map_err(|diagnostics| {
                CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
            })?;
        let mut compiled = Vec::with_capacity(params.len());
        for (index, (slot, param)) in slots.into_iter().zip(params.iter()).enumerate() {
            let Some(arg) = slot else {
                if preserve_missing_defaults {
                    compiled.push(CallArgument::Missing);
                }
                continue;
            };
            let callback_shapes = self.hir_callback_param_shapes(receiver_shape, method, arg.value);
            let expected =
                typed_container_mutation_arg_contract(receiver_type, method, &param.name, index);
            let (register, _) = if let Some(expected) = expected {
                self.compile_hir_expression_for_expected_type(
                    arg.value,
                    expected,
                    TypeContractContext::NativeParameter {
                        function: method.to_owned(),
                        name: mutation_arg_debug_name(method, &param.name, index),
                        index: u16::try_from(index).unwrap_or(u16::MAX),
                    },
                    callback_shapes.as_deref().unwrap_or(&[]),
                )?
            } else {
                self.compile_hir_argument_for_expected_param(
                    method,
                    index,
                    arg.value,
                    param,
                    callback_shapes.as_deref().unwrap_or(&[]),
                    false,
                )?
            };
            compiled.push(CallArgument::Register(register));
        }
        Ok(compiled)
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

    pub(in crate::compiler) fn hir_block_tail_expression(
        &self,
        block: HirBlockId,
    ) -> Option<HirExprId> {
        let statement = self
            .hir_bodies
            .iter()
            .find_map(|body| body.blocks.get(&block))?
            .statements
            .last()?;
        match self
            .hir_bodies
            .iter()
            .find_map(|body| body.statements.get(statement))?
            .kind
        {
            HirStmtKind::Expr {
                expression: Some(expression),
                ..
            } => Some(expression),
            _ => None,
        }
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
        type_name: &str,
        variant: &str,
        arguments: &[vela_hir::body::HirArgument],
        call_span: Span,
    ) -> CompileResult<Vec<(String, Register)>> {
        let Some(shape) = self.enum_constructor_shape(type_name, variant) else {
            if arguments.iter().any(|argument| argument.name.is_some()) {
                return Err(hir_unsupported(
                    "named tuple constructor arguments",
                    call_span,
                ));
            }
            return self.compile_hir_call_arguments(arguments).map(|values| {
                values
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| (index.to_string(), value))
                    .collect()
            });
        };
        let params = (0..shape.len())
            .map(|index| ParamHint {
                name: shape.argument_name_at(index).unwrap_or("").to_owned(),
                span: call_span,
                type_hint: None,
                default_value_span: shape.field_has_default_at(index).then_some(call_span),
                default_body: None,
            })
            .collect::<Vec<_>>();
        let field_uses = arguments
            .iter()
            .enumerate()
            .map(|(position, argument)| {
                let name = match argument.name.as_deref() {
                    Some(name) => shape
                        .argument_index(name)
                        .and_then(|index| shape.field_name_at(index))
                        .unwrap_or(name)
                        .to_owned(),
                    None => shape
                        .field_name_at(position)
                        .map(str::to_owned)
                        .unwrap_or_else(|| position.to_string()),
                };
                ConstructorFieldUse {
                    name,
                    span: argument.origin.span,
                }
            })
            .collect::<Vec<_>>();
        self.reject_constructor_diagnostics(record_constructor_field_diagnostics(
            &format!("{type_name}::{variant}"),
            Some(&shape),
            &field_uses,
            call_span,
        ))?;
        let args = self.hir_call_arguments(arguments)?;
        let slots =
            resolve_hir_call_arguments(&params, &args, call_span).map_err(|diagnostics| {
                CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
            })?;
        let mut fields = Vec::new();
        let mut explicit = BTreeSet::new();
        for (index, slot) in slots.into_iter().enumerate() {
            let Some(arg) = slot else {
                continue;
            };
            let field_name = shape.field_name_at(index).unwrap_or("").to_owned();
            let value = if let Some(expected) = shape.field_value_type_at(index) {
                self.compile_hir_expression_for_expected_type(
                    arg.value,
                    expected,
                    TypeContractContext::Field {
                        name: field_name.clone(),
                    },
                    &[],
                )?
                .0
            } else {
                self.compile_hir_expression(arg.value)?
            };
            explicit.insert(field_name.clone());
            fields.push((field_name, value));
        }
        self.compile_schema_default_fields(
            &mut fields,
            &explicit,
            schema_default_fields(Some(&shape)),
            Some(&shape),
        )?;
        Ok(fields)
    }
}
