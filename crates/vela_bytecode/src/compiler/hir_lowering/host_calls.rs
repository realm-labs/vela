use super::*;
use vela_mir::{CompileCallArguments, CompileCalleeTarget};

impl Compiler<'_, '_> {
    pub(super) fn try_compile_placed_hir_host_call(
        &mut self,
        span: Span,
        call: &vela_hir::body::HirCall,
    ) -> CompileResult<Option<Register>> {
        let Some(field) = self.hir_field_for_expression(call.callee).cloned() else {
            return Ok(None);
        };
        let Some(resolved) = self.hir_host_path(field.receiver) else {
            return Ok(None);
        };
        let placed = self.placed_call_target(call.expression)?;
        match placed.callee {
            CompileCalleeTarget::HostRemove { path } => {
                if field.name != "remove"
                    || !matches!(
                        self.hir_expression_record(field.receiver)?.1,
                        HirExprKind::Index(_)
                    )
                {
                    return Err(self.compile_target_input_error(
                        call.expression,
                        "host remove target disagrees with the HIR callee",
                    ));
                }
                let CompileCallArguments::Positional(values) = placed.arguments else {
                    return Err(self.compile_target_input_error(
                        call.expression,
                        "host remove owns non-positional arguments",
                    ));
                };
                if !values.is_empty() {
                    return Err(self.compile_target_input_error(
                        call.expression,
                        "host remove placement has unexpected arguments",
                    ));
                }
                self.require_direct_host_path_target(
                    call.expression,
                    field.receiver,
                    &resolved.path,
                    &path,
                )?;
                self.reject_invalid_hir_host_index_access(
                    field.receiver,
                    HostIndexAccessKind::Remove,
                    span,
                )?;
                let root = self.compile_host_path_root(&resolved.path.root)?;
                self.emit_host_remove(root, resolved.path, span)?;
                self.emit_constant(Constant::Unit).map(Some)
            }
            CompileCalleeTarget::HostPush { path } => {
                if field.name != "push" || resolved.path.segments.is_empty() {
                    return Err(self.compile_target_input_error(
                        call.expression,
                        "host push target disagrees with the HIR callee",
                    ));
                }
                let CompileCallArguments::Positional(values) = placed.arguments else {
                    return Err(self.compile_target_input_error(
                        call.expression,
                        "host push owns non-positional arguments",
                    ));
                };
                let [value] = values.as_slice() else {
                    return Err(self.compile_target_input_error(
                        call.expression,
                        "host push placement must contain one argument",
                    ));
                };
                self.require_direct_host_path_target(
                    call.expression,
                    field.receiver,
                    &resolved.path,
                    &path,
                )?;
                self.reject_invalid_hir_host_assignment(field.receiver, HirAssignOp::Set, span)?;
                let value = self.compile_hir_expression(*value)?;
                let root = self.compile_host_path_root(&resolved.path.root)?;
                self.emit_host_mutate(root, resolved.path, HostMutationOp::Push, value, span)?;
                self.emit_constant(Constant::Unit).map(Some)
            }
            CompileCalleeTarget::HostMethod(_) => {
                let path = self.placed_host_path_target(field.receiver)?;
                self.require_direct_host_path_target(
                    call.expression,
                    field.receiver,
                    &resolved.path,
                    &path,
                )?;
                let legacy_method = self.host_method_id(resolved.type_name.as_deref(), &field.name);
                let legacy_owner = resolved
                    .type_name
                    .as_deref()
                    .and_then(|name| self.host_type_id_for_name(name));
                let (method_id, args) = self.compile_hir_host_method_arguments(
                    call.expression,
                    &field.name,
                    legacy_owner,
                    legacy_method,
                    span,
                )?;
                let root = self.compile_host_path_root(&resolved.path.root)?;
                let dst = self.alloc_register()?;
                self.emit_host_call(Some(dst), root, resolved.path, method_id, args, span)?;
                Ok(Some(dst))
            }
            CompileCalleeTarget::ScriptFunction { .. }
            | CompileCalleeTarget::ScriptMethod { .. }
            | CompileCalleeTarget::Local(_)
            | CompileCalleeTarget::Lambda(_)
            | CompileCalleeTarget::NativeFunction { .. }
            | CompileCalleeTarget::StdlibFunction { .. }
            | CompileCalleeTarget::Reflection { .. }
            | CompileCalleeTarget::SetFromArray { .. }
            | CompileCalleeTarget::ValueMethod { .. }
            | CompileCalleeTarget::DynamicCallable
            | CompileCalleeTarget::DynamicMethod(_) => Ok(None),
        }
    }
}
