use vela_common::HostMethodId;
use vela_syntax::{Expr, ExprKind};

use super::{
    CompilerOptions,
    host_paths::{HostPath, HostPathPart, HostPathRoot, host_field_path, host_field_path_parts},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HostMethodCall<'ast> {
    pub(super) receiver: HostPathRoot<'ast>,
    pub(super) segments: Vec<HostPathPart<'ast>>,
    pub(super) method: HostMethodId,
}

pub(super) fn host_method_call<'ast>(
    options: &CompilerOptions,
    callee: &'ast Expr,
    receiver_type: Option<&str>,
) -> Option<HostMethodCall<'ast>> {
    match &callee.kind {
        ExprKind::Field { base, name } => {
            let method = options.host_method(receiver_type, name)?;
            let path = host_method_receiver_path(options, base)?;
            Some(HostMethodCall {
                receiver: path.root,
                segments: path.segments,
                method,
            })
        }
        ExprKind::Path(path) => {
            if path.len() < 2 {
                return None;
            }
            if options.is_native_module_root(&path[0]) {
                return None;
            }
            let method_name = path.last()?;
            let method = options.host_method(receiver_type, method_name)?;
            let path = host_method_path_receiver(options, &path[..path.len() - 1])?;
            Some(HostMethodCall {
                receiver: path.root,
                segments: path.segments,
                method,
            })
        }
        _ => None,
    }
}

fn host_method_receiver_path<'ast>(
    options: &CompilerOptions,
    receiver: &'ast Expr,
) -> Option<HostPath<'ast>> {
    host_field_path(options, receiver).or(Some(HostPath {
        root: HostPathRoot::Expr(receiver),
        segments: Vec::new(),
    }))
}

fn host_method_path_receiver<'ast>(
    options: &CompilerOptions,
    path: &'ast [String],
) -> Option<HostPath<'ast>> {
    let root = path.first()?;
    if path.len() == 1 {
        Some(HostPath {
            root: HostPathRoot::LocalPath(root),
            segments: Vec::new(),
        })
    } else {
        host_field_path_parts(options, path)
    }
}
