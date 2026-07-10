use std::collections::BTreeMap;

use vela_analysis::registry::{RegistryFieldTargetFact, RegistryIndexCapabilityFact};
use vela_analysis::semantic_facts::{
    CallTargetFact, ConstructorTargetFact, HostPathIndexKindFact, HostPathSegmentFact,
    MemberTargetFact,
};
use vela_analysis::type_fact::TypeFact;
use vela_common::{PrimitiveTag, ScalarValue};
use vela_def::{FieldId, FunctionId, TypeId};
use vela_hir::body::{
    HirArgument, HirBody, HirBodyOwner, HirExprKind, HirLiteral, HirPathKind, HirPathOwner,
    HirPatternKind, HirRecordField,
};
use vela_hir::ids::{HirDeclId, HirExprId, HirPatternId, ModuleId};
use vela_hir::type_hint::{EnumVariantFieldsHint, HirTypeHint, ParamHint, StructFieldHint};
use vela_mir::{
    CompileCallTarget, CompileCalleeTarget, CompileConstructorField, CompileConstructorTarget,
    CompileConstructorValue, CompileDynamicCallArgument, CompileDynamicConstructorField,
    CompileFieldTarget, CompileHostIndexCapability, CompileHostPathSegment, CompileHostPathTarget,
    CompileMemberTarget, CompilePatternConstructorTarget, CompileReflectionCall,
    CompileScriptCallArgument, DynamicMethodTarget, HostFieldTarget, HostMethodTarget,
    MirBuildError, MirSourceOrigin,
};

use super::contracts::ContractBoundary;
use super::external::{external_signature, unresolved_method, unresolved_native};
use super::schema::{contract_from_fact, registry_hint_contract};
use super::{GenerationBuilder, input_error, registry_input_error};
use crate::compiler::call_args::{HirCallArgument, resolve_hir_call_arguments};
use crate::compiler::calls::metadata::registry_param_hints;
use crate::compiler::error::{CompileError, CompileErrorKind, CompileResult};
use crate::compiler::schema_defaults::{ConstructorFieldUse, record_constructor_field_diagnostics};

impl GenerationBuilder<'_, '_> {
    pub(super) fn insert_placements(&mut self) -> CompileResult<()> {
        for (function, root) in self.selected_executable_roots()? {
            for body_id in self.executable_body_ids(root) {
                let body = self
                    .request
                    .graph
                    .body(body_id)
                    .cloned()
                    .ok_or_else(registry_input_error)?;
                for expression in body.expressions.values() {
                    match &expression.kind {
                        HirExprKind::Call(_) => {
                            self.insert_call(function, &body, expression.id)?;
                        }
                        HirExprKind::Field(_) if !field_is_call_callee(&body, expression.id) => {
                            self.insert_member(function, &body, expression.id)?;
                        }
                        HirExprKind::Field(_) => {}
                        HirExprKind::Record { .. } => {
                            self.insert_constructor(function, &body, expression.id)?;
                        }
                        HirExprKind::Try { .. } => {
                            self.insert_try_target(function, &body, expression.id)?;
                        }
                        _ => {}
                    }
                    if self
                        .executable_analysis(function)?
                        .host_path_target(expression.id)
                        .is_some()
                    {
                        self.insert_host_path(function, expression.id)?;
                    }
                }
                for pattern in body.patterns.values() {
                    if matches!(
                        &pattern.kind,
                        HirPatternKind::Path { path: Some(_) }
                            | HirPatternKind::TupleVariant { path: Some(_), .. }
                            | HirPatternKind::RecordVariant { path: Some(_), .. }
                    ) {
                        self.insert_pattern_constructor(function, &body, pattern.id)?;
                    }
                }
                self.collect_typed_let_boundaries(function, &body);
            }
        }
        Ok(())
    }

