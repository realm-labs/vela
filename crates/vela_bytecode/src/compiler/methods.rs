use vela_common::HostMethodId;
use vela_syntax::ast::{Expr, ExprKind};

use super::body_payloads::CompilerExpressionPayload;
use super::host_paths::{HostPath, HostPathPart, HostPathRoot, ResolvedHostPath};

pub(super) struct HostMethodCall<'ast> {
    pub(super) receiver: HostPathRoot<'ast>,
    pub(super) segments: Vec<HostPathPart<'ast>>,
    pub(super) method: HostMethodId,
}

pub(super) fn host_method_call<'ast>(
    compiler: &super::Compiler<'_, '_>,
    callee: &'ast Expr,
    callee_payload: Option<&CompilerExpressionPayload<'ast>>,
    receiver_type: Option<&str>,
    path_root_is_local: bool,
) -> Option<HostMethodCall<'ast>> {
    match &callee.kind {
        ExprKind::Field { base, name } => {
            let receiver_payload = callee_payload.and_then(|payload| payload.field_base_payload());
            let receiver = host_method_receiver_path(compiler, base, receiver_payload.as_ref())?;
            let name = callee_payload
                .and_then(CompilerExpressionPayload::syntax_field_name)
                .unwrap_or_else(|| name.to_owned());
            let method =
                compiler.host_method_id(receiver_type.or(receiver.type_name.as_deref()), &name)?;
            Some(HostMethodCall {
                receiver: receiver.path.root,
                segments: receiver.path.segments,
                method,
            })
        }
        ExprKind::Path(path) => {
            let cst_path = callee_payload.and_then(CompilerExpressionPayload::syntax_path_segments);
            let lookup_path = match (cst_path.as_deref(), callee_payload.is_some()) {
                (Some(path), _) => path,
                (None, true) => return None,
                (None, false) => path,
            };
            if lookup_path.len() < 2 {
                return None;
            }
            if compiler.is_native_module_root(&lookup_path[0]) && !path_root_is_local {
                return None;
            }
            let method_name = lookup_path.last()?;
            let receiver = host_method_path_receiver(compiler, callee, &path[..path.len() - 1])?;
            let method = compiler
                .host_method_id(receiver_type.or(receiver.type_name.as_deref()), method_name)?;
            Some(HostMethodCall {
                receiver: receiver.path.root,
                segments: receiver.path.segments,
                method,
            })
        }
        _ => None,
    }
}

fn host_method_receiver_path<'ast>(
    compiler: &super::Compiler<'_, '_>,
    receiver: &'ast Expr,
    receiver_payload: Option<&CompilerExpressionPayload<'ast>>,
) -> Option<ResolvedHostPath<'ast>> {
    compiler
        .resolve_host_path_with_payload(receiver, receiver_payload)
        .or_else(|| {
            Some(ResolvedHostPath {
                path: HostPath {
                    root: HostPathRoot::Expr {
                        expr: receiver,
                        payload: receiver_payload.cloned(),
                    },
                    segments: Vec::new(),
                },
                type_name: compiler.script_type_for_expr_with_payload(receiver, receiver_payload),
            })
        })
}

fn host_method_path_receiver<'ast>(
    compiler: &super::Compiler<'_, '_>,
    callee: &'ast Expr,
    path: &'ast [String],
) -> Option<ResolvedHostPath<'ast>> {
    let root = path.first()?;
    if path.len() == 1 {
        Some(ResolvedHostPath {
            path: HostPath {
                root: HostPathRoot::LocalPath {
                    name: root,
                    span: callee.span,
                },
                segments: Vec::new(),
            },
            type_name: compiler.host_local_type_name(root, callee.span),
        })
    } else {
        compiler.host_field_path_parts(callee.span, path)
    }
}
