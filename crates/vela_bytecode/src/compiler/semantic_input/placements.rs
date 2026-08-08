use vela_analysis::registry::{RegistryFieldTargetFact, RegistryIndexCapabilityFact};
use vela_analysis::semantic_facts::{
    CallTargetFact, ConstructorTargetFact, HostPathIndexKindFact, HostPathSegmentFact,
    MemberTargetFact,
};
use vela_analysis::type_fact::TypeFact;
use vela_analysis::validation::{
    CallParameterSlotValueFact, CallPlacementModeFact, HostAccessUseFact, HostAccessUseKind,
    HostIndexCapabilityResolutionFact,
};
use vela_common::{Detachability, PrimitiveTag, ScalarValue};
use vela_def::{FieldId, FunctionId, TypeId};
use vela_hir::binding::{BindingResolution, TaskLexicalCapability};
use vela_hir::body::{
    HirAssignOp, HirBody, HirBodyOwner, HirExprKind, HirLiteral, HirPathKind, HirPathOwner,
    HirPatternKind,
};
use vela_hir::ids::{HirDeclId, HirExprId, HirPatternId};
use vela_hir::type_hint::{HirTypeHint, StructFieldHint};
use vela_mir::{
    CompileCallArguments, CompileCallTarget, CompileCalleeTarget, CompileConstructorField,
    CompileConstructorTarget, CompileConstructorValue, CompileDynamicConstructorField,
    CompileFieldTarget, CompileHostIndexCapability, CompileHostPathSegment, CompileHostPathTarget,
    CompileMemberTarget, CompileParameterDefault, CompilePatternConstructorTarget,
    CompilePlacedCallValue, CompileReflectionCall, CompileTaskContinuationTarget,
    CompileTaskDetachability, CompileTaskOperation, CompileTaskTarget, DynamicMethodTarget,
    HostFieldTarget, HostMethodTarget, MirBuildError, MirSourceOrigin,
};

use super::contracts::{
    ContractBoundary, mutation_arg_debug_name, typed_container_mutation_arg_fact,
};
use super::external::{external_signature, unresolved_method, unresolved_native};
use super::schema::contract_from_fact;
use super::{GenerationBuilder, input_error, registry_input_error};
use crate::compiler::error::{CompileError, CompileErrorKind, CompileResult};

