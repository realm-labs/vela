use vela_common::{FieldId, HostMethodId};
use vela_syntax::{Expr, ExprKind};

use super::CompilerOptions;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HostMethodCall<'ast> {
    pub(super) receiver: HostMethodReceiver<'ast>,
    pub(super) fields: Vec<FieldId>,
    pub(super) method: HostMethodId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HostMethodReceiver<'ast> {
    Expr(&'ast Expr),
    LocalPath(&'ast str),
}

pub(super) fn host_method_call<'ast>(
    options: &CompilerOptions,
    callee: &'ast Expr,
    receiver_type: Option<&str>,
) -> Option<HostMethodCall<'ast>> {
    match &callee.kind {
        ExprKind::Field { base, name } => {
            let method = options.host_method(receiver_type, name)?;
            let (receiver, fields) = host_method_receiver_path(options, base)?;
            Some(HostMethodCall {
                receiver,
                fields,
                method,
            })
        }
        ExprKind::Path(path) => {
            if path.len() < 2 {
                return None;
            }
            let method_name = path.last()?;
            let method = options.host_method(receiver_type, method_name)?;
            let (receiver, fields) = host_method_path_receiver(options, &path[..path.len() - 1])?;
            Some(HostMethodCall {
                receiver,
                fields,
                method,
            })
        }
        _ => None,
    }
}

fn host_method_receiver_path<'ast>(
    options: &CompilerOptions,
    receiver: &'ast Expr,
) -> Option<(HostMethodReceiver<'ast>, Vec<FieldId>)> {
    match &receiver.kind {
        ExprKind::Field { base, name } => {
            let field = options.host_fields.get(name).copied()?;
            let (receiver, mut fields) = host_method_receiver_path(options, base)?;
            fields.push(field);
            Some((receiver, fields))
        }
        ExprKind::Path(path) => host_method_path_receiver(options, path),
        _ => Some((HostMethodReceiver::Expr(receiver), Vec::new())),
    }
}

fn host_method_path_receiver<'ast>(
    options: &CompilerOptions,
    path: &'ast [String],
) -> Option<(HostMethodReceiver<'ast>, Vec<FieldId>)> {
    let root = path.first()?;
    let fields = path[1..]
        .iter()
        .map(|segment| options.host_fields.get(segment).copied())
        .collect::<Option<Vec<_>>>()?;
    Some((HostMethodReceiver::LocalPath(root), fields))
}
