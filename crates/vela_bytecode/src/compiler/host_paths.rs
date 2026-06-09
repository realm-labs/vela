use vela_common::Span;
use vela_common::{FieldId, HostTypeId};
use vela_syntax::ast::{Argument, Expr, ExprKind};

use crate::{CacheSiteId, Constant, HostTargetPlanId, InstructionKind, Register};
use vela_host::resolved::HostMutationOp;
use vela_host::target::HostTargetPlan;

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
    LocalPath { name: &'ast str, span: Span },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HostPathPart<'ast> {
    Field(FieldId),
    VariantField(FieldId),
    Value(&'ast Expr),
}

impl HostPath<'_> {
    pub(super) fn requires_path_instruction(&self) -> bool {
        !matches!(self.segments.as_slice(), [HostPathPart::Field(_)])
    }
}

pub(super) fn host_field_path<'ast>(
    options: &CompilerOptions,
    expr: &'ast Expr,
) -> Option<HostPath<'ast>> {
    match &expr.kind {
        ExprKind::Field { base, name } => {
            let field = host_path_field_part(options, name)?;
            let mut path = host_path_receiver(options, base)?;
            path.segments.push(field);
            Some(path)
        }
        ExprKind::Path(path) => host_field_path_parts(options, expr.span, path),
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
            let field = host_path_field_part(options, name)?;
            let mut path = host_path_receiver(options, base)?;
            path.segments.push(field);
            Some(path)
        }
        ExprKind::Index { base, index } => {
            let mut path = host_path_receiver(options, base)?;
            path.segments.push(HostPathPart::Value(index));
            Some(path)
        }
        ExprKind::Path(path) => host_field_path_parts(options, receiver.span, path).or_else(|| {
            path.first().map(|root| HostPath {
                root: HostPathRoot::LocalPath {
                    name: root,
                    span: receiver.span,
                },
                segments: Vec::new(),
            })
        }),
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
            let field = host_path_field_part(options, name)?;
            let mut path = host_path_receiver(options, base)?;
            path.segments.push(field);
            Some(path)
        }
        ExprKind::Index { base, index } => {
            let mut path = host_path_index_receiver(options, base)?;
            path.segments.push(HostPathPart::Value(index));
            Some(path)
        }
        ExprKind::Path(path) => host_field_path_parts(options, receiver.span, path),
        _ => None,
    }
}

pub(super) fn host_field_path_parts<'ast>(
    options: &CompilerOptions,
    span: Span,
    path: &'ast [String],
) -> Option<HostPath<'ast>> {
    if path.len() < 2 {
        return None;
    }
    let root = path.first()?;
    let segments = path[1..]
        .iter()
        .map(|segment| host_path_field_part(options, segment))
        .collect::<Option<Vec<_>>>()?;
    Some(HostPath {
        root: HostPathRoot::LocalPath { name: root, span },
        segments,
    })
}

fn host_path_field_part<'ast>(options: &CompilerOptions, name: &str) -> Option<HostPathPart<'ast>> {
    options
        .host_field(None, name)
        .map(|field| field.id)
        .map(HostPathPart::Field)
        .or_else(|| {
            options
                .host_variant_fields
                .get(name)
                .copied()
                .map(HostPathPart::VariantField)
        })
}