    fn insert_call(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
        expression: HirExprId,
    ) -> CompileResult<()> {
        let call = body.call(expression).ok_or_else(registry_input_error)?;
        let origin = self
            .expression_origin(expression)
            .ok_or_else(registry_input_error)?;
        let target = self
            .executable_analysis(executable)?
            .call_target(expression)
            .cloned()
            .unwrap_or(CallTargetFact::Unresolved);

        if let Some(special) = self.host_behavior_call(executable, body, call, origin)? {
            self.targets
                .insert_call(executable, expression, special, origin)
                .map_err(input_error)?;
            return Ok(());
        }

        let placed = match target {
            CallTargetFact::Declaration(declaration) => {
                let function = self.function_ids.get(&declaration).copied().ok_or_else(|| {
                    input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: format!(
                            "call target references unindexed function declaration {declaration:?}"
                        ),
                    })
                })?;
                let signature = self
                    .request
                    .graph
                    .function_signature(declaration)
                    .ok_or_else(registry_input_error)?;
                let module = self
                    .request
                    .graph
                    .declaration(declaration)
                    .map(|declaration| declaration.module)
                    .ok_or_else(registry_input_error)?;
                CompileCallTarget::script(
                    CompileCalleeTarget::ScriptFunction {
                        function,
                        debug_name: self.request.script_function_symbols[&declaration].clone(),
                    },
                    self.script_arguments(
                        executable,
                        call,
                        signature.params.as_slice(),
                        module,
                        origin,
                    )?,
                )
            }
            CallTargetFact::Variant {
                enum_declaration,
                variant,
            } => {
                self.insert_variant_call_constructor(
                    executable,
                    body,
                    expression,
                    enum_declaration,
                    &variant,
                )?;
                return Ok(());
            }
            CallTargetFact::ScriptMethod { method } => {
                let field = body.field(call.callee).ok_or_else(registry_input_error)?;
                let owner = self
                    .owner_type_for_expression(executable, field.receiver)
                    .ok_or_else(registry_input_error)?;
                let method_target = self
                    .method_targets
                    .get(&(method, owner))
                    .copied()
                    .ok_or_else(registry_input_error)?;
                let method_input = self
                    .request
                    .script_methods
                    .iter()
                    .find(|candidate| {
                        candidate.node == method
                            && self.method_targets.get(&(candidate.node, owner))
                                == Some(&method_target)
                    })
                    .ok_or_else(registry_input_error)?;
                let module = self
                    .request
                    .graph
                    .declaration(method_input.bindings.declaration)
                    .map(|declaration| declaration.module)
                    .ok_or_else(registry_input_error)?;
                let arguments = self.script_arguments(
                    executable,
                    call,
                    method_input.signature.params.get(1..).unwrap_or_default(),
                    module,
                    origin,
                )?;
                self.targets
                    .insert_member(
                        executable,
                        call.callee,
                        CompileMemberTarget::ScriptMethod {
                            target: method_target,
                            debug_name: field.name.clone(),
                        },
                        self.expression_origin(call.callee).unwrap_or(origin),
                    )
                    .map_err(input_error)?;
                CompileCallTarget::script(
                    CompileCalleeTarget::ScriptMethod {
                        target: method_target,
                        debug_name: field.name.clone(),
                    },
                    arguments,
                )
            }
            CallTargetFact::Local(local) => CompileCallTarget::positional(
                CompileCalleeTarget::Local(local),
                positional_values(&call.arguments)?,
            ),
            CallTargetFact::Lambda(lambda) => CompileCallTarget::positional(
                CompileCalleeTarget::Lambda(lambda),
                positional_values(&call.arguments)?,
            ),
            CallTargetFact::RegistryFunction { path }
            | CallTargetFact::NativeFunction { path }
            | CallTargetFact::StdlibFunction { path } => {
                self.external_function_call(executable, &path, call, origin)?
            }
            CallTargetFact::HostMethod { owner, name } => {
                self.host_method_call(executable, &owner, &name, call, origin)?
            }
            CallTargetFact::RegistryMethod { owner, name } => {
                self.registry_method_call(executable, &owner, &name, call, origin)?
            }
            CallTargetFact::StdlibMethod { name } => {
                self.value_method_call(executable, body, call, &name, origin)?
            }
            CallTargetFact::KnownReceiverMiss { method, .. } => {
                return Err(unresolved_method(&method, origin.span));
            }
            CallTargetFact::Dynamic => self.dynamic_call(body, call, origin)?,
            CallTargetFact::Unresolved => {
                let path = callee_path(body, call.callee).ok_or_else(|| {
                    input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: format!(
                            "unresolved non-path call target for expression {expression:?}"
                        ),
                    })
                })?;
                let name = path.join("::");
                if self.registry_was_provided {
                    return Err(unresolved_native(&name, origin.span));
                }
                let function = self.ensure_derived_native_function(&name, origin)?;
                CompileCallTarget::positional(
                    CompileCalleeTarget::NativeFunction {
                        function,
                        debug_name: name,
                    },
                    positional_values(&call.arguments)?,
                )
            }
        };
        self.targets
            .insert_call(executable, expression, placed, origin)
            .map_err(input_error)
    }

    fn external_function_call(
        &mut self,
        executable: FunctionId,
        path: &str,
        call: &vela_hir::body::HirCall,
        origin: MirSourceOrigin,
    ) -> CompileResult<CompileCallTarget> {
        let definition = self.catalog.function_by_source(path).cloned();
        let function = match definition.as_ref() {
            Some(definition) => {
                self.ensure_external_function(definition.id, origin)?;
                definition.id
            }
            None if !self.registry_was_provided => {
                self.ensure_derived_native_function(path, origin)?
            }
            None => return Err(unresolved_native(path, origin.span)),
        };
        let arguments = definition.as_ref().map_or_else(
            || positional_values(&call.arguments),
            |definition| {
                self.external_arguments(
                    executable,
                    path,
                    &definition.signature.params,
                    call,
                    origin,
                )
            },
        )?;
        if path == "set::from_array" {
            return Ok(CompileCallTarget::positional(
                CompileCalleeTarget::SetFromArray {
                    function,
                    debug_name: path.to_owned(),
                },
                arguments,
            ));
        }
        if let Some(operation) = reflection_operation(path) {
            return Ok(CompileCallTarget::positional(
                CompileCalleeTarget::Reflection {
                    operation,
                    function,
                    debug_name: path.to_owned(),
                },
                arguments,
            ));
        }
        let callee = if definition
            .as_ref()
            .is_some_and(|definition| definition.path.package == "std")
        {
            CompileCalleeTarget::StdlibFunction {
                function,
                debug_name: path.to_owned(),
            }
        } else {
            CompileCalleeTarget::NativeFunction {
                function,
                debug_name: path.to_owned(),
            }
        };
        Ok(CompileCallTarget::positional(callee, arguments))
    }

    fn host_method_call(
        &mut self,
        executable: FunctionId,
        owner_name: &str,
        name: &str,
        call: &vela_hir::body::HirCall,
        origin: MirSourceOrigin,
    ) -> CompileResult<CompileCallTarget> {
        let owner = self
            .registry_facts
            .type_target_fact(owner_name)
            .map(|target| target.semantic)
            .ok_or_else(registry_input_error)?;
        let definition = self
            .catalog
            .method_by_owner_name(owner, name)
            .cloned()
            .ok_or_else(registry_input_error)?;
        self.ensure_external_method(definition.id, origin)?;
        let owner_target = self
            .host_type_target(owner)
            .ok_or_else(registry_input_error)?;
        let runtime = definition
            .host_runtime_id
            .map(vela_common::HostMethodId::new)
            .ok_or_else(registry_input_error)?;
        let descriptor = self
            .catalog
            .method(definition.id)
            .ok_or_else(registry_input_error)?;
        let signature = external_signature(
            &descriptor.signature,
            descriptor.effects,
            &self.catalog,
            origin,
        )?;
        let arguments =
            self.external_arguments(executable, name, &definition.signature.params, call, origin)?;
        Ok(CompileCallTarget::positional(
            CompileCalleeTarget::HostMethod(HostMethodTarget {
                owner: owner_target,
                semantic: definition.id,
                runtime,
                signature,
                access: vela_mir::CompileMethodAccess::new(
                    definition.access.public,
                    definition.access.reflect_callable,
                    definition.access.required_permissions().to_vec(),
                ),
            }),
            arguments,
        ))
    }

    fn registry_method_call(
        &mut self,
        executable: FunctionId,
        owner_name: &str,
        name: &str,
        call: &vela_hir::body::HirCall,
        origin: MirSourceOrigin,
    ) -> CompileResult<CompileCallTarget> {
        let owner = self
            .registry_facts
            .type_target_fact(owner_name)
            .map(|target| target.semantic)
            .ok_or_else(registry_input_error)?;
        let method = self
            .catalog
            .method_by_owner_name(owner, name)
            .cloned()
            .ok_or_else(registry_input_error)?;
        self.ensure_external_method(method.id, origin)?;
        let arguments =
            self.external_arguments(executable, name, &method.signature.params, call, origin)?;
        Ok(CompileCallTarget::positional(
            CompileCalleeTarget::ValueMethod {
                owner,
                method: method.id,
                debug_name: name.to_owned(),
            },
            arguments,
        ))
    }

    fn value_method_call(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
        call: &vela_hir::body::HirCall,
        name: &str,
        origin: MirSourceOrigin,
    ) -> CompileResult<CompileCallTarget> {
        let field = body.field(call.callee).ok_or_else(registry_input_error)?;
        let owner = self
            .owner_type_for_expression(executable, field.receiver)
            .ok_or_else(registry_input_error)?;
        let method = match self.catalog.method_by_owner_name(owner, name).cloned() {
            Some(method) => method,
            None => {
                let analysis = self.executable_analysis(executable)?;
                let owner_name = type_owner_name(analysis.expression(field.receiver))
                    .ok_or_else(registry_input_error)?;
                let method = vela_stdlib::std_method_id(owner_name, name)
                    .ok_or_else(registry_input_error)?;
                self.catalog
                    .method(method)
                    .cloned()
                    .ok_or_else(registry_input_error)?
            }
        };
        self.ensure_external_method(method.id, origin)?;
        let arguments =
            self.external_arguments(executable, name, &method.signature.params, call, origin)?;
        Ok(CompileCallTarget::positional(
            CompileCalleeTarget::ValueMethod {
                owner: method.owner,
                method: method.id,
                debug_name: name.to_owned(),
            },
            arguments,
        ))
    }

    fn dynamic_call(
        &self,
        body: &HirBody,
        call: &vela_hir::body::HirCall,
        origin: MirSourceOrigin,
    ) -> CompileResult<CompileCallTarget> {
        let arguments = dynamic_values(&call.arguments)?;
        if let Some(field) = body.field(call.callee) {
            return Ok(CompileCallTarget::dynamic(
                CompileCalleeTarget::DynamicMethod(DynamicMethodTarget::method(
                    &field.name,
                    checked_u32(
                        call.arguments
                            .iter()
                            .filter(|argument| argument.name.is_none())
                            .count(),
                        origin,
                        "dynamic positional arity",
                    )?,
                    call.arguments
                        .iter()
                        .filter_map(|argument| argument.name.clone())
                        .collect(),
                )),
                arguments,
            ));
        }
        Ok(CompileCallTarget::dynamic(
            CompileCalleeTarget::DynamicCallable,
            arguments,
        ))
    }

    fn host_behavior_call(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
        call: &vela_hir::body::HirCall,
        _origin: MirSourceOrigin,
    ) -> CompileResult<Option<CompileCallTarget>> {
        let Some(field) = body.field(call.callee) else {
            return Ok(None);
        };
        let Some(path) = self
            .executable_analysis(executable)?
            .host_path_target(field.receiver)
            .cloned()
        else {
            return Ok(None);
        };
        if field.name == "remove"
            && call.arguments.is_empty()
            && matches!(
                body.expression(field.receiver).map(|value| &value.kind),
                Some(HirExprKind::Index(_))
            )
        {
            let path = self.convert_host_path(executable, path)?;
            return Ok(Some(CompileCallTarget::positional(
                CompileCalleeTarget::HostRemove { path },
                Vec::new(),
            )));
        }
        if field.name == "push" && !path.segments.is_empty() && call.arguments.len() == 1 {
            let path = self.convert_host_path(executable, path)?;
            return Ok(Some(CompileCallTarget::positional(
                CompileCalleeTarget::HostPush { path },
                positional_values(&call.arguments)?,
            )));
        }
        Ok(None)
    }

    fn script_arguments(
        &mut self,
        executable: FunctionId,
        call: &vela_hir::body::HirCall,
        params: &[ParamHint],
        module: ModuleId,
        origin: MirSourceOrigin,
    ) -> CompileResult<Vec<CompileScriptCallArgument>> {
        let arguments = hir_call_arguments(&call.arguments)?;
        let slots = resolve_hir_call_arguments(params, &arguments, origin.span)
            .map_err(semantic_diagnostics)?;
        slots
            .into_iter()
            .zip(params)
            .enumerate()
            .map(|(index, (argument, parameter))| {
                if let (Some(argument), Some(hint)) = (&argument, &parameter.type_hint)
                    && let Some(contract) = self.type_contract_for_hint(module, hint)
                {
                    self.boundaries.push(ContractBoundary::function_parameter(
                        executable,
                        argument.value,
                        contract,
                        parameter.name.clone(),
                    ));
                }
                Ok(CompileScriptCallArgument {
                    parameter: checked_u32(index, origin, "script call parameter")?,
                    value: argument.map(|argument| argument.value),
                })
            })
            .collect()
    }

    fn external_arguments(
        &mut self,
        executable: FunctionId,
        debug_function: &str,
        params: &[vela_registry::ParamDef],
        call: &vela_hir::body::HirCall,
        origin: MirSourceOrigin,
    ) -> CompileResult<Vec<HirExprId>> {
        if call
            .arguments
            .iter()
            .all(|argument| argument.name.is_none())
        {
            let values = positional_values(&call.arguments)?;
            for (index, (value, parameter)) in values.iter().zip(params).enumerate() {
                if let Some(contract) = parameter
                    .type_hint
                    .as_ref()
                    .and_then(|hint| registry_hint_contract(hint, &self.catalog))
                    .and_then(super::schema::meaningful_contract)
                {
                    self.boundaries.push(ContractBoundary::native_parameter(
                        executable,
                        *value,
                        contract,
                        debug_function,
                        &parameter.name,
                        checked_u16(index, origin, "native parameter index")?,
                    ));
                }
            }
            return Ok(values);
        }

        let param_hints = registry_param_hints(params, origin.span);
        let arguments = hir_call_arguments(&call.arguments)?;
        let slots = resolve_hir_call_arguments(&param_hints, &arguments, origin.span)
            .map_err(semantic_diagnostics)?;
        let mut values = Vec::new();
        for (index, (slot, parameter)) in slots.into_iter().zip(params).enumerate() {
            let Some(slot) = slot else {
                continue;
            };
            if let Some(contract) = parameter
                .type_hint
                .as_ref()
                .and_then(|hint| registry_hint_contract(hint, &self.catalog))
                .and_then(super::schema::meaningful_contract)
            {
                self.boundaries.push(ContractBoundary::native_parameter(
                    executable,
                    slot.value,
                    contract,
                    debug_function,
                    &parameter.name,
                    checked_u16(index, origin, "native parameter index")?,
                ));
            }
            values.push(slot.value);
        }
        Ok(values)
    }

    fn insert_member(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
        expression: HirExprId,
    ) -> CompileResult<()> {
        let origin = self
            .expression_origin(expression)
            .ok_or_else(registry_input_error)?;
        let target = self
            .executable_analysis(executable)?
            .member_target(expression)
            .cloned()
            .ok_or_else(|| {
                input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: format!("missing analysis member target for {expression:?}"),
                })
            })?;
        let placed = match target {
            MemberTargetFact::ScriptField {
                owner,
                variant,
                name,
            } => self.script_field_target(owner, variant.as_deref(), &name)?,
            MemberTargetFact::HostField(field) => {
                CompileMemberTarget::HostField(self.host_field_target(&field, origin)?)
            }
            MemberTargetFact::LogicalRecordField(field) => {
                self.ensure_logical_record(field.kind, origin)?;
                CompileMemberTarget::ScriptField(CompileFieldTarget::RecordSlot {
                    type_id: field.type_id,
                    shape: field.shape,
                    field: field.field,
                })
            }
            MemberTargetFact::RegistryField { owner, name } => {
                self.registry_field_target(&owner, &name, origin)?
            }
            MemberTargetFact::RegistryMethod { owner, name } => {
                let owner = self
                    .registry_facts
                    .type_target_fact(&owner)
                    .map(|target| target.semantic)
                    .ok_or_else(registry_input_error)?;
                let method = self
                    .catalog
                    .method_by_owner_name(owner, &name)
                    .cloned()
                    .ok_or_else(registry_input_error)?;
                self.ensure_external_method(method.id, origin)?;
                CompileMemberTarget::ValueMethod {
                    owner,
                    method: method.id,
                    debug_name: name,
                }
            }
            MemberTargetFact::StdlibMethod { name } => {
                let field = body.field(expression).ok_or_else(registry_input_error)?;
                let owner = self
                    .owner_type_for_expression(executable, field.receiver)
                    .ok_or_else(registry_input_error)?;
                let method = self
                    .catalog
                    .method_by_owner_name(owner, &name)
                    .cloned()
                    .ok_or_else(registry_input_error)?;
                self.ensure_external_method(method.id, origin)?;
                CompileMemberTarget::ValueMethod {
                    owner,
                    method: method.id,
                    debug_name: name,
                }
            }
            MemberTargetFact::TupleIndex(index) => {
                CompileMemberTarget::TupleIndex(checked_u32(index, origin, "tuple member index")?)
            }
            MemberTargetFact::Dynamic => {
                let name = body
                    .field(expression)
                    .map(|field| field.name.clone())
                    .ok_or_else(registry_input_error)?;
                CompileMemberTarget::Dynamic { name }
            }
            MemberTargetFact::Unresolved => {
                return Err(input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: format!("unresolved analysis member target for {expression:?}"),
                }));
            }
        };
        self.targets
            .insert_member(executable, expression, placed, origin)
            .map_err(input_error)
    }

    fn script_field_target(
        &self,
        owner: HirDeclId,
        variant: Option<&str>,
        name: &str,
    ) -> CompileResult<CompileMemberTarget> {
        let type_id = self.type_ids[&owner];
        let field = self.field_ids[&(owner, variant.map(str::to_owned), name.to_owned())];
        if let Some(variant) = variant {
            return Ok(CompileMemberTarget::ScriptField(
                CompileFieldTarget::VariantSlot {
                    type_id,
                    variant: self.variant_ids[&(owner, variant.to_owned())],
                    field,
                },
            ));
        }
        Ok(CompileMemberTarget::ScriptField(
            CompileFieldTarget::RecordSlot {
                type_id,
                shape: self.type_shapes[&type_id],
                field,
            },
        ))
    }

    fn registry_field_target(
        &mut self,
        owner_name: &str,
        name: &str,
        origin: MirSourceOrigin,
    ) -> CompileResult<CompileMemberTarget> {
        let (type_name, variant_name) = owner_name
            .rsplit_once("::")
            .map_or((owner_name, None), |(owner, variant)| {
                (owner, Some(variant))
            });
        let owner = self
            .registry_facts
            .type_target_fact(type_name)
            .or_else(|| self.registry_facts.type_target_fact(owner_name))
            .map(|target| target.semantic)
            .ok_or_else(registry_input_error)?;
        let variant = variant_name
            .and_then(|variant| self.catalog.variant_by_owner_name(owner, variant))
            .map(|variant| variant.id);
        let field = self
            .catalog
            .field_by_owner_name(owner, variant, name)
            .cloned()
            .ok_or_else(registry_input_error)?;
        self.ensure_external_field(field.id, origin)?;
        if let Some(variant) = variant {
            return Ok(CompileMemberTarget::ScriptField(
                CompileFieldTarget::VariantSlot {
                    type_id: owner,
                    variant,
                    field: field.id,
                },
            ));
        }
        let shape = self
            .external_shape(owner)
            .ok_or_else(registry_input_error)?;
        Ok(CompileMemberTarget::ScriptField(
            CompileFieldTarget::RecordSlot {
                type_id: owner,
                shape,
                field: field.id,
            },
        ))
    }

    fn insert_host_path(
        &mut self,
        executable: FunctionId,
        expression: HirExprId,
    ) -> CompileResult<()> {
        let fact = self
            .executable_analysis(executable)?
            .host_path_target(expression)
            .cloned()
            .ok_or_else(registry_input_error)?;
        let origin = self
            .expression_origin(expression)
            .ok_or_else(registry_input_error)?;
        let path = self.convert_host_path(executable, fact)?;
        self.targets
            .insert_host_path(executable, expression, path, origin)
            .map_err(input_error)
    }

    fn convert_host_path(
        &mut self,
        executable: FunctionId,
        fact: vela_analysis::semantic_facts::HostPathTargetFact,
    ) -> CompileResult<CompileHostPathTarget> {
        let origin = self
            .expression_origin(fact.root)
            .ok_or_else(registry_input_error)?;
        self.ensure_external_type(fact.root_type.semantic, origin)?;
        let root_type = self
            .host_type_target(fact.root_type.semantic)
            .ok_or_else(registry_input_error)?;
        let mut segments = Vec::new();
        for segment in fact.segments {
            match segment {
                HostPathSegmentFact::Field(field) => {
                    let target = self.host_field_target(&field, origin)?;
                    segments.push(if field.variant_field {
                        CompileHostPathSegment::VariantField(target)
                    } else {
                        CompileHostPathSegment::Field(target)
                    });
                }
                HostPathSegmentFact::Index {
                    expression,
                    kind,
                    capability,
                    ..
                } => {
                    let capability = self.host_index_capability(&capability, origin);
                    let constant = self.constant_host_index(executable, expression)?;
                    segments.push(match (kind, constant) {
                        (HostPathIndexKindFact::Index, Some(ConstantHostIndex::Index(value))) => {
                            CompileHostPathSegment::ConstantIndex { value, capability }
                        }
                        (HostPathIndexKindFact::Key, Some(ConstantHostIndex::Key(value))) => {
                            CompileHostPathSegment::ConstantKey { value, capability }
                        }
                        (HostPathIndexKindFact::Index, _) => CompileHostPathSegment::DynamicIndex {
                            expression,
                            capability,
                        },
                        (HostPathIndexKindFact::Key, _) => CompileHostPathSegment::DynamicKey {
                            expression,
                            capability,
                        },
                    });
                }
            }
        }
        Ok(CompileHostPathTarget {
            root: fact.root,
            root_type,
            segments,
        })
    }

    fn host_field_target(
        &mut self,
        fact: &RegistryFieldTargetFact,
        origin: MirSourceOrigin,
    ) -> CompileResult<HostFieldTarget> {
        self.ensure_external_field(fact.semantic, origin)?;
        let owner = self
            .host_type_target(fact.owner)
            .ok_or_else(registry_input_error)?;
        Ok(HostFieldTarget {
            owner,
            semantic: fact.semantic,
            runtime: fact.host_runtime.ok_or_else(registry_input_error)?,
            access: vela_mir::CompileFieldAccess::new(
                fact.access.readable,
                fact.access.writable,
                fact.access.reflect_readable,
                fact.access.reflect_writable,
                fact.access.required_permissions.clone(),
            ),
        })
    }

    fn host_index_capability(
        &mut self,
        capability: &RegistryIndexCapabilityFact,
        origin: MirSourceOrigin,
    ) -> CompileHostIndexCapability {
        let capability = CompileHostIndexCapability {
            readable: capability.readable,
            writable: capability.writable,
            mutable: capability.addable,
            removable: capability.removable,
            key: contract_from_fact(
                &capability.key,
                &self.registry_facts,
                self.request.graph,
                &self.type_ids,
                &self.type_shapes,
            )
            .and_then(super::schema::meaningful_contract),
            value: contract_from_fact(
                &capability.value,
                &self.registry_facts,
                self.request.graph,
                &self.type_ids,
                &self.type_shapes,
            )
            .and_then(super::schema::meaningful_contract),
        };
        if let Some(contract) = &capability.key {
            self.remember_contract(contract, origin);
        }
        if let Some(contract) = &capability.value {
            self.remember_contract(contract, origin);
        }
        capability
    }

    fn constant_host_index(
        &self,
        executable: FunctionId,
        expression: HirExprId,
    ) -> CompileResult<Option<ConstantHostIndex>> {
        let Some(body) = self.body_for_expression(expression) else {
            return Ok(None);
        };
        let Some(record) = body.expression(expression) else {
            return Ok(None);
        };
        Ok(match &record.kind {
            HirExprKind::Literal(HirLiteral::String(value)) => {
                Some(ConstantHostIndex::Key(value.clone()))
            }
            HirExprKind::Literal(HirLiteral::Integer(_)) => {
                let Some(scalar) = self
                    .executable_analysis(executable)?
                    .literal(expression)
                    .and_then(|literal| literal.as_ref().ok())
                    .and_then(|literal| literal.scalar())
                else {
                    return Ok(None);
                };
                match scalar {
                    ScalarValue::I64(value) => {
                        u32::try_from(value).ok().map(ConstantHostIndex::Index)
                    }
                    ScalarValue::U64(value) => {
                        u32::try_from(value).ok().map(ConstantHostIndex::Index)
                    }
                    ScalarValue::I8(value) => {
                        u32::try_from(value).ok().map(ConstantHostIndex::Index)
                    }
                    ScalarValue::I16(value) => {
                        u32::try_from(value).ok().map(ConstantHostIndex::Index)
                    }
                    ScalarValue::I32(value) => {
                        u32::try_from(value).ok().map(ConstantHostIndex::Index)
                    }
                    ScalarValue::U8(value) => Some(ConstantHostIndex::Index(u32::from(value))),
                    ScalarValue::U16(value) => Some(ConstantHostIndex::Index(u32::from(value))),
                    ScalarValue::U32(value) => Some(ConstantHostIndex::Index(value)),
                    ScalarValue::F32(_) | ScalarValue::F64(_) => None,
                }
            }
            _ => None,
        })
    }
}

pub(super) fn runtime_semantic_body(body: &HirBody) -> bool {
    matches!(
        body.owner,
        HirBodyOwner::Declaration(_)
            | HirBodyOwner::TraitDefaultMethod(_)
            | HirBodyOwner::ImplMethod(_)
            | HirBodyOwner::Lambda { .. }
            | HirBodyOwner::ParameterDefault { .. }
    )
}

mod constructors;
mod helpers;
use self::helpers::*;