fn assignment_field_expression(body: &HirBody, mut expression: HirExprId) -> Option<HirExprId> {
    loop {
        match &body.expressions.get(&expression)?.kind {
            HirExprKind::Paren {
                expression: Some(inner),
            } => expression = *inner,
            HirExprKind::Field(_) => return Some(expression),
            _ => return None,
        }
    }
}

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
                        HirExprKind::Path(_)
                            if self
                                .executable_analysis(function)?
                                .constructor_target(expression.id)
                                .is_some() =>
                        {
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
                    } else if let Some(path) =
                        self.derived_host_index_path(function, &body, expression.id)?
                    {
                        let origin = self
                            .expression_origin(expression.id)
                            .ok_or_else(registry_input_error)?;
                        self.targets
                            .insert_host_path(function, expression.id, path, origin)
                            .map_err(input_error)?;
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
                self.collect_field_assignment_boundaries(function, &body)?;
            }
        }
        Ok(())
    }

    fn collect_field_assignment_boundaries(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
    ) -> CompileResult<()> {
        let assignments = body
            .expressions
            .values()
            .filter_map(|expression| match &expression.kind {
                HirExprKind::Assign {
                    op: Some(HirAssignOp::Set),
                    target: Some(target),
                    value: Some(value),
                } => Some((*target, *value)),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (target, value) in assignments {
            let Some(target) = assignment_field_expression(body, target) else {
                continue;
            };
            if !matches!(
                self.executable_analysis(executable)?.member_target(target),
                Some(MemberTargetFact::ScriptField { .. })
            ) {
                continue;
            }
            let Some(contract) = self
                .executable_analysis(executable)?
                .expression(target)
                .and_then(|fact| {
                    contract_from_fact(
                        fact,
                        &self.registry_facts,
                        self.request.graph,
                        &self.type_ids,
                        &self.type_shapes,
                    )
                })
                .and_then(super::schema::meaningful_contract)
            else {
                continue;
            };
            let name = body
                .field(target)
                .map(|field| field.name.clone())
                .ok_or_else(registry_input_error)?;
            self.boundaries
                .push(ContractBoundary::field(executable, value, contract, name));
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
        if self.insert_task_call(executable, body, expression, call, origin)? {
            return Ok(());
        }
        if let Some(target) = self.service_call_target(executable, body, call, origin)? {
            self.targets
                .insert_call(executable, expression, target, origin)
                .map_err(input_error)?;
            return Ok(());
        }
        let placement = self.checked_call_placement(executable, call, origin)?;
        let target = require_analysis_call_target(
            self.executable_analysis(executable)?
                .call_target(expression),
            expression,
            origin,
        )?;

        if let Some(special) =
            self.host_access_call(executable, body, expression, &target, &placement, origin)?
        {
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
                let (evaluation_order, parameter_slots) = self.script_argument_slots(
                    executable,
                    &placement,
                    signature.params.as_slice(),
                    module,
                    origin,
                )?;
                CompileCallTarget::script(
                    CompileCalleeTarget::ScriptFunction {
                        function,
                        debug_name: self.request.script_function_symbols[&declaration].clone(),
                    },
                    evaluation_order,
                    parameter_slots,
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
                    &placement,
                )?;
                return Ok(());
            }
            CallTargetFact::RegistryVariant { owner, variant } => {
                self.insert_registry_variant_call_constructor(
                    executable, body, expression, &owner, &variant, &placement,
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
                    .methods()
                    .find(|candidate| {
                        candidate.node() == method
                            && self.method_targets.get(&(candidate.node(), owner))
                                == Some(&method_target)
                    })
                    .ok_or_else(registry_input_error)?;
                let (evaluation_order, parameter_slots) = self.script_argument_slots(
                    executable,
                    &placement,
                    method_input.signature().params.get(1..).unwrap_or_default(),
                    method_input.signature_module(),
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
                    evaluation_order,
                    parameter_slots,
                )
            }
            CallTargetFact::Local(local) => CompileCallTarget::positional(
                CompileCalleeTarget::Local(local),
                self.positional_argument_values(
                    &placement,
                    CallPlacementModeFact::Positional,
                    origin,
                )?,
            ),
            CallTargetFact::Lambda(lambda) => CompileCallTarget::positional(
                CompileCalleeTarget::Lambda(lambda),
                self.positional_argument_values(
                    &placement,
                    CallPlacementModeFact::Positional,
                    origin,
                )?,
            ),
            CallTargetFact::RegistryFunction { path }
            | CallTargetFact::NativeFunction { path }
            | CallTargetFact::StdlibFunction { path } => {
                self.external_function_call(executable, &path, &placement, origin)?
            }
            CallTargetFact::HostMethod { .. } => {
                return Err(input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: "host method call is missing its HostAccess use fact".to_owned(),
                }));
            }
            CallTargetFact::RegistryMethod { owner, name } => {
                self.registry_method_call(executable, &owner, &name, &placement, origin)?
            }
            CallTargetFact::StdlibMethod { name } => {
                self.value_method_call(executable, body, call, &placement, &name, origin)?
            }
            CallTargetFact::KnownReceiverMiss { method, .. } => {
                return Err(unresolved_method(&method, origin.span));
            }
            CallTargetFact::Dynamic => self.dynamic_call(body, call, &placement, origin)?,
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
                    self.positional_argument_values(
                        &placement,
                        CallPlacementModeFact::Unresolved,
                        origin,
                    )?,
                )
            }
        };
        self.targets
            .insert_call(executable, expression, placed, origin)
            .map_err(input_error)
    }

    fn insert_task_call(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
        expression: HirExprId,
        call: &vela_hir::body::HirCall,
        origin: MirSourceOrigin,
    ) -> CompileResult<bool> {
        let Some(bindings) = self.request.graph.bindings_for_body(body.id) else {
            return Ok(false);
        };
        let Some(capability) = bindings.task_capability(call.callee) else {
            return Ok(false);
        };
        let worker_call = call
            .arguments
            .first()
            .and_then(|argument| argument.value)
            .ok_or_else(registry_input_error)?;
        let worker = body.call(worker_call).ok_or_else(registry_input_error)?;
        let worker_declaration = match self
            .executable_analysis(executable)?
            .call_target(worker_call)
        {
            Some(CallTargetFact::Declaration(declaration)) => *declaration,
            _ => return Err(registry_input_error()),
        };
        let worker_function = self
            .function_ids
            .get(&worker_declaration)
            .copied()
            .ok_or_else(registry_input_error)?;
        if !matches!(
            bindings.resolution(worker.callee),
            Some(BindingResolution::Declaration(declaration)) if *declaration == worker_declaration
        ) {
            return Err(registry_input_error());
        }
        let worker_origin = self
            .expression_origin(worker_call)
            .ok_or_else(registry_input_error)?;
        let placement = self.checked_call_placement(executable, worker, worker_origin)?;
        let signature = self
            .request
            .graph
            .function_signature(worker_declaration)
            .cloned()
            .ok_or_else(registry_input_error)?;
        let slots = self.strict_parameter_slots(&placement, &signature.params, worker_origin)?;
        let descriptor = self
            .targets
            .target_table()
            .function(worker_function)
            .cloned()
            .ok_or_else(registry_input_error)?;
        if descriptor.signature.parameters.len() != slots.len() {
            return Err(registry_input_error());
        }
        let analysis = self.executable_analysis(executable)?;
        let mut parameter_detachability = Vec::with_capacity(slots.len());
        for (index, (slot, parameter)) in slots
            .iter()
            .zip(&descriptor.signature.parameters)
            .enumerate()
        {
            let expected = vela_mir::contract_detachability(
                self.targets.target_table(),
                parameter.contract.as_ref(),
            );
            if let Some(kind) = expected.fact.rejection() {
                let suffix = expected.rejection_path.concat();
                return Err(CompileError::new(CompileErrorKind::TaskValueNotDetachable {
                    target: descriptor.debug_name.clone(),
                    path: format!("parameter `{}`{suffix}", parameter.name),
                    kind,
                })
                .with_span(parameter.origin.unwrap_or(worker_origin).span));
            }
            let explicit_value = match &slot.value {
                CallParameterSlotValueFact::Explicit { value, .. } => {
                    Some(value.ok_or_else(registry_input_error)?)
                }
                CallParameterSlotValueFact::MissingDefault => None,
            };
            let actual = explicit_value.map_or(Detachability::Detachable, |value| {
                analysis
                    .expression(value)
                    .map(TypeFact::detachability)
                    .unwrap_or(Detachability::RuntimeChecked)
            });
            if let Some(kind) = actual.rejection() {
                let span = match explicit_value {
                    Some(value) => self
                        .expression_origin(value)
                        .map_or(worker_origin.span, |origin| origin.span),
                    None => worker_origin.span,
                };
                return Err(CompileError::new(CompileErrorKind::TaskValueNotDetachable {
                    target: descriptor.debug_name.clone(),
                    path: format!("argument for parameter `{}`", parameter.name),
                    kind,
                })
                .with_span(span));
            }
            debug_assert_eq!(slot.parameter_index, index);
            parameter_detachability.push(expected.fact.union(actual));
        }
        let result = vela_mir::contract_detachability(
            self.targets.target_table(),
            descriptor.signature.return_contract.as_ref(),
        );
        if let Some(kind) = result.fact.rejection() {
            let suffix = result.rejection_path.concat();
            return Err(CompileError::new(CompileErrorKind::TaskValueNotDetachable {
                target: descriptor.debug_name.clone(),
                path: format!("return value{suffix}"),
                kind,
            })
            .with_span(worker_origin.span));
        }
        let continuation = if capability == TaskLexicalCapability::SpawnScopedThen {
            let expression = call
                .arguments
                .get(1)
                .and_then(|argument| argument.value)
                .ok_or_else(registry_input_error)?;
            let Some(BindingResolution::Declaration(declaration)) = bindings.resolution(expression)
            else {
                return Err(registry_input_error());
            };
            let function = self
                .function_ids
                .get(declaration)
                .copied()
                .ok_or_else(registry_input_error)?;
            let continuation_descriptor = self
                .targets
                .target_table()
                .function(function)
                .cloned()
                .ok_or_else(registry_input_error)?;
            let outcome_contract = vela_mir::MirTypeContract::Result {
                ok: descriptor.signature.return_contract.clone().map(Box::new),
                err: Some(Box::new(vela_mir::MirTypeContract::TaskError)),
            };
            let continuation_origin = self
                .expression_origin(expression)
                .ok_or_else(registry_input_error)?;
            let invalid = |reason: String| {
                CompileError::new(CompileErrorKind::TaskContinuationInvalid {
                    target: continuation_descriptor.debug_name.clone(),
                    reason,
                })
                .with_span(continuation_origin.span)
            };
            let Some(outcome) = continuation_descriptor.signature.parameters.first() else {
                return Err(invalid(format!(
                    "first parameter must be `{}`",
                    task_outcome_contract_name(&outcome_contract)
                )));
            };
            if outcome.default != CompileParameterDefault::Required {
                return Err(invalid(
                    "the owned outcome parameter cannot have a default".to_owned(),
                ));
            }
            if outcome.contract.as_ref() != Some(&outcome_contract) {
                return Err(invalid(format!(
                    "first parameter must be exactly `{}`",
                    task_outcome_contract_name(&outcome_contract)
                )));
            }
            Some(CompileTaskContinuationTarget {
                function,
                debug_name: self.request.script_function_symbols[declaration].clone(),
                outcome_contract,
                resume_parameters: continuation_descriptor
                    .signature
                    .parameters
                    .into_iter()
                    .skip(1)
                    .collect(),
            })
        } else {
            None
        };
        let task = CompileTaskTarget {
            operation: match capability {
                TaskLexicalCapability::SpawnScoped => CompileTaskOperation::SpawnScoped,
                TaskLexicalCapability::SpawnScopedThen => CompileTaskOperation::SpawnScopedThen,
            },
            worker_call,
            worker: worker_function,
            worker_debug_name: self.request.script_function_symbols[&worker_declaration].clone(),
            detachability: CompileTaskDetachability {
                parameters: parameter_detachability,
                result: result.fact,
            },
            continuation,
        };
        self.targets
            .insert_task(executable, expression, task, origin)
            .map_err(input_error)?;
        Ok(true)
    }

    fn external_function_call(
        &mut self,
        executable: FunctionId,
        path: &str,
        placement: &vela_analysis::validation::CallArgumentPlacementFact,
        origin: MirSourceOrigin,
    ) -> CompileResult<CompileCallTarget> {
        let Some(definition) = self.catalog.function_by_source(path).cloned() else {
            if self.registry_was_provided {
                return Err(unresolved_native(path, origin.span));
            }
            return Err(input_error(MirBuildError::InconsistentInput {
                origin,
                message: format!(
                    "resolved external call `{path}` has no authoritative compile manifest"
                ),
            }));
        };
        self.ensure_external_function(definition.id, origin)?;
        let callee = if path == "set::from_array" {
            CompileCalleeTarget::SetFromArray {
                function: definition.id,
                debug_name: path.to_owned(),
            }
        } else if let Some(operation) = reflection_operation(path) {
            CompileCalleeTarget::Reflection {
                operation,
                function: definition.id,
                debug_name: path.to_owned(),
            }
        } else if definition.path.package == "std" {
            CompileCalleeTarget::StdlibFunction {
                function: definition.id,
                debug_name: path.to_owned(),
            }
        } else {
            CompileCalleeTarget::NativeFunction {
                function: definition.id,
                debug_name: path.to_owned(),
            }
        };
        if matches!(&callee, CompileCalleeTarget::SetFromArray { .. }) {
            self.set_from_array_call_target(
                executable,
                callee,
                &definition.signature.params,
                placement,
                origin,
            )
        } else {
            self.external_call_target(
                executable,
                callee,
                path,
                &definition.signature.params,
                placement,
                origin,
            )
        }
    }

    fn host_method_call(
        &mut self,
        executable: FunctionId,
        owner_name: &str,
        name: &str,
        placement: &vela_analysis::validation::CallArgumentPlacementFact,
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
            self.request.options,
            origin,
        )?;
        self.external_call_target(
            executable,
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
                scoped_resource_return: definition.scoped_resource_return,
            }),
            name,
            &definition.signature.params,
            placement,
            origin,
        )
    }

    fn registry_method_call(
        &mut self,
        executable: FunctionId,
        owner_name: &str,
        name: &str,
        placement: &vela_analysis::validation::CallArgumentPlacementFact,
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
        self.external_call_target(
            executable,
            CompileCalleeTarget::ValueMethod {
                owner,
                method: method.id,
                debug_name: name.to_owned(),
            },
            name,
            &method.signature.params,
            placement,
            origin,
        )
    }

    fn value_method_call(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
        call: &vela_hir::body::HirCall,
        placement: &vela_analysis::validation::CallArgumentPlacementFact,
        name: &str,
        origin: MirSourceOrigin,
    ) -> CompileResult<CompileCallTarget> {
        let field = body.field(call.callee).ok_or_else(registry_input_error)?;
        let owner = self
            .owner_type_for_expression(executable, field.receiver)
            .ok_or_else(registry_input_error)?;
        let receiver_fact = match self.direct_declared_receiver_fact(body, field.receiver) {
            Some(declared) => Some(declared),
            None => self
                .executable_analysis(executable)?
                .expression(field.receiver)
                .cloned(),
        };
        if matches!(
            receiver_fact,
            Some(TypeFact::Iterator { .. } | TypeFact::ScopedIterator { .. })
        ) && matches!(name, "map" | "filter" | "take" | "skip")
        {
            let arguments = self.dynamic_argument_values_from_source(placement, origin)?;
            let scoped_resource = receiver_fact.as_ref().and_then(|receiver| {
                vela_analysis::stdlib::scoped_iterator_resource(receiver, name)
            });
            return Ok(CompileCallTarget::dynamic(
                CompileCalleeTarget::DynamicMethod(DynamicMethodTarget::method(
                    name,
                    checked_u32(
                        placement
                            .source_order
                            .iter()
                            .filter(|argument| argument.name.is_none())
                            .count(),
                        origin,
                        "dynamic positional arity",
                    )?,
                    placement
                        .source_order
                        .iter()
                        .filter_map(|argument| argument.name.clone())
                        .collect(),
                )),
                arguments,
            )
            .with_scoped_resource(scoped_resource));
        }
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
        let target = self
            .external_call_target(
                executable,
                CompileCalleeTarget::ValueMethod {
                    owner: method.owner,
                    method: method.id,
                    debug_name: name.to_owned(),
                },
                name,
                &method.signature.params,
                placement,
                origin,
            )?
            .with_scoped_resource(receiver_fact.as_ref().and_then(|receiver| {
                vela_analysis::stdlib::scoped_iterator_resource(receiver, name)
            }));
        self.insert_typed_container_mutation_contracts(
            executable,
            receiver_fact.as_ref(),
            name,
            &method.signature.params,
            &target.arguments,
            origin,
        )?;
        Ok(target)
    }

    fn insert_typed_container_mutation_contracts(
        &mut self,
        executable: FunctionId,
        receiver: Option<&TypeFact>,
        method: &str,
        params: &[vela_registry::ParamDef],
        arguments: &CompileCallArguments,
        origin: MirSourceOrigin,
    ) -> CompileResult<()> {
        for (index, parameter) in params.iter().enumerate() {
            let Some(expected) =
                typed_container_mutation_arg_fact(receiver, method, &parameter.name, index)
            else {
                continue;
            };
            let Some(expression) = external_parameter_expression(arguments, index) else {
                continue;
            };
            let actual = self
                .body_for_expression(expression)
                .and_then(|body| self.direct_declared_receiver_fact(body, expression))
                .or_else(|| {
                    self.executable_analysis(executable)
                        .ok()?
                        .expression(expression)
                        .cloned()
                });
            if method == "extend"
                && let Some(actual) = actual.as_ref()
                && borrowed_collection_source(actual, &expected)
            {
                self.remove_native_parameter_boundary(
                    executable,
                    expression,
                    method,
                    mutation_arg_debug_name(method, &parameter.name, index),
                    checked_u32(index, origin, "typed container mutation parameter")?,
                );
                continue;
            }
            let Some(contract) = contract_from_fact(
                &expected,
                &self.registry_facts,
                self.request.graph,
                &self.type_ids,
                &self.type_shapes,
            )
            .and_then(super::schema::meaningful_contract) else {
                continue;
            };
            self.replace_native_parameter_boundary(
                executable,
                expression,
                contract,
                method,
                mutation_arg_debug_name(method, &parameter.name, index),
                checked_u32(index, origin, "typed container mutation parameter")?,
            );
        }
        Ok(())
    }

    fn dynamic_call(
        &self,
        body: &HirBody,
        call: &vela_hir::body::HirCall,
        placement: &vela_analysis::validation::CallArgumentPlacementFact,
        origin: MirSourceOrigin,
    ) -> CompileResult<CompileCallTarget> {
        let arguments = self.dynamic_argument_values(placement, origin)?;
        if let Some(field) = body.field(call.callee) {
            return Ok(CompileCallTarget::dynamic(
                CompileCalleeTarget::DynamicMethod(DynamicMethodTarget::method(
                    &field.name,
                    checked_u32(
                        placement
                            .source_order
                            .iter()
                            .filter(|argument| argument.name.is_none())
                            .count(),
                        origin,
                        "dynamic positional arity",
                    )?,
                    placement
                        .source_order
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

    fn host_access_call(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
        expression: HirExprId,
        call_target: &CallTargetFact,
        placement: &vela_analysis::validation::CallArgumentPlacementFact,
        origin: MirSourceOrigin,
    ) -> CompileResult<Option<CompileCallTarget>> {
        let Some(fact) = self
            .executable_analysis(executable)?
            .host_access_use(expression)
            .cloned()
        else {
            if matches!(call_target, CallTargetFact::HostMethod { .. }) {
                return Err(input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: "host method target has no HostAccess use fact".to_owned(),
                }));
            }
            if let Some(field) = body.field(
                body.call(expression)
                    .ok_or_else(registry_input_error)?
                    .callee,
            ) && matches!(field.name.as_str(), "remove" | "push")
                && let Some(path) =
                    self.derived_host_index_path(executable, body, field.receiver)?
            {
                let arguments = body
                    .call(expression)
                    .ok_or_else(registry_input_error)?
                    .arguments
                    .iter()
                    .map(|argument| argument.value.ok_or_else(registry_input_error))
                    .collect::<CompileResult<Vec<_>>>()?;
                return match field.name.as_str() {
                    "remove" if arguments.is_empty() => Ok(Some(CompileCallTarget::positional(
                        CompileCalleeTarget::HostRemove { path },
                        arguments,
                    ))),
                    "push" if arguments.len() == 1 => Ok(Some(CompileCallTarget::positional(
                        CompileCalleeTarget::HostPush { path },
                        arguments,
                    ))),
                    _ => Ok(None),
                };
            }
            return Ok(None);
        };
        self.validate_host_access_call(executable, body, expression, &fact, origin)?;
        let path_fact = self
            .executable_analysis(executable)?
            .host_path_target(fact.target)
            .cloned()
            .ok_or_else(|| {
                input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: "HostAccess call target has no host-path target".to_owned(),
                })
            })?;
        let path = self.convert_host_path(executable, path_fact)?;
        let target = match fact.kind {
            HostAccessUseKind::Remove => {
                let arguments = self.source_argument_values(placement, origin)?;
                if !arguments.is_empty() {
                    return Err(input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: "host remove placement has unexpected arguments".to_owned(),
                    }));
                }
                CompileCallTarget::positional(CompileCalleeTarget::HostRemove { path }, arguments)
            }
            HostAccessUseKind::Push => {
                let arguments = self.source_argument_values(placement, origin)?;
                if arguments.len() != 1 {
                    return Err(input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: "host push placement must contain one argument".to_owned(),
                    }));
                }
                CompileCallTarget::positional(CompileCalleeTarget::HostPush { path }, arguments)
            }
            HostAccessUseKind::Call => {
                let CallTargetFact::HostMethod { owner, name } = call_target else {
                    return Err(input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: "HostAccess call fact disagrees with its host method target"
                            .to_owned(),
                    }));
                };
                self.host_method_call(executable, owner, name, placement, origin)?
            }
            HostAccessUseKind::Read | HostAccessUseKind::Write | HostAccessUseKind::Mutate => {
                return Err(input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: "non-call HostAccess use fact is attached to a call expression"
                        .to_owned(),
                }));
            }
        };
        Ok(Some(target))
    }

    fn validate_host_access_call(
        &self,
        executable: FunctionId,
        body: &HirBody,
        expression: HirExprId,
        fact: &HostAccessUseFact,
        origin: MirSourceOrigin,
    ) -> CompileResult<()> {
        let call = body.call(expression).ok_or_else(registry_input_error)?;
        let field = body.field(call.callee).ok_or_else(|| {
            input_error(MirBuildError::InconsistentInput {
                origin,
                message: "HostAccess call has no field callee".to_owned(),
            })
        })?;
        if field.receiver != fact.target {
            return Err(input_error(MirBuildError::InconsistentInput {
                origin,
                message: "HostAccess target disagrees with the call receiver".to_owned(),
            }));
        }
        let analysis = self.executable_analysis(executable)?;
        let path = analysis.host_path_target(fact.target).ok_or_else(|| {
            input_error(MirBuildError::InconsistentInput {
                origin,
                message: "HostAccess call target has no host-path fact".to_owned(),
            })
        })?;
        let path_indexes = path
            .segments
            .iter()
            .filter_map(|segment| match segment {
                HostPathSegmentFact::Index {
                    expression,
                    owner,
                    capability,
                    ..
                } => Some((*expression, owner, capability)),
                HostPathSegmentFact::Field(_) => None,
            })
            .collect::<Vec<_>>();
        if path_indexes.len() != fact.indexes.len() {
            return Err(input_error(MirBuildError::InconsistentInput {
                origin,
                message: "HostAccess indexes disagree with the host path".to_owned(),
            }));
        }
        for (index_use, (key, owner, capability)) in fact.indexes.iter().zip(path_indexes) {
            let hir_index = body.index(index_use.expression).ok_or_else(|| {
                input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: "HostAccess index use does not identify an index expression"
                        .to_owned(),
                })
            })?;
            let HostIndexCapabilityResolutionFact::Registered(index_capability) =
                &index_use.capability
            else {
                return Err(input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: "HostAccess index has no registered compile capability".to_owned(),
                }));
            };
            if hir_index.receiver != index_use.receiver
                || hir_index.index != index_use.key
                || index_use.key != key
                || &index_use.owner != owner
                || index_capability != capability
            {
                return Err(input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: "HostAccess index metadata disagrees with the host path".to_owned(),
                }));
            }
        }
        let expected_accessed_index = (fact.kind != HostAccessUseKind::Call)
            .then(|| body.index(fact.target))
            .flatten()
            .and_then(|target| {
                fact.indexes
                    .iter()
                    .position(|index| index.expression == target.expression)
            });
        if fact.accessed_index != expected_accessed_index
            || fact
                .accessed_index
                .is_some_and(|index| index >= fact.indexes.len())
        {
            return Err(input_error(MirBuildError::InconsistentInput {
                origin,
                message: "HostAccess accessed index disagrees with its target".to_owned(),
            }));
        }
        Ok(())
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
            MemberTargetFact::HostProperty { owner, name } => {
                let field = self
                    .registry_facts
                    .host_field_target_fact(&owner, &name)
                    .cloned()
                    .ok_or_else(registry_input_error)?;
                self.ensure_external_field(field.semantic, origin)?;
                let placement = vela_analysis::validation::CallArgumentPlacementFact {
                    mode: CallPlacementModeFact::ExternalPositional,
                    source_order: Vec::new(),
                    parameter_slots: Some(Vec::new()),
                };
                let target =
                    self.host_method_call(executable, &owner, &name, &placement, origin)?;
                let CompileCalleeTarget::HostMethod(method) = target.callee else {
                    return Err(input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: "host property did not resolve to a host method".to_owned(),
                    }));
                };
                CompileMemberTarget::HostProperty(method)
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
                self.closed_dynamic_script_field(executable, body, expression, &name)?
                    .unwrap_or(CompileMemberTarget::Dynamic { name })
            }
            MemberTargetFact::Unresolved => {
                let name = body
                    .field(expression)
                    .map(|field| field.name.clone())
                    .ok_or_else(registry_input_error)?;
                self.closed_dynamic_script_field(executable, body, expression, &name)?
                    .unwrap_or(CompileMemberTarget::Dynamic { name })
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

    /// Close a generic-analysis member when its receiver still carries an
    /// authoritative script-type identity. The physical backend must never
    /// recover field slots from HIR.
    fn closed_dynamic_script_field(
        &self,
        executable: FunctionId,
        body: &HirBody,
        expression: HirExprId,
        name: &str,
    ) -> CompileResult<Option<CompileMemberTarget>> {
        let receiver = body
            .field(expression)
            .map(|field| field.receiver)
            .ok_or_else(registry_input_error)?;
        let analysis = self.executable_analysis(executable)?;
        if let Some(script) = analysis.script_type(receiver) {
            let key = (script.declaration, script.variant.clone(), name.to_owned());
            if !self.field_ids.contains_key(&key) {
                return Ok(None);
            }
            return self
                .script_field_target(script.declaration, script.variant.as_deref(), name)
                .map(Some);
        }

        let (owner_name, variant) = match analysis.expression(receiver) {
            Some(TypeFact::Record { name }) => (name.as_str(), None),
            Some(TypeFact::Enum { name, variant }) => (name.as_str(), variant.as_deref()),
            _ => return Ok(None),
        };
        let declaration = self
            .request
            .type_symbols
            .iter()
            .find_map(|(declaration, symbol)| {
                (symbol == owner_name || symbol.ends_with(&format!("::{owner_name}")))
                    .then_some(*declaration)
            });
        declaration
            .filter(|declaration| {
                self.field_ids.contains_key(&(
                    *declaration,
                    variant.map(str::to_owned),
                    name.to_owned(),
                ))
            })
            .map(|declaration| self.script_field_target(declaration, variant, name))
            .transpose()
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
}

fn borrowed_collection_source(actual: &TypeFact, expected: &TypeFact) -> bool {
    let borrowed = matches!(
        actual,
        TypeFact::ArrayView { .. }
            | TypeFact::ArrayMut { .. }
            | TypeFact::MapView { .. }
            | TypeFact::MapMut { .. }
            | TypeFact::SetView { .. }
            | TypeFact::SetMut { .. }
    );
    borrowed
        && matches!(
            (actual, expected),
            (
                TypeFact::ArrayView { .. } | TypeFact::ArrayMut { .. },
                TypeFact::Array { .. }
            ) | (
                TypeFact::MapView { .. } | TypeFact::MapMut { .. },
                TypeFact::Map { .. }
            ) | (
                TypeFact::SetView { .. } | TypeFact::SetMut { .. },
                TypeFact::Set { .. }
            )
        )
}

fn task_outcome_contract_name(contract: &vela_mir::MirTypeContract) -> String {
    let vela_mir::MirTypeContract::Result { ok, .. } = contract else {
        unreachable!("task outcome contract is always Result")
    };
    let ok = ok
        .as_deref()
        .map_or_else(|| "Any".to_owned(), task_contract_name);
    format!("Result<{ok}, task::Error>")
}

fn task_contract_name(contract: &vela_mir::MirTypeContract) -> String {
    match contract {
        vela_mir::MirTypeContract::Any => "Any".to_owned(),
        vela_mir::MirTypeContract::TaskError => "task::Error".to_owned(),
        vela_mir::MirTypeContract::Primitive(tag) => tag.name().to_owned(),
        vela_mir::MirTypeContract::Definition(type_id)
        | vela_mir::MirTypeContract::Shape { type_id, .. }
        | vela_mir::MirTypeContract::Variant { type_id, .. } => format!("type#{}", type_id.get()),
        vela_mir::MirTypeContract::Host(target) => format!("host#{}", target.semantic.get()),
        vela_mir::MirTypeContract::Range => "Range".to_owned(),
        vela_mir::MirTypeContract::Array(_) => "Array".to_owned(),
        vela_mir::MirTypeContract::Map { .. } => "Map".to_owned(),
        vela_mir::MirTypeContract::Set(_) => "Set".to_owned(),
        vela_mir::MirTypeContract::Iterator(_) => "Iterator".to_owned(),
        vela_mir::MirTypeContract::Tuple(_) => "Tuple".to_owned(),
        vela_mir::MirTypeContract::Option(_) => "Option".to_owned(),
        vela_mir::MirTypeContract::Result { .. } => "Result".to_owned(),
        vela_mir::MirTypeContract::Callable { .. } => "Callable".to_owned(),
    }
}

fn external_parameter_expression(
    arguments: &CompileCallArguments,
    parameter: usize,
) -> Option<HirExprId> {
    match arguments {
        CompileCallArguments::Positional(values) => values.get(parameter).copied(),
        CompileCallArguments::ExternalNamed {
            parameter_slots, ..
        }
        | CompileCallArguments::Script {
            parameter_slots, ..
        } => parameter_slots
            .get(parameter)
            .and_then(|slot| match slot.value {
                CompilePlacedCallValue::Explicit { value, .. } => Some(value),
                CompilePlacedCallValue::MissingDefault => None,
            }),
        CompileCallArguments::Dynamic(_) => None,
    }
}

pub(super) fn runtime_semantic_body(body: &HirBody) -> bool {
    matches!(
        body.owner,
        HirBodyOwner::Declaration(_)
            | HirBodyOwner::StateInitializer(_)
            | HirBodyOwner::TraitDefaultMethod(_)
            | HirBodyOwner::ImplMethod(_)
            | HirBodyOwner::Lambda { .. }
            | HirBodyOwner::ParameterDefault { .. }
    )
}

mod call_arguments;
mod constructor_arguments;
mod constructors;
mod helpers;
mod host_paths;
mod service_calls;
use self::constructor_arguments::{
    require_constructor_slot_identity, unavailable_constructor_default,
};
use self::helpers::{
    ConstantHostIndex, ConstructorFieldSpec, ConstructorSpec, callee_path, checked_u32,
    constructor_variant_specs, field_is_call_callee, pattern_field_names, reflection_operation,
    require_analysis_call_target, type_owner_name,
};
