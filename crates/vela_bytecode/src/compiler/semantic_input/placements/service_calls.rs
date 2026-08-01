use std::collections::BTreeSet;
use vela_common::ServiceCallMode;
use vela_def::FunctionId;

use vela_analysis::semantic_facts::CallTargetFact;
use vela_hir::body::{HirBody, HirCall, HirExprKind};
use vela_mir::{CompileCallTarget, CompileCalleeTarget, MirSourceOrigin};

use super::GenerationBuilder;
use crate::compiler::error::{CompileError, CompileErrorKind, CompileResult};
use crate::compiler::semantic_input::registry_input_error;

impl GenerationBuilder<'_, '_> {
    pub(super) fn service_call_target(
        &self,
        executable: FunctionId,
        body: &HirBody,
        call: &HirCall,
        origin: MirSourceOrigin,
    ) -> CompileResult<Option<CompileCallTarget>> {
        let bindings = self
            .request
            .graph
            .bindings_for_body(body.id)
            .ok_or_else(registry_input_error)?;
        let Some(capability) = bindings.service_capability(call.callee) else {
            return Ok(None);
        };
        let Some(expression) = body.expression(call.callee) else {
            return Err(service_call_error(
                "service call has no callee expression",
                origin,
            ));
        };
        let HirExprKind::Path(path_id) = expression.kind else {
            return Err(service_call_error(
                "service call target is not a path",
                origin,
            ));
        };
        let path = body
            .paths
            .get(&path_id)
            .ok_or_else(|| service_call_error("service call path is missing", origin))?;
        let (mode, service_name, method_name) = match capability {
            vela_hir::binding::ServiceLexicalCapability::Base => {
                let current = self
                    .current_service_path(executable, origin)?
                    .ok_or_else(|| {
                        service_call_error(
                            "base call is not owned by a compiled service method",
                            origin,
                        )
                    })?;
                (ServiceCallMode::Base, current, path.path[2].clone())
            }
            vela_hir::binding::ServiceLexicalCapability::Pinned => (
                ServiceCallMode::Pinned,
                path.path[2].clone(),
                path.path[3].clone(),
            ),
        };
        let schema = self.request.service_schema.ok_or_else(|| {
            service_call_error(
                "service calls require the generated service-set schema",
                origin,
            )
        })?;
        let service = match mode {
            ServiceCallMode::Base => schema.service_by_path(&service_name),
            ServiceCallMode::Pinned => schema.service_by_member(&service_name),
        }
        .ok_or_else(|| {
            service_call_error(format!("unknown service target `{service_name}`"), origin)
        })?;
        let method = service.method(&method_name).ok_or_else(|| {
            service_call_error(
                format!("unknown service method `{}::{method_name}`", service.path),
                origin,
            )
        })?;
        if call
            .arguments
            .iter()
            .any(|argument| argument.name.is_some())
        {
            return Err(service_call_error(
                "service calls accept positional arguments only",
                origin,
            ));
        }
        let arguments = call
            .arguments
            .iter()
            .map(|argument| {
                argument.value.ok_or_else(|| {
                    service_call_error("service call argument is missing a value", origin)
                })
            })
            .collect::<CompileResult<Vec<_>>>()?;
        let parameter_count = usize::try_from(method.parameter_count)
            .expect("service method parameter count fits usize");
        if arguments.len() != parameter_count {
            return Err(service_call_error(
                format!(
                    "service method `{}::{}` expects {} arguments, found {}",
                    service.path,
                    method.name,
                    method.parameter_count,
                    arguments.len()
                ),
                origin,
            ));
        }
        let signature = vela_mir::CompileSignature {
            asyncness: method.asyncness,
            parameters: (0..method.parameter_count)
                .map(|index| vela_mir::CompileParameter {
                    name: format!("arg{index}"),
                    contract: None,
                    default: vela_mir::CompileParameterDefault::Required,
                    origin: None,
                })
                .collect(),
            positional: vela_mir::CompilePositionalPolicy::ExactOrTrailingDefaults,
            return_contract: None,
            effect: match mode {
                ServiceCallMode::Base => method.rust_default_effect,
                ServiceCallMode::Pinned => method.patch_effect_ceiling,
            },
        };
        Ok(Some(CompileCallTarget::positional(
            CompileCalleeTarget::Service {
                mode,
                service: service.id,
                method: method.id,
                debug_name: format!(
                    "__vela_service.{}.{}.{}",
                    mode.abi_name(),
                    service.path.replace("::", "."),
                    method.name
                ),
                signature,
            },
            arguments,
        )))
    }

    fn current_service_path(
        &self,
        executable: FunctionId,
        origin: MirSourceOrigin,
    ) -> CompileResult<Option<String>> {
        let mut origins = BTreeSet::new();
        for implementation in self.request.service_impls.implementations() {
            for method in implementation.methods() {
                let Some(root) = self.targets.service_function_for_node(method.node()) else {
                    continue;
                };
                if self.script_reaches(root, executable)? {
                    origins.insert(implementation.service_path_text());
                }
            }
        }
        if origins.len() > 1 {
            return Err(service_call_error(
                format!(
                    "base call is reachable from multiple Service origins: {}",
                    origins.into_iter().collect::<Vec<_>>().join(", ")
                ),
                origin,
            ));
        }
        Ok(origins.into_iter().next())
    }

    fn script_reaches(&self, start: FunctionId, target: FunctionId) -> CompileResult<bool> {
        let mut pending = vec![start];
        let mut visited = BTreeSet::new();
        while let Some(function) = pending.pop() {
            if function == target {
                return Ok(true);
            }
            if !visited.insert(function) {
                continue;
            }
            let Some((_, root)) = self
                .selected_executable_roots()?
                .into_iter()
                .find(|(candidate, _)| *candidate == function)
            else {
                continue;
            };
            let analysis = self.executable_analysis(function)?;
            for body_id in self.executable_body_ids(root) {
                let body = self
                    .request
                    .graph
                    .body(body_id)
                    .ok_or_else(registry_input_error)?;
                for expression in body.expressions.values() {
                    let Some(CallTargetFact::Declaration(declaration)) =
                        analysis.call_target(expression.id)
                    else {
                        continue;
                    };
                    if let Some(callee) = self.function_ids.get(declaration) {
                        pending.push(*callee);
                    }
                }
            }
        }
        Ok(false)
    }
}

fn service_call_error(message: impl Into<String>, origin: MirSourceOrigin) -> CompileError {
    CompileError::new(CompileErrorKind::ServiceCall(message.into())).with_span(origin.span)
}
