use vela_syntax::ast::{AssignOp, Expr, ExprKind, SyntaxExpressionKind};

use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::operators::compound_assignment_instruction;
use crate::compiler::value_types::RuntimeTypeFact;
use crate::compiler::{CompileError, CompileErrorKind, CompileResult};
use crate::{Register, UnlinkedInstructionKind};

use super::RecordFieldExprParts;

pub(super) fn expressions_are_i64(
    left: Option<RuntimeTypeFact>,
    right: Option<RuntimeTypeFact>,
) -> bool {
    matches!(
        (left, right),
        (
            Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64)),
            Some(RuntimeTypeFact::Primitive(vela_common::PrimitiveTag::I64))
        )
    )
}

pub(super) fn record_path_parts(path: &[String]) -> Option<(&str, Vec<String>)> {
    if path.len() < 2 {
        return None;
    }
    record_field_base_parts(path)
}

pub(super) fn record_field_base_parts(path: &[String]) -> Option<(&str, Vec<String>)> {
    let root = path.first()?;
    Some((root.as_str(), path[1..].to_vec()))
}

pub(super) fn record_field_expr_parts_with_payload<'expr>(
    expr: &'expr Expr,
    payload: Option<CompilerExpressionPayload<'expr>>,
) -> Option<RecordFieldExprParts<'expr>> {
    if payload
        .as_ref()
        .and_then(CompilerExpressionPayload::syntax_kind)
        .is_some_and(|kind| kind != SyntaxExpressionKind::Field)
    {
        return None;
    }
    match &expr.kind {
        ExprKind::Field { base, name } => {
            let (base_payload, name) = field_payload_parts(payload.as_ref(), name)?;
            let mut parts = record_field_expr_parts_with_payload(base, base_payload)
                .unwrap_or_else(|| RecordFieldExprParts {
                    root: base,
                    fields: Vec::new(),
                });
            parts.fields.push(name);
            Some(parts)
        }
        _ => None,
    }
}

pub(super) fn indexed_record_field_parts_with_payload<'expr>(
    target: &'expr Expr,
    payload: Option<CompilerExpressionPayload<'expr>>,
) -> Option<(&'expr Expr, &'expr Expr, Vec<String>)> {
    if payload
        .as_ref()
        .and_then(CompilerExpressionPayload::syntax_kind)
        .is_some_and(|kind| kind != SyntaxExpressionKind::Field)
    {
        return None;
    }
    let ExprKind::Field { base, name } = &target.kind else {
        return None;
    };
    let (base_payload, name) = field_payload_parts(payload.as_ref(), name)?;
    let (collection, index, mut fields) =
        indexed_record_field_base_parts_with_payload(base, base_payload)?;
    fields.push(name);
    Some((collection, index, fields))
}

fn indexed_record_field_base_parts_with_payload<'expr>(
    expr: &'expr Expr,
    payload: Option<CompilerExpressionPayload<'expr>>,
) -> Option<(&'expr Expr, &'expr Expr, Vec<String>)> {
    if let Some(kind) = payload
        .as_ref()
        .and_then(CompilerExpressionPayload::syntax_kind)
    {
        return match kind {
            SyntaxExpressionKind::Index => {
                let ExprKind::Index { base, index } = &expr.kind else {
                    return None;
                };
                let (base_payload, _) = payload.as_ref()?.index_operand_payloads()?;
                is_local_index_collection_with_payload(base, Some(&base_payload)).then_some((
                    base.as_ref(),
                    index.as_ref(),
                    Vec::new(),
                ))
            }
            SyntaxExpressionKind::Field => {
                let ExprKind::Field { base, name } = &expr.kind else {
                    return None;
                };
                let (base_payload, name) = field_payload_parts(payload.as_ref(), name)?;
                let (collection, index, mut fields) =
                    indexed_record_field_base_parts_with_payload(base, base_payload)?;
                fields.push(name);
                Some((collection, index, fields))
            }
            _ => None,
        };
    }
    match &expr.kind {
        ExprKind::Index { base, index } if is_local_index_collection(base) => {
            Some((base, index, Vec::new()))
        }
        ExprKind::Field { base, name } => {
            let (base_payload, name) = field_payload_parts(payload.as_ref(), name)?;
            let (collection, index, mut fields) =
                indexed_record_field_base_parts_with_payload(base, base_payload)?;
            fields.push(name);
            Some((collection, index, fields))
        }
        _ => None,
    }
}

fn field_payload_parts<'expr>(
    payload: Option<&CompilerExpressionPayload<'expr>>,
    default_name: &str,
) -> Option<(Option<CompilerExpressionPayload<'expr>>, String)> {
    match payload {
        Some(payload) => Some((
            Some(payload.field_base_payload()?),
            payload.syntax_field_name()?,
        )),
        None => Some((None, default_name.to_owned())),
    }
}

pub(super) fn is_local_index_collection(expr: &Expr) -> bool {
    matches!(&expr.kind, ExprKind::Path(path) if path.len() == 1)
}

fn is_local_index_collection_with_payload(
    expr: &Expr,
    payload: Option<&CompilerExpressionPayload<'_>>,
) -> bool {
    if let Some(payload) = payload {
        return match payload.syntax_kind() {
            Some(SyntaxExpressionKind::Path) | None => payload
                .syntax_path_segments()
                .is_some_and(|path| path.len() == 1),
            Some(_) => false,
        };
    }
    is_local_index_collection(expr)
}

pub(super) fn compound_assignment_instruction_or_error(
    op: AssignOp,
    dst: Register,
    lhs: Register,
    rhs: Register,
) -> CompileResult<UnlinkedInstructionKind> {
    compound_assignment_instruction(op, dst, lhs, rhs).ok_or_else(|| {
        CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "compound assignment operator",
        ))
    })
}
