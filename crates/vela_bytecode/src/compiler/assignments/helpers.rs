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

fn record_field_expr_parts_with_payload<'expr>(
    expr: &'expr Expr,
    payload: CompilerExpressionPayload<'expr>,
) -> Option<RecordFieldExprParts<'expr>> {
    match payload.syntax_kind()? {
        SyntaxExpressionKind::Field => match &expr.kind {
            ExprKind::Field { base, .. } => {
                let (base_payload, name) = field_payload_parts(&payload)?;
                let mut parts = record_field_expr_parts_with_payload(base, base_payload)?;
                parts.fields.push(name);
                Some(parts)
            }
            _ => None,
        },
        _ => Some(RecordFieldExprParts {
            root: expr,
            fields: Vec::new(),
        }),
    }
}

pub(super) fn record_field_root_parts_with_payload<'expr>(
    expr: &'expr Expr,
    payload: CompilerExpressionPayload<'expr>,
) -> Option<RecordFieldExprParts<'expr>> {
    let parts = record_field_expr_parts_with_payload(expr, payload)?;
    if parts.fields.is_empty() {
        return None;
    }
    Some(parts)
}

pub(super) fn indexed_record_field_parts_with_payload<'expr>(
    target: &'expr Expr,
    payload: Option<CompilerExpressionPayload<'expr>>,
) -> Option<(&'expr Expr, &'expr Expr, Vec<String>)> {
    let payload = payload?;
    if payload
        .syntax_kind()
        .is_some_and(|kind| kind != SyntaxExpressionKind::Field)
    {
        return None;
    }
    let ExprKind::Field { base, .. } = &target.kind else {
        return None;
    };
    let (base_payload, name) = field_payload_parts(&payload)?;
    let (collection, index, mut fields) =
        indexed_record_field_base_parts_with_payload(base, base_payload)?;
    fields.push(name);
    Some((collection, index, fields))
}

fn indexed_record_field_base_parts_with_payload<'expr>(
    expr: &'expr Expr,
    payload: CompilerExpressionPayload<'expr>,
) -> Option<(&'expr Expr, &'expr Expr, Vec<String>)> {
    match payload.syntax_kind()? {
        SyntaxExpressionKind::Index => {
            let ExprKind::Index { base, index } = &expr.kind else {
                return None;
            };
            let (base_payload, _) = payload.index_operand_payloads()?;
            is_local_index_collection_with_payload(&base_payload).then_some((
                base.as_ref(),
                index.as_ref(),
                Vec::new(),
            ))
        }
        SyntaxExpressionKind::Field => {
            let ExprKind::Field { base, .. } = &expr.kind else {
                return None;
            };
            let (base_payload, name) = field_payload_parts(&payload)?;
            let (collection, index, mut fields) =
                indexed_record_field_base_parts_with_payload(base, base_payload)?;
            fields.push(name);
            Some((collection, index, fields))
        }
        _ => None,
    }
}

fn field_payload_parts<'expr>(
    payload: &CompilerExpressionPayload<'expr>,
) -> Option<(CompilerExpressionPayload<'expr>, String)> {
    Some((payload.field_base_payload()?, payload.syntax_field_name()?))
}

fn is_local_index_collection_with_payload(payload: &CompilerExpressionPayload<'_>) -> bool {
    match payload.syntax_kind() {
        Some(SyntaxExpressionKind::Path) => payload
            .syntax_path_segments()
            .is_some_and(|path| path.len() == 1),
        Some(_) | None => false,
    }
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
