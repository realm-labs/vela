use vela_common::HostMethodId;
use vela_syntax::ast::{Expr, ExprKind, SyntaxExpressionKind};

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
    let callee_payload = callee_payload?;
    match &callee.kind {
        ExprKind::Field { base, .. } => {
            let receiver_payload = callee_payload.field_base_payload();
            let receiver = host_method_receiver_path(compiler, base, receiver_payload.as_ref())?;
            let name = callee_field_name(callee_payload)?;
            let method =
                compiler.host_method_id(receiver_type.or(receiver.type_name.as_deref()), &name)?;
            Some(HostMethodCall {
                receiver: receiver.path.root,
                segments: receiver.path.segments,
                method,
            })
        }
        ExprKind::Path(_) => {
            let cst_path = callee_payload.syntax_path_segments()?;
            let lookup_path = cst_path.as_slice();
            if lookup_path.len() < 2 {
                return None;
            }
            if compiler.is_native_module_root(&lookup_path[0]) && !path_root_is_local {
                return None;
            }
            let method_name = lookup_path.last()?;
            let receiver =
                host_method_path_receiver(compiler, callee, &lookup_path[..lookup_path.len() - 1])?;
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

fn callee_field_name(callee_payload: &CompilerExpressionPayload<'_>) -> Option<String> {
    match callee_payload.syntax_kind() {
        Some(SyntaxExpressionKind::Field) | None => callee_payload.syntax_field_name(),
        Some(SyntaxExpressionKind::Path) => None,
        Some(_) => None,
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
                type_name: compiler.script_type_for_expression_payload(receiver_payload),
            })
        })
}

fn host_method_path_receiver<'ast>(
    compiler: &super::Compiler<'_, '_>,
    callee: &'ast Expr,
    path: &[String],
) -> Option<ResolvedHostPath<'ast>> {
    let root = path.first()?;
    if path.len() == 1 {
        Some(ResolvedHostPath {
            path: HostPath {
                root: HostPathRoot::OwnedLocalPath {
                    name: root.clone(),
                    span: callee.span,
                },
                segments: Vec::new(),
            },
            type_name: compiler.host_local_type_name(root, callee.span),
        })
    } else {
        compiler.owned_host_field_path_parts(callee.span, path)
    }
}
