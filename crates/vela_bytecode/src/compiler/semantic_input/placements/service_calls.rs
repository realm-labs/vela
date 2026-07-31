use vela_common::ServiceCallMode;
use vela_def::FunctionId;
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
                let current = self.current_service_path(executable).ok_or_else(|| {
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
            effect: method.effect,
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

    fn current_service_path(&self, executable: FunctionId) -> Option<String> {
        self.request
            .service_impls
            .implementations()
            .find_map(|implementation| {
                implementation
                    .methods()
                    .any(|method| {
                        self.targets.service_function_for_node(method.node()) == Some(executable)
                    })
                    .then(|| implementation.service_path_text())
            })
    }
}

fn service_call_error(message: impl Into<String>, origin: MirSourceOrigin) -> CompileError {
    CompileError::new(CompileErrorKind::ServiceCall(message.into())).with_span(origin.span)
}
