use std::collections::HashMap;

use vela_syntax::ast::{Expr, ExprKind, SyntaxExpressionKind};

use crate::Register;
use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::{CompileError, CompileErrorKind, CompileResult};

pub(super) fn local_path_method_call<'expr>(
    cst_path: Option<&'expr [String]>,
    has_payload: bool,
    callee: &'expr Expr,
    locals: &HashMap<String, Register>,
) -> Option<(&'expr str, &'expr [String])> {
    let path = callee_path_segments(cst_path, has_payload, callee)?;
    let (method, receiver_path) = path.split_last()?;
    (!receiver_path.is_empty() && locals.contains_key(&receiver_path[0]))
        .then_some((method.as_str(), receiver_path))
}

pub(super) fn path_root_is_local(
    cst_path: Option<&[String]>,
    has_payload: bool,
    callee: &Expr,
    locals: &HashMap<String, Register>,
) -> bool {
    let Some(path) = callee_path_segments(cst_path, has_payload, callee) else {
        return false;
    };
    path.first().is_some_and(|root| locals.contains_key(root))
}

pub(super) fn callable_name(
    cst_path: Option<&[String]>,
    has_payload: bool,
    callee: &Expr,
) -> CompileResult<String> {
    let Some(path) = callee_path_segments(cst_path, has_payload, callee) else {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "callable expression",
        )));
    };
    Ok(path.join("::"))
}

pub(super) fn callee_is_closure_call(
    callee_payload: Option<&CompilerExpressionPayload<'_>>,
    callee: &Expr,
) -> bool {
    if let Some(payload) = callee_payload {
        return !matches!(payload.syntax_kind(), Some(SyntaxExpressionKind::Path));
    }
    !matches!(callee.kind, ExprKind::Path(_))
}

pub(super) fn callee_path_segments<'expr>(
    cst_path: Option<&'expr [String]>,
    has_payload: bool,
    callee: &'expr Expr,
) -> Option<&'expr [String]> {
    if let Some(path) = cst_path {
        return Some(path);
    }
    if has_payload {
        return None;
    }
    match &callee.kind {
        ExprKind::Path(path) => Some(path),
        _ => None,
    }
}
