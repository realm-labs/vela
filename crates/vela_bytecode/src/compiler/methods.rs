use vela_common::HostMethodId;
use vela_syntax::ast::{Expr, ExprKind};

use super::host_paths::{HostPath, HostPathPart, HostPathRoot, ResolvedHostPath};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HostMethodCall<'ast> {
    pub(super) receiver: HostPathRoot<'ast>,
    pub(super) segments: Vec<HostPathPart<'ast>>,
    pub(super) method: HostMethodId,
}

pub(super) fn host_method_call<'ast>(
    compiler: &super::Compiler<'_, '_>,
    callee: &'ast Expr,
    receiver_type: Option<&str>,
    path_root_is_local: bool,
) -> Option<HostMethodCall<'ast>> {
    match &callee.kind {
        ExprKind::Field { base, name } => {
            let receiver = host_method_receiver_path(compiler, base)?;
            let method =
                compiler.host_method_id(receiver_type.or(receiver.type_name.as_deref()), name)?;
            Some(HostMethodCall {
                receiver: receiver.path.root,
                segments: receiver.path.segments,
                method,
            })
        }
        ExprKind::Path(path) => {
            if path.len() < 2 {
                return None;
            }
            if compiler.is_native_module_root(&path[0]) && !path_root_is_local {
                return None;
            }
            let method_name = path.last()?;
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
) -> Option<ResolvedHostPath<'ast>> {
    compiler.resolve_host_path(receiver).or_else(|| {
        Some(ResolvedHostPath {
            path: HostPath {
                root: HostPathRoot::Expr(receiver),
                segments: Vec::new(),
            },
            type_name: compiler.script_type_for_expr(receiver),
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
