use vela_common::HostMethodId;
use vela_syntax::{Expr, ExprKind};

use super::CompilerOptions;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HostMethodCall<'ast> {
    pub(super) receiver: HostMethodReceiver<'ast>,
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
            Some(HostMethodCall {
                receiver: HostMethodReceiver::Expr(base),
                method,
            })
        }
        ExprKind::Path(path) => {
            let [receiver, method_name] = path.as_slice() else {
                return None;
            };
            let method = options.host_method(receiver_type, method_name)?;
            Some(HostMethodCall {
                receiver: HostMethodReceiver::LocalPath(receiver),
                method,
            })
        }
        _ => None,
    }
}