impl Compiler<'_> {
    pub(super) fn emit_host_read(
        &mut self,
        dst: Register,
        root: Register,
        path: HostPath<'_>,
        span: Span,
    ) -> CompileResult<()> {
        let CompiledHostTarget {
            target,
            dynamic_args,
        } = self.compile_host_target(path)?;
        self.emit_spanned(
            InstructionKind::HostRead {
                dst,
                root,
                target,
                dynamic_args,
                cache_site: CacheSiteId::new(0),
            },
            span,
        );
        Ok(())
    }

    pub(super) fn emit_host_write(
        &mut self,
        root: Register,
        path: HostPath<'_>,
        src: Register,
        span: Span,
    ) -> CompileResult<()> {
        let CompiledHostTarget {
            target,
            dynamic_args,
        } = self.compile_host_target(path)?;
        self.emit_spanned(
            InstructionKind::HostWrite {
                root,
                target,
                dynamic_args,
                src,
                cache_site: CacheSiteId::new(0),
            },
            span,
        );
        Ok(())
    }

    pub(super) fn emit_host_mutate(
        &mut self,
        root: Register,
        path: HostPath<'_>,
        op: HostMutationOp,
        rhs: Register,
        span: Span,
    ) -> CompileResult<()> {
        let CompiledHostTarget {
            target,
            dynamic_args,
        } = self.compile_host_target(path)?;
        self.emit_spanned(
            InstructionKind::HostMutate {
                root,
                target,
                dynamic_args,
                op,
                rhs,
                cache_site: CacheSiteId::new(0),
            },
            span,
        );
        Ok(())
    }

    pub(super) fn emit_host_remove(
        &mut self,
        root: Register,
        path: HostPath<'_>,
        span: Span,
    ) -> CompileResult<()> {
        let CompiledHostTarget {
            target,
            dynamic_args,
        } = self.compile_host_target(path)?;
        self.emit_spanned(
            InstructionKind::HostRemove {
                root,
                target,
                dynamic_args,
                cache_site: CacheSiteId::new(0),
            },
            span,
        );
        Ok(())
    }

    pub(super) fn emit_host_call(
        &mut self,
        dst: Option<Register>,
        root: Register,
        path: HostPath<'_>,
        method: vela_common::HostMethodId,
        args: Vec<Register>,
        span: Span,
    ) -> CompileResult<()> {
        let CompiledHostTarget {
            target,
            dynamic_args,
        } = self.compile_host_target(path)?;
        self.emit_spanned(
            InstructionKind::HostCall {
                dst,
                root,
                target,
                dynamic_args,
                method,
                args,
                cache_site: CacheSiteId::new(0),
            },
            span,
        );
        Ok(())
    }

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
                host_field_path_parts(&self.facts.options, callee.span, &parts[..parts.len() - 1])
            }
            _ => None,
        };
        let Some(path) = path else {
            return Ok(None);
        };
        if path.segments.is_empty() {
            return Ok(None);
        }
        reject_named_args(args, "host path push")?;
        let [arg] = args else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "host path push arity",
            )));
        };
        let root = self.compile_host_path_root(path.root)?;
        let value = self.compile_expr(&arg.value)?;
        self.emit_host_mutate(root, path, HostMutationOp::Push, value, callee.span)?;
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
                host_field_path_parts(&self.facts.options, callee.span, &parts[..parts.len() - 1])
            }
            _ => None,
        };
        let Some(path) = path else {
            return Ok(None);
        };
        if path.segments.is_empty() {
            return Ok(None);
        }
        reject_named_args(args, "host path remove")?;
        if !args.is_empty() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "host path remove arity",
            )));
        }
        let root = self.compile_host_path_root(path.root)?;
        self.emit_host_remove(root, path, callee.span)?;
        let dst = self.alloc_register()?;
        self.emit_constant_to(dst, Constant::Null);
        Ok(Some(dst))
    }

    pub(super) fn compile_host_path_root<'expr>(
        &mut self,
        root: HostPathRoot<'expr>,
    ) -> CompileResult<Register> {
        match root {
            HostPathRoot::Expr(expr) => self.compile_expr(expr),
            HostPathRoot::LocalPath { name, span } => self.local_register_at_span(span, name),
        }
    }

    fn compile_host_target<'expr>(
        &mut self,
        path: HostPath<'expr>,
    ) -> CompileResult<CompiledHostTarget> {
        let root_type = self.host_path_root_type(path.root);
        let mut plan = HostTargetPlan::with_part_capacity(root_type, path.segments.len());
        let mut dynamic_args = Vec::new();
        for segment in path.segments {
            match segment {
                HostPathPart::Field(field) => {
                    plan = plan.field(field);
                }
                HostPathPart::VariantField(field) => {
                    plan = plan.variant_field(field);
                }
                HostPathPart::Value(expr) => {
                    if let Some(arg) = const_host_path_arg(expr) {
                        plan = match arg {
                            ConstHostPathArg::Index(index) => plan.const_index(index),
                            ConstHostPathArg::Key(key) => plan.const_key(key),
                        };
                        continue;
                    }
                    let arg = u8::try_from(dynamic_args.len()).map_err(|_| {
                        CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "host path dynamic argument count",
                        ))
                    })?;
                    let register = self.compile_expr(expr)?;
                    dynamic_args.push(register);
                    plan = plan.dyn_key(arg);
                }
            }
        }
        Ok(CompiledHostTarget {
            target: self.code.intern_host_target(plan),
            dynamic_args,
        })
    }

    fn host_path_root_type(&self, root: HostPathRoot<'_>) -> HostTypeId {
        self.host_path_root_type_name(root)
            .and_then(|type_name| self.facts.options.host_type_id(&type_name))
            .unwrap_or_else(|| HostTypeId::new(0))
    }

    fn host_path_root_type_name(&self, root: HostPathRoot<'_>) -> Option<String> {
        match root {
            HostPathRoot::Expr(expr) => self.script_type_for_expr(expr),
            HostPathRoot::LocalPath { name, span } => self.host_local_type_name(name, span),
        }
    }

    fn host_local_type_name(&self, name: &str, span: Span) -> Option<String> {
        self.script_types
            .local_at_span(self.bindings, span)
            .or_else(|| self.global_type_at_span(span))
            .or_else(|| self.script_types.name(name))
            .or_else(|| self.global_type_named(name))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompiledHostTarget {
    pub(super) target: HostTargetPlanId,
    pub(super) dynamic_args: Vec<Register>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ConstHostPathArg {
    Index(u32),
    Key(String),
}

fn const_host_path_arg(expr: &Expr) -> Option<ConstHostPathArg> {
    match &expr.kind {
        ExprKind::Literal(vela_syntax::ast::Literal::Int(value)) => {
            value.parse::<u32>().ok().map(ConstHostPathArg::Index)
        }
        ExprKind::Literal(vela_syntax::ast::Literal::String(value)) => {
            Some(ConstHostPathArg::Key(value.clone()))
        }
        _ => None,
    }
}
