use vela_common::FieldId;
use vela_syntax::{Expr, ExprKind};

use super::CompilerOptions;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HostPath<'ast> {
    pub(super) root: HostPathRoot<'ast>,
    pub(super) segments: Vec<HostPathPart<'ast>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HostPathRoot<'ast> {
    Expr(&'ast Expr),
    LocalPath(&'ast str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HostPathPart<'ast> {
    Field(FieldId),
    Value(&'ast Expr),
}

pub(super) fn host_field_path<'ast>(
    options: &CompilerOptions,
    expr: &'ast Expr,
) -> Option<HostPath<'ast>> {
    match &expr.kind {
        ExprKind::Field { base, name } => {
            let field = options.host_fields.get(name).copied()?;
            let mut path = host_path_receiver(options, base)?;
            path.segments.push(HostPathPart::Field(field));
            Some(path)
        }
        ExprKind::Path(path) => host_field_path_parts(options, path),
        ExprKind::Index { base, index } => {
            let mut path = host_path_index_receiver(options, base)?;
            path.segments.push(HostPathPart::Value(index));
            Some(path)
        }
        _ => None,
    }
}

fn host_path_receiver<'ast>(
    options: &CompilerOptions,
    receiver: &'ast Expr,
) -> Option<HostPath<'ast>> {
    match &receiver.kind {
        ExprKind::Field { base, name } => {
            let field = options.host_fields.get(name).copied()?;
            let mut path = host_path_receiver(options, base)?;
            path.segments.push(HostPathPart::Field(field));
            Some(path)
        }
        ExprKind::Index { base, index } => {
            let mut path = host_path_receiver(options, base)?;
            path.segments.push(HostPathPart::Value(index));
            Some(path)
        }
        ExprKind::Path(path) => host_field_path_parts(options, path),
        _ => Some(HostPath {
            root: HostPathRoot::Expr(receiver),
            segments: Vec::new(),
        }),
    }
}

fn host_path_index_receiver<'ast>(
    options: &CompilerOptions,
    receiver: &'ast Expr,
) -> Option<HostPath<'ast>> {
    match &receiver.kind {
        ExprKind::Field { base, name } => {
            let field = options.host_fields.get(name).copied()?;
            let mut path = host_path_receiver(options, base)?;
            path.segments.push(HostPathPart::Field(field));
            Some(path)
        }
        ExprKind::Index { base, index } => {
            let mut path = host_path_index_receiver(options, base)?;
            path.segments.push(HostPathPart::Value(index));
            Some(path)
        }
        ExprKind::Path(path) => host_field_path_parts(options, path),
        _ => None,
    }
}

pub(super) fn host_field_path_parts<'ast>(
    options: &CompilerOptions,
    path: &'ast [String],
) -> Option<HostPath<'ast>> {
    if path.len() < 2 {
        return None;
    }
    let root = path.first()?;
    let segments = path[1..]
        .iter()
        .map(|segment| options.host_fields.get(segment).copied())
        .map(|field| field.map(HostPathPart::Field))
        .collect::<Option<Vec<_>>>()?;
    Some(HostPath {
        root: HostPathRoot::LocalPath(root),
        segments,
    })
}
