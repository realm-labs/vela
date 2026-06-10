use vela_common::HostTypeId;
use vela_common::Span;
use vela_def::FieldId;
use vela_syntax::ast::{Argument, Expr, ExprKind};

use crate::{CacheSiteId, Constant, HostTargetPlanId, InstructionKind, Register};
use vela_host::resolved::HostMutationOp;
use vela_host::target::HostTargetPlan;

use super::{CompileError, CompileErrorKind, CompileResult, Compiler, reject_named_args};

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

impl Compiler<'_, '_> {
    pub(super) fn host_field_path<'ast>(&self, expr: &'ast Expr) -> Option<HostPath<'ast>> {
        self.resolve_host_path(expr).map(|resolved| resolved.path)
    }

    pub(super) fn resolve_host_path<'ast>(
        &self,
        expr: &'ast Expr,
    ) -> Option<ResolvedHostPath<'ast>> {
        match &expr.kind {
            ExprKind::Field { base, name } => {
                let mut receiver = self.resolve_host_path_receiver(base);
                let field = self.host_path_field_part(receiver.type_name.as_deref(), name)?;
                receiver.path.segments.push(field.part);
                Some(ResolvedHostPath {
                    path: receiver.path,
                    type_name: field.type_hint,
                })
            }
            ExprKind::Path(path) => self.host_field_path_parts(expr.span, path),
            ExprKind::Index { base, index } => {
                let mut receiver = self.resolve_host_path_index_receiver(base)?;
                receiver.path.segments.push(HostPathPart::Value(index));
                let value_type = receiver.type_name.as_deref().and_then(|type_name| {
                    self.facts
                        .options
                        .host_index_capability(type_name)
                        .and_then(|capability| capability.value_type.clone())
                });
                Some(ResolvedHostPath {
                    path: receiver.path,
                    type_name: value_type,
                })
            }
            _ => None,
        }
    }

    fn resolve_host_path_receiver<'ast>(&self, receiver: &'ast Expr) -> ResolvedHostPath<'ast> {
        match &receiver.kind {
            ExprKind::Field { .. } | ExprKind::Index { .. } => self
                .resolve_host_path(receiver)
                .unwrap_or_else(|| self.expr_host_path_receiver(receiver)),
            ExprKind::Path(path) => self
                .host_field_path_parts(receiver.span, path)
                .or_else(|| {
                    path.first().map(|root| ResolvedHostPath {
                        path: HostPath {
                            root: HostPathRoot::LocalPath {
                                name: root,
                                span: receiver.span,
                            },
                            segments: Vec::new(),
                        },
                        type_name: self.host_local_type_name(root, receiver.span),
                    })
                })
                .unwrap_or_else(|| self.expr_host_path_receiver(receiver)),
            _ => self.expr_host_path_receiver(receiver),
        }
    }

    fn expr_host_path_receiver<'ast>(&self, receiver: &'ast Expr) -> ResolvedHostPath<'ast> {
        ResolvedHostPath {
            path: HostPath {
                root: HostPathRoot::Expr(receiver),
                segments: Vec::new(),
            },
            type_name: self.script_type_for_expr(receiver),
        }
    }

    fn resolve_host_path_index_receiver<'ast>(
        &self,
        receiver: &'ast Expr,
    ) -> Option<ResolvedHostPath<'ast>> {
        match &receiver.kind {
            ExprKind::Field { .. } | ExprKind::Index { .. } => self.resolve_host_path(receiver),
            ExprKind::Path(path) => self.host_field_path_parts(receiver.span, path),
            _ => None,
        }
    }

    pub(super) fn host_field_path_parts<'ast>(
        &self,
        span: Span,
        path: &'ast [String],
    ) -> Option<ResolvedHostPath<'ast>> {
        if path.len() < 2 {
            return None;
        }
        let root = path.first()?;
        let mut current_type = self.host_local_type_name(root, span);
        let mut segments = Vec::with_capacity(path.len() - 1);
        for segment in &path[1..] {
            let field = self.host_path_field_part(current_type.as_deref(), segment)?;
            segments.push(field.part);
            current_type = field.type_hint;
        }
        Some(ResolvedHostPath {
            path: HostPath {
                root: HostPathRoot::LocalPath { name: root, span },
                segments,
            },
            type_name: current_type,
        })
    }

    fn host_path_field_part<'ast>(
        &self,
        receiver_type: Option<&str>,
        name: &str,
    ) -> Option<ResolvedHostPathField<'ast>> {
        if let Some(field) = self.host_field_info(receiver_type, name) {
            return Some(ResolvedHostPathField {
                part: if field.variant_field {
                    HostPathPart::VariantField(field.id)
                } else {
                    HostPathPart::Field(field.id)
                },
                type_hint: field.type_hint,
            });
        }
        None
    }

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
            ExprKind::Field { base, name } if name == "push" => self.host_field_path(base),
            ExprKind::Path(parts) if parts.last().is_some_and(|name| name == "push") => self
                .host_field_path_parts(callee.span, &parts[..parts.len() - 1])
                .map(|resolved| resolved.path),
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
            ExprKind::Field { base, name } if name == "remove" => self.host_field_path(base),
            ExprKind::Path(parts) if parts.last().is_some_and(|name| name == "remove") => self
                .host_field_path_parts(callee.span, &parts[..parts.len() - 1])
                .map(|resolved| resolved.path),
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
            .and_then(|type_name| self.host_runtime_type_id(&type_name))
            .unwrap_or_else(|| HostTypeId::new(0))
    }

    fn host_path_root_type_name(&self, root: HostPathRoot<'_>) -> Option<String> {
        match root {
            HostPathRoot::Expr(expr) => self.script_type_for_expr(expr),
            HostPathRoot::LocalPath { name, span } => self.host_local_type_name(name, span),
        }
    }

    pub(super) fn host_local_type_name(&self, name: &str, span: Span) -> Option<String> {
        self.script_types
            .local_at_span(self.bindings, span)
            .or_else(|| self.global_type_at_span(span))
            .or_else(|| self.script_types.name(name))
            .or_else(|| self.global_type_named(name))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ResolvedHostPath<'ast> {
    pub(super) path: HostPath<'ast>,
    pub(super) type_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResolvedHostPathField<'ast> {
    part: HostPathPart<'ast>,
    type_hint: Option<String>,
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
