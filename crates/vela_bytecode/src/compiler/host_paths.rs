use vela_common::FieldId;
use vela_common::Span;
use vela_syntax::{Argument, Expr, ExprKind};

use crate::{Constant, HostPathSegment, InstructionKind, Register};

use super::{
    CompileError, CompileErrorKind, CompileResult, Compiler, CompilerOptions, reject_named_args,
};

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

impl Compiler<'_> {
    pub(super) fn host_path_push_call(
        &mut self,
        callee: &Expr,
        args: &[Argument],
    ) -> CompileResult<Option<Register>> {
        let path = match &callee.kind {
            ExprKind::Field { base, name } if name == "push" => {
                host_field_path(&self.facts.options, base)
            }
            ExprKind::Path(parts) if parts.last().is_some_and(|name| name == "push") => {
                host_field_path_parts(&self.facts.options, &parts[..parts.len() - 1])
            }
            _ => None,
        };
        let Some(path) = path else {
            return Ok(None);
        };
        reject_named_args(args, "host path push")?;
        let [arg] = args else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "host path push arity",
            )));
        };
        let root = self.compile_host_path_root(callee.span, path.root)?;
        let segments = self.compile_host_path_segments(path.segments)?;
        let value = self.compile_expr(&arg.value)?;
        self.emit(InstructionKind::PushHostPath {
            root,
            segments,
            value,
        });
        let dst = self.alloc_register()?;
        self.emit_constant_to(dst, Constant::Null);
        Ok(Some(dst))
    }

    pub(super) fn host_path_remove_call(
        &mut self,
        callee: &Expr,
        args: &[Argument],
    ) -> CompileResult<Option<Register>> {
        let path = match &callee.kind {
            ExprKind::Field { base, name } if name == "remove" => {
                host_field_path(&self.facts.options, base)
            }
            ExprKind::Path(parts) if parts.last().is_some_and(|name| name == "remove") => {
                host_field_path_parts(&self.facts.options, &parts[..parts.len() - 1])
            }
            _ => None,
        };
        let Some(path) = path else {
            return Ok(None);
        };
        reject_named_args(args, "host path remove")?;
        if !args.is_empty() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "host path remove arity",
            )));
        }
        let root = self.compile_host_path_root(callee.span, path.root)?;
        let segments = self.compile_host_path_segments(path.segments)?;
        self.emit(InstructionKind::RemoveHostPath { root, segments });
        let dst = self.alloc_register()?;
        self.emit_constant_to(dst, Constant::Null);
        Ok(Some(dst))
    }

    pub(super) fn compile_host_path_root<'expr>(
        &mut self,
        span: Span,
        root: HostPathRoot<'expr>,
    ) -> CompileResult<Register> {
        match root {
            HostPathRoot::Expr(expr) => self.compile_expr(expr),
            HostPathRoot::LocalPath(name) => self.local_register_at_span(span, name),
        }
    }

    pub(super) fn compile_host_path_segments<'expr>(
        &mut self,
        segments: Vec<HostPathPart<'expr>>,
    ) -> CompileResult<Vec<HostPathSegment>> {
        segments
            .into_iter()
            .map(|segment| match segment {
                HostPathPart::Field(field) => Ok(HostPathSegment::Field(field)),
                HostPathPart::Value(expr) => self.compile_expr(expr).map(HostPathSegment::Value),
            })
            .collect()
    }
}
