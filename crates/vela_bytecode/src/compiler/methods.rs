use vela_common::{HostMethodId, Span};
use vela_syntax::ast::SyntaxExpressionKind;

use super::body_payloads::CompilerExpressionPayload;
use super::host_paths::{HostPath, HostPathPart, HostPathRoot, ResolvedHostPath};

pub(super) struct HostMethodCall {
    pub(super) receiver: HostPathRoot<'static>,
    pub(super) segments: Vec<HostPathPart<'static>>,
    pub(super) method: HostMethodId,
}

pub(super) fn host_method_call(
    compiler: &super::Compiler<'_, '_>,
    callee_payload: Option<&CompilerExpressionPayload<'_>>,
    receiver_type: Option<&str>,
    path_root_is_local: bool,
) -> Option<HostMethodCall> {
    let callee_payload = callee_payload?;
    match callee_payload.syntax_kind()? {
        SyntaxExpressionKind::Field => {
            let receiver_payload = callee_payload.field_base_payload();
            let receiver = host_method_receiver_path(compiler, receiver_payload.as_ref())?;
            let name = callee_field_name(callee_payload)?;
            let method =
                compiler.host_method_id(receiver_type.or(receiver.type_name.as_deref()), &name)?;
            Some(HostMethodCall {
                receiver: receiver.path.root,
                segments: receiver.path.segments,
                method,
            })
        }
        SyntaxExpressionKind::Path => {
            let cst_path = callee_payload.syntax_path_segments()?;
            let lookup_path = cst_path.as_slice();
            if lookup_path.len() < 2 {
                return None;
            }
            if compiler.is_native_module_root(&lookup_path[0]) && !path_root_is_local {
                return None;
            }
            let method_name = lookup_path.last()?;
            let span = callee_payload.syntax_span()?;
            let receiver =
                host_method_path_receiver(compiler, span, &lookup_path[..lookup_path.len() - 1])?;
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
    receiver_payload: Option<&CompilerExpressionPayload<'ast>>,
) -> Option<ResolvedHostPath<'static>> {
    let payload = receiver_payload?;
    let source = payload.source()?;
    let expression = payload.syntax_expression()?.clone();
    Some(compiler.syntax_host_method_receiver(source, &expression))
}

fn host_method_path_receiver(
    compiler: &super::Compiler<'_, '_>,
    span: Span,
    path: &[String],
) -> Option<ResolvedHostPath<'static>> {
    let root = path.first()?;
    if path.len() == 1 {
        Some(ResolvedHostPath {
            path: HostPath {
                root: HostPathRoot::OwnedLocalPath {
                    name: root.clone(),
                    span,
                },
                segments: Vec::new(),
            },
            type_name: compiler.host_local_type_name(root, span),
        })
    } else {
        compiler.owned_host_field_path_parts(span, path)
    }
}
