use vela_common::FieldId;
use vela_syntax::{Expr, ExprKind};

use super::CompilerOptions;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HostFieldPath<'ast> {
    pub(super) root: HostPathRoot<'ast>,
    pub(super) fields: Vec<FieldId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HostPathRoot<'ast> {
    Expr(&'ast Expr),
    LocalPath(&'ast str),
}

pub(super) fn host_field_path<'ast>(
    options: &CompilerOptions,
    expr: &'ast Expr,
) -> Option<HostFieldPath<'ast>> {
    match &expr.kind {
        ExprKind::Field { base, name } => {
            let field = options.host_fields.get(name).copied()?;
            let mut path = host_field_receiver_path(options, base)?;
            path.fields.push(field);
            Some(path)
        }
        ExprKind::Path(path) => host_field_path_parts(options, path),
        _ => None,
    }
}

fn host_field_receiver_path<'ast>(
    options: &CompilerOptions,
    receiver: &'ast Expr,
) -> Option<HostFieldPath<'ast>> {
    match &receiver.kind {
        ExprKind::Field { base, name } => {
            let field = options.host_fields.get(name).copied()?;
            let mut path = host_field_receiver_path(options, base)?;
            path.fields.push(field);
            Some(path)
        }
        ExprKind::Path(path) => host_field_path_parts(options, path),
        _ => Some(HostFieldPath {
            root: HostPathRoot::Expr(receiver),
            fields: Vec::new(),
        }),
    }
}

pub(super) fn host_field_path_parts<'ast>(
    options: &CompilerOptions,
    path: &'ast [String],
) -> Option<HostFieldPath<'ast>> {
    if path.len() < 2 {
        return None;
    }
    let root = path.first()?;
    let fields = path[1..]
        .iter()
        .map(|segment| options.host_fields.get(segment).copied())
        .collect::<Option<Vec<_>>>()?;
    Some(HostFieldPath {
        root: HostPathRoot::LocalPath(root),
        fields,
    })
}
