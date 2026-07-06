use std::collections::HashMap;

use vela_syntax::ast::SyntaxExpressionKind;

use crate::Register;
use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::{CompileError, CompileErrorKind, CompileResult};

pub(super) fn local_path_method_call<'expr>(
    cst_path: Option<&'expr [String]>,
    locals: &HashMap<String, Register>,
) -> Option<(&'expr str, &'expr [String])> {
    let path = callee_path_segments(cst_path)?;
    let (method, receiver_path) = path.split_last()?;
    (!receiver_path.is_empty() && locals.contains_key(&receiver_path[0]))
        .then_some((method.as_str(), receiver_path))
}

pub(super) fn path_root_is_local(
    cst_path: Option<&[String]>,
    locals: &HashMap<String, Register>,
) -> bool {
    let Some(path) = callee_path_segments(cst_path) else {
        return false;
    };
    path.first().is_some_and(|root| locals.contains_key(root))
}

pub(super) fn callable_name(cst_path: Option<&[String]>) -> CompileResult<String> {
    let Some(path) = callee_path_segments(cst_path) else {
        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "callable expression",
        )));
    };
    Ok(path.join("::"))
}

pub(super) fn callee_is_closure_call(
    callee_payload: Option<&CompilerExpressionPayload<'_>>,
) -> bool {
    if let Some(payload) = callee_payload {
        return !matches!(payload.syntax_kind(), Some(SyntaxExpressionKind::Path));
    }
    false
}

pub(super) fn callee_path_segments(cst_path: Option<&[String]>) -> Option<&[String]> {
    cst_path
}
