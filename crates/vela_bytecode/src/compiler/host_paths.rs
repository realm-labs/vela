use vela_common::SourceId;
use vela_common::Span;
use vela_common::{Diagnostic, HostTypeId};
use vela_def::FieldId;
use vela_syntax::ast::{
    Argument, AstNode, Expr, ExprKind, Literal, SyntaxExpression, SyntaxExpressionKind,
};

use crate::{CacheSiteId, Constant, HostTargetPlanId, Register, UnlinkedInstructionKind};
use vela_host::resolved::HostMutationOp;
use vela_host::target::HostTargetPlan;

use super::body_payloads::{CompilerExpressionPayload, expression_syntax_path_or_self};
use super::call_args::CallArgumentSyntax;
use super::expression_checks::payload_aligns_with_expr_span;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler, reject_named_args};

pub(super) struct HostPath<'ast> {
    pub(super) root: HostPathRoot<'ast>,
    pub(super) segments: Vec<HostPathPart<'ast>>,
}

#[derive(Clone)]
pub(super) enum HostPathRoot<'ast> {
    Expr {
        expr: &'ast Expr,
        payload: Option<CompilerExpressionPayload<'ast>>,
    },
    LocalPath {
        name: &'ast str,
        span: Span,
    },
    OwnedLocalPath {
        name: String,
        span: Span,
    },
}

pub(super) enum HostPathPart<'ast> {
    Field(FieldId),
    VariantField(FieldId),
    Value {
        expr: &'ast Expr,
        payload: Option<CompilerExpressionPayload<'ast>>,
        dynamic_kind: DynamicHostPathPart,
    },
    SyntaxValue {
        source: SourceId,
        expression: SyntaxExpression,
        dynamic_kind: DynamicHostPathPart,
    },
}

struct HostCollectionMethodTarget<'ast> {
    path: HostPath<'ast>,
    field_receiver: Option<HostCollectionFieldReceiver<'ast>>,
}

struct HostCollectionFieldReceiver<'ast> {
    expr: Option<&'ast Expr>,
    payload: Option<CompilerExpressionPayload<'ast>>,
}

#[derive(Clone, Copy)]
pub(super) enum DynamicHostPathPart {
    Index,
    Key,
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

    pub(super) fn host_field_path_with_payload<'ast>(
        &self,
        expr: &'ast Expr,
        payload: Option<&CompilerExpressionPayload<'ast>>,
    ) -> Option<HostPath<'ast>> {
        self.resolve_host_path_with_payload(expr, payload)
            .map(|resolved| resolved.path)
    }

    pub(super) fn resolve_host_path_with_payload<'ast>(
        &self,
        expr: &'ast Expr,
        payload: Option<&CompilerExpressionPayload<'ast>>,
    ) -> Option<ResolvedHostPath<'ast>> {
        self.resolve_host_path_with_owned_payload(expr, payload.cloned())
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
                let dynamic_kind = receiver
                    .type_name
                    .as_deref()
                    .and_then(|type_name| self.facts.options.host_index_capability(type_name))
                    .and_then(|capability| capability.key_type.as_deref())
                    .map_or(DynamicHostPathPart::Key, dynamic_host_path_part);
                receiver.path.segments.push(HostPathPart::Value {
                    expr: index,
                    payload: None,
                    dynamic_kind,
                });
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

    fn host_path_from_syntax_path_payload<'ast>(
        &self,
        payload: CompilerExpressionPayload<'ast>,
    ) -> Option<ResolvedHostPath<'ast>> {
        let path = payload.syntax_path_segments()?;
        let span = payload.syntax_span()?;
        match path.len() {
            0 => None,
            1 => {
                let name = path.into_iter().next()?;
                let type_name = self.host_local_type_name(&name, span);
                Some(ResolvedHostPath {
                    path: HostPath {
                        root: HostPathRoot::OwnedLocalPath { name, span },
                        segments: Vec::new(),
                    },
                    type_name,
                })
            }
            _ => self.owned_host_field_path_parts(span, &path),
        }
    }

    fn resolve_host_path_with_owned_payload<'ast>(
        &self,
        expr: &'ast Expr,
        payload: Option<CompilerExpressionPayload<'ast>>,
    ) -> Option<ResolvedHostPath<'ast>> {
        match &expr.kind {
            ExprKind::Field { base, name } => {
                let name = host_path_field_name(payload.as_ref(), name)?;
                let base_payload = payload
                    .as_ref()
                    .and_then(CompilerExpressionPayload::field_base_payload);
                let mut receiver = self.resolve_host_path_receiver_with_payload(base, base_payload);
                let field = self.host_path_field_part(receiver.type_name.as_deref(), &name)?;
                receiver.path.segments.push(field.part);
                Some(ResolvedHostPath {
                    path: receiver.path,
                    type_name: field.type_hint,
                })
            }
            ExprKind::Path(_) => match payload
                .as_ref()
                .and_then(CompilerExpressionPayload::syntax_kind)
            {
                Some(SyntaxExpressionKind::Path) => {
                    self.host_path_from_syntax_path_payload(payload.clone()?)
                }
                Some(_) => None,
                None if payload
                    .as_ref()
                    .is_some_and(CompilerExpressionPayload::has_missing_syntax) =>
                {
                    None
                }
                None => self.resolve_host_path(expr),
            },
            ExprKind::Index { base, index } => {
                let (base_payload, index_payload) = match payload.as_ref() {
                    Some(payload) => {
                        let (base, index) = payload.index_operand_payloads()?;
                        (Some(base), Some(index))
                    }
                    None => (None, None),
                };
                let mut receiver =
                    self.resolve_host_path_index_receiver_with_payload(base, base_payload)?;
                let dynamic_kind = receiver
                    .type_name
                    .as_deref()
                    .and_then(|type_name| self.facts.options.host_index_capability(type_name))
                    .and_then(|capability| capability.key_type.as_deref())
                    .map_or(DynamicHostPathPart::Key, dynamic_host_path_part);
                receiver.path.segments.push(HostPathPart::Value {
                    expr: index,
                    payload: index_payload,
                    dynamic_kind,
                });
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

    fn resolve_host_path_receiver_with_payload<'ast>(
        &self,
        receiver: &'ast Expr,
        payload: Option<CompilerExpressionPayload<'ast>>,
    ) -> ResolvedHostPath<'ast> {
        match &receiver.kind {
            ExprKind::Field { .. } | ExprKind::Index { .. } => self
                .resolve_host_path_with_owned_payload(receiver, payload.clone())
                .unwrap_or_else(|| self.expr_host_path_receiver_with_payload(receiver, payload)),
            ExprKind::Path(_) => match payload {
                Some(payload) => self
                    .host_path_from_syntax_path_payload(payload.clone())
                    .unwrap_or_else(|| {
                        self.expr_host_path_receiver_with_payload(receiver, Some(payload))
                    }),
                None => self
                    .resolve_host_path(receiver)
                    .unwrap_or_else(|| self.expr_host_path_receiver(receiver)),
            },
            _ => self.expr_host_path_receiver_with_payload(receiver, payload),
        }
    }

    fn expr_host_path_receiver<'ast>(&self, receiver: &'ast Expr) -> ResolvedHostPath<'ast> {
        ResolvedHostPath {
            path: HostPath {
                root: HostPathRoot::Expr {
                    expr: receiver,
                    payload: None,
                },
                segments: Vec::new(),
            },
            type_name: None,
        }
    }

    fn expr_host_path_receiver_with_payload<'ast>(
        &self,
        receiver: &'ast Expr,
        payload: Option<CompilerExpressionPayload<'ast>>,
    ) -> ResolvedHostPath<'ast> {
        let type_name = self.script_type_for_expr_with_payload(receiver, payload.as_ref());
        ResolvedHostPath {
            path: HostPath {
                root: HostPathRoot::Expr {
                    expr: receiver,
                    payload,
                },
                segments: Vec::new(),
            },
            type_name,
        }
    }

    fn resolve_host_path_index_receiver<'ast>(
        &self,
        receiver: &'ast Expr,
    ) -> Option<ResolvedHostPath<'ast>> {
        match &receiver.kind {
            ExprKind::Field { .. } | ExprKind::Index { .. } => self.resolve_host_path(receiver),
            ExprKind::Path(path) => self
                .host_field_path_parts(receiver.span, path)
                .or_else(|| self.host_index_root_path(receiver.span, path)),
            _ => None,
        }
    }

    fn resolve_host_path_index_receiver_with_payload<'ast>(
        &self,
        receiver: &'ast Expr,
        payload: Option<CompilerExpressionPayload<'ast>>,
    ) -> Option<ResolvedHostPath<'ast>> {
        match &receiver.kind {
            ExprKind::Field { .. } | ExprKind::Index { .. } => {
                self.resolve_host_path_with_owned_payload(receiver, payload)
            }
            ExprKind::Path(path) => match payload {
                Some(payload) => self.host_index_payload_root_path(receiver, Some(payload)),
                None => self
                    .resolve_host_path(receiver)
                    .or_else(|| self.host_index_root_path(receiver.span, path)),
            },
            _ => None,
        }
    }

    fn host_index_payload_root_path<'ast>(
        &self,
        receiver: &'ast Expr,
        payload: Option<CompilerExpressionPayload<'ast>>,
    ) -> Option<ResolvedHostPath<'ast>> {
        let payload = payload?;
        let cst_path = payload.syntax_path_segments()?;
        if cst_path.len() != 1 {
            return None;
        }
        let type_name = self
            .script_type_for_payload(&payload)
            .or_else(|| self.script_type_for_expr_with_payload(receiver, Some(&payload)))?;
        self.facts.options.host_index_capability(&type_name)?;
        Some(ResolvedHostPath {
            path: HostPath {
                root: HostPathRoot::Expr {
                    expr: receiver,
                    payload: Some(payload),
                },
                segments: Vec::new(),
            },
            type_name: Some(type_name),
        })
    }

    fn host_index_root_path<'ast>(
        &self,
        span: Span,
        path: &'ast [String],
    ) -> Option<ResolvedHostPath<'ast>> {
        if path.len() != 1 {
            return None;
        }
        let root = path.first()?;
        let type_name = self.host_local_type_name(root, span)?;
        self.facts.options.host_index_capability(&type_name)?;
        Some(ResolvedHostPath {
            path: HostPath {
                root: HostPathRoot::LocalPath { name: root, span },
                segments: Vec::new(),
            },
            type_name: Some(type_name),
        })
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

    pub(super) fn owned_host_field_path_parts<'ast>(
        &self,
        span: Span,
        path: &[String],
    ) -> Option<ResolvedHostPath<'ast>> {
        if path.len() < 2 {
            return None;
        }
        let root = path.first()?.clone();
        let mut current_type = self.host_local_type_name(&root, span);
        let mut segments = Vec::with_capacity(path.len() - 1);
        for segment in &path[1..] {
            let field = self.host_path_field_part(current_type.as_deref(), segment)?;
            segments.push(field.part);
            current_type = field.type_hint;
        }
        Some(ResolvedHostPath {
            path: HostPath {
                root: HostPathRoot::OwnedLocalPath { name: root, span },
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
            UnlinkedInstructionKind::HostRead {
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
            UnlinkedInstructionKind::HostWrite {
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
            UnlinkedInstructionKind::HostMutate {
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
            UnlinkedInstructionKind::HostRemove {
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
            UnlinkedInstructionKind::HostCall {
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
        callee_payload: Option<&CompilerExpressionPayload<'_>>,
        args: &[Argument],
        arg_syntax: CallArgumentSyntax<'_, '_>,
    ) -> CompileResult<Option<Register>> {
        let Some(target) = self.host_collection_method_target(callee, callee_payload, "push")
        else {
            return Ok(None);
        };
        let path = target.path;
        if path.segments.is_empty() {
            return Ok(None);
        }
        reject_named_args(args, "host path push")?;
        let [arg] = args else {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "host path push arity",
            )));
        };
        let root = self.compile_host_path_root(&path.root)?;
        let value = self.compile_call_argument_value(arg, arg_syntax)?;
        self.emit_host_mutate(root, path, HostMutationOp::Push, value, callee.span)?;
        let dst = self.alloc_register()?;
        self.emit_constant_to(dst, Constant::Null);
        Ok(Some(dst))
    }

    pub(super) fn host_path_remove_call(
        &mut self,
        callee: &Expr,
        callee_payload: Option<&CompilerExpressionPayload<'_>>,
        args: &[Argument],
    ) -> CompileResult<Option<Register>> {
        let Some(target) = self.host_collection_method_target(callee, callee_payload, "remove")
        else {
            return Ok(None);
        };
        let path = target.path;
        if path.segments.is_empty() {
            return Ok(None);
        }
        reject_named_args(args, "host path remove")?;
        if !args.is_empty() {
            return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                "host path remove arity",
            )));
        }
        if let Some(base) = target.field_receiver {
            self.reject_terminal_host_index_receiver_access(base, HostIndexAccessKind::Remove)?;
        }
        let root = self.compile_host_path_root(&path.root)?;
        self.emit_host_remove(root, path, callee.span)?;
        let dst = self.alloc_register()?;
        self.emit_constant_to(dst, Constant::Null);
        Ok(Some(dst))
    }

    fn host_collection_method_target<'ast>(
        &self,
        callee: &'ast Expr,
        callee_payload: Option<&CompilerExpressionPayload<'ast>>,
        method: &str,
    ) -> Option<HostCollectionMethodTarget<'ast>> {
        if let Some(payload) = callee_payload {
            return match payload.syntax_kind() {
                Some(SyntaxExpressionKind::Field) | None => {
                    if payload.syntax_field_name()?.as_str() != method {
                        return None;
                    }
                    let ExprKind::Field { base, .. } = &callee.kind else {
                        return None;
                    };
                    let base = base.as_ref();
                    let base_payload = payload.field_base_payload()?;
                    let base_expr =
                        payload_aligns_with_expr_span(&base_payload, base).then_some(base);
                    let path = self.host_field_path_with_payload(base, Some(&base_payload))?;
                    Some(HostCollectionMethodTarget {
                        path,
                        field_receiver: Some(HostCollectionFieldReceiver {
                            expr: base_expr,
                            payload: Some(base_payload),
                        }),
                    })
                }
                Some(SyntaxExpressionKind::Path) => {
                    let parts = payload.syntax_path_segments()?;
                    if parts.last().is_none_or(|name| name != method) {
                        return None;
                    }
                    let span = payload.syntax_span()?;
                    self.owned_host_field_path_parts(span, &parts[..parts.len() - 1])
                        .map(|resolved| HostCollectionMethodTarget {
                            path: resolved.path,
                            field_receiver: None,
                        })
                }
                Some(_) => None,
            };
        }

        match &callee.kind {
            ExprKind::Field { base, name } if name == method => {
                self.host_field_path(base)
                    .map(|path| HostCollectionMethodTarget {
                        path,
                        field_receiver: Some(HostCollectionFieldReceiver {
                            expr: Some(base),
                            payload: None,
                        }),
                    })
            }
            ExprKind::Path(parts) if parts.last().is_some_and(|name| name == method) => self
                .host_field_path_parts(callee.span, &parts[..parts.len() - 1])
                .map(|resolved| HostCollectionMethodTarget {
                    path: resolved.path,
                    field_receiver: None,
                }),
            _ => None,
        }
    }

    fn reject_terminal_host_index_receiver_access(
        &self,
        receiver: HostCollectionFieldReceiver<'_>,
        kind: HostIndexAccessKind,
    ) -> CompileResult<()> {
        let Some(expr) = receiver.expr else {
            return Ok(());
        };
        self.reject_terminal_host_index_access(expr, receiver.payload.as_ref(), kind)
    }

    #[cfg(test)]
    pub(in crate::compiler) fn host_collection_method_target_root_name_for_test<'ast>(
        &self,
        callee: &'ast Expr,
        callee_payload: Option<&CompilerExpressionPayload<'ast>>,
        method: &str,
    ) -> Option<String> {
        let target = self.host_collection_method_target(callee, callee_payload, method)?;
        match target.path.root {
            HostPathRoot::Expr { .. } => Some("<expr>".to_owned()),
            HostPathRoot::LocalPath { name, .. } => Some(name.to_owned()),
            HostPathRoot::OwnedLocalPath { name, .. } => Some(name),
        }
    }

    pub(super) fn compile_host_path_root<'expr>(
        &mut self,
        root: &HostPathRoot<'expr>,
    ) -> CompileResult<Register> {
        match root {
            HostPathRoot::Expr { expr, payload } => {
                self.compile_expr_with_payload(expr, payload.as_ref())
            }
            HostPathRoot::LocalPath { name, span } => self.local_register_at_span(*span, name),
            HostPathRoot::OwnedLocalPath { name, span } => self.local_register_at_span(*span, name),
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
                HostPathPart::Value {
                    expr,
                    payload,
                    dynamic_kind,
                } => {
                    if let Some(arg) = const_host_path_arg_with_payload(payload.as_ref()) {
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
                    let register = self.compile_expr_with_payload(expr, payload.as_ref())?;
                    dynamic_args.push(register);
                    plan = match dynamic_kind {
                        DynamicHostPathPart::Index => plan.dyn_index(arg),
                        DynamicHostPathPart::Key => plan.dyn_key(arg),
                    };
                }
                HostPathPart::SyntaxValue {
                    source,
                    expression,
                    dynamic_kind,
                } => {
                    let arg = u8::try_from(dynamic_args.len()).map_err(|_| {
                        CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "host path dynamic argument count",
                        ))
                    })?;
                    let register = self
                        .compile_syntax_expression(source, &expression)?
                        .ok_or_else(|| {
                            CompileError::new(CompileErrorKind::UnsupportedSyntax(
                                "host path dynamic argument",
                            ))
                        })?;
                    dynamic_args.push(register);
                    plan = match dynamic_kind {
                        DynamicHostPathPart::Index => plan.dyn_index(arg),
                        DynamicHostPathPart::Key => plan.dyn_key(arg),
                    };
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
            HostPathRoot::Expr {
                expr,
                payload: Some(payload),
            } => self
                .script_type_for_payload(&payload)
                .or_else(|| self.script_type_for_expr_with_payload(expr, Some(&payload))),
            HostPathRoot::Expr { payload: None, .. } => None,
            HostPathRoot::LocalPath { name, span } => self.host_local_type_name(name, span),
            HostPathRoot::OwnedLocalPath { name, span } => self.host_local_type_name(&name, span),
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) fn host_path_root_type_name_for_test(
        &self,
        root: HostPathRoot<'_>,
    ) -> Option<String> {
        self.host_path_root_type_name(root)
    }

    pub(super) fn host_local_type_name(&self, name: &str, span: Span) -> Option<String> {
        self.script_types
            .local_at_span(self.bindings, span)
            .or_else(|| self.global_type_at_span(span))
            .or_else(|| self.script_types.name(name))
            .or_else(|| self.global_type_named(name))
    }

    pub(in crate::compiler) fn syntax_root_host_index_path(
        &self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> Option<HostPath<'static>> {
        let index = expression.as_index()?;
        let receiver = index.receiver()?;
        let index_expression = index.index()?;
        let path = expression_syntax_path_or_self(&receiver)?;
        let [name] = path.as_slice() else {
            return None;
        };
        let span = syntax_host_expression_span(source, &receiver);
        let type_name = self.host_local_type_name(name, span)?;
        let dynamic_kind = self
            .facts
            .options
            .host_index_capability(&type_name)
            .and_then(|capability| capability.key_type.as_deref())
            .map_or(DynamicHostPathPart::Key, dynamic_host_path_part);
        Some(HostPath {
            root: HostPathRoot::OwnedLocalPath {
                name: name.clone(),
                span,
            },
            segments: vec![HostPathPart::SyntaxValue {
                source,
                expression: index_expression,
                dynamic_kind,
            }],
        })
    }

    pub(super) fn reject_invalid_host_index_access(
        &self,
        expr: &Expr,
        base: &Expr,
        index: &Expr,
        kind: HostIndexAccessKind,
    ) -> CompileResult<()> {
        self.reject_invalid_host_index_access_with_payload(expr, base, index, kind, None, None)
    }

    pub(in crate::compiler) fn reject_invalid_host_index_access_with_payload(
        &self,
        expr: &Expr,
        base: &Expr,
        index: &Expr,
        kind: HostIndexAccessKind,
        base_payload: Option<&CompilerExpressionPayload<'_>>,
        index_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<()> {
        let Some(receiver_type) = self.host_index_receiver_type_name(base, base_payload) else {
            return Ok(());
        };
        let Some(capability) = self.facts.options.host_index_capability(&receiver_type) else {
            return Err(host_index_diagnostic_error(
                Diagnostic::error(format!(
                    "type `{receiver_type}` does not support host index access"
                ))
                .with_code("analysis::host_index_not_supported")
                .with_span(expr.span)
                .with_label(
                    expr.span,
                    "host index access is not registered for this type",
                )
                .with_label(
                    base.span,
                    "register a host index capability or expose a field/method instead",
                ),
            ));
        };
        if !kind.allowed_by(capability) {
            return Err(host_index_diagnostic_error(
                Diagnostic::error(format!(
                    "type `{receiver_type}` does not allow host index {}",
                    kind.access_name()
                ))
                .with_code(kind.denied_code())
                .with_span(expr.span)
                .with_label(expr.span, kind.capability_label())
                .with_label(base.span, kind.enable_label()),
            ));
        }
        if let Some(expected) = capability.key_type.as_deref()
            && let Some(actual) = self.value_type_for_expr_with_payload(index, index_payload)
            && actual.source_type_name() != expected
            && actual.std_type_name() != expected
        {
            return Err(host_index_diagnostic_error(
                Diagnostic::error(format!(
                    "host index key for `{receiver_type}` must be `{expected}`"
                ))
                .with_code("analysis::host_index_key_mismatch")
                .with_span(expr.span)
                .with_label(
                    index.span,
                    format!("index expression has type `{}`", actual.source_type_name()),
                ),
            ));
        }
        Ok(())
    }

    pub(in crate::compiler) fn reject_invalid_host_index_read_with_payload(
        &self,
        expr: &Expr,
        base: &Expr,
        index: &Expr,
        base_payload: Option<&CompilerExpressionPayload<'_>>,
        index_payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> CompileResult<()> {
        self.reject_invalid_host_index_access_with_payload(
            expr,
            base,
            index,
            HostIndexAccessKind::Read,
            base_payload,
            index_payload,
        )
    }

    pub(in crate::compiler) fn reject_terminal_host_index_access(
        &self,
        expr: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
        kind: HostIndexAccessKind,
    ) -> CompileResult<()> {
        if let Some(payload) = payload {
            return match payload.syntax_kind() {
                Some(SyntaxExpressionKind::Index) => {
                    let ExprKind::Index { base, index } = &expr.kind else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "mismatched CST host index receiver payload",
                        )));
                    };
                    let Some((base_payload, index_payload)) = payload.index_operand_payloads()
                    else {
                        return Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "mismatched CST host index receiver payload",
                        )));
                    };
                    self.reject_invalid_host_index_access_with_payload(
                        expr,
                        base,
                        index,
                        kind,
                        Some(&base_payload),
                        Some(&index_payload),
                    )
                }
                Some(_) => Ok(()),
                None => {
                    let ExprKind::Index { base, index } = &expr.kind else {
                        return Ok(());
                    };
                    self.reject_invalid_host_index_access(expr, base, index, kind)
                }
            };
        }
        let ExprKind::Index { base, index } = &expr.kind else {
            return Ok(());
        };
        self.reject_invalid_host_index_access(expr, base, index, kind)
    }

    fn host_index_receiver_type_name(
        &self,
        receiver: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> Option<String> {
        self.resolve_host_path_index_receiver_with_payload(receiver, payload.cloned())
            .and_then(|resolved| resolved.type_name)
            .or_else(|| {
                let type_name = payload
                    .and_then(|payload| self.script_type_for_payload(payload))
                    .or_else(|| self.script_type_for_expr_with_payload(receiver, payload))?;
                self.host_runtime_type_id(&type_name).map(|_| type_name)
            })
    }
}

fn host_path_field_name(
    payload: Option<&CompilerExpressionPayload<'_>>,
    default_name: &str,
) -> Option<String> {
    match payload {
        Some(payload) => match payload.syntax_kind() {
            Some(SyntaxExpressionKind::Field) | None => payload.syntax_field_name(),
            Some(_) => None,
        },
        None => Some(default_name.to_owned()),
    }
}

fn host_index_diagnostic_error(diagnostic: Diagnostic) -> CompileError {
    CompileError::new(CompileErrorKind::SemanticDiagnostics(vec![diagnostic]))
}

pub(super) struct ResolvedHostPath<'ast> {
    pub(super) path: HostPath<'ast>,
    pub(super) type_name: Option<String>,
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HostIndexAccessKind {
    Read,
    Write,
    Mutate,
    Remove,
}

impl HostIndexAccessKind {
    fn allowed_by(self, capability: &crate::compiler::options::HostIndexCapabilityInfo) -> bool {
        match self {
            Self::Read => capability.readable,
            Self::Write => capability.writable,
            Self::Mutate => capability.addable,
            Self::Remove => capability.removable,
        }
    }

    const fn denied_code(self) -> &'static str {
        match self {
            Self::Read => "analysis::host_index_not_readable",
            Self::Write => "analysis::host_index_not_writable",
            Self::Mutate => "analysis::host_index_not_mutable",
            Self::Remove => "analysis::host_index_not_removable",
        }
    }

    const fn access_name(self) -> &'static str {
        match self {
            Self::Read => "reads",
            Self::Write => "writes",
            Self::Mutate => "mutations",
            Self::Remove => "removals",
        }
    }

    const fn capability_label(self) -> &'static str {
        match self {
            Self::Read => "host index capability is not readable",
            Self::Write => "host index capability is not writable",
            Self::Mutate => "host index capability is not addable",
            Self::Remove => "host index capability is not removable",
        }
    }

    const fn enable_label(self) -> &'static str {
        match self {
            Self::Read => "enable readable host index access for this type",
            Self::Write => "enable writable host index access for this type",
            Self::Mutate => "enable addable host index access for this type",
            Self::Remove => "enable removable host index access for this type",
        }
    }
}

fn const_host_path_arg_with_payload(
    payload: Option<&CompilerExpressionPayload<'_>>,
) -> Option<ConstHostPathArg> {
    payload
        .and_then(CompilerExpressionPayload::syntax_literal)
        .and_then(|literal| const_host_path_arg_from_literal(&literal))
}

fn const_host_path_arg_from_literal(literal: &Literal) -> Option<ConstHostPathArg> {
    match literal {
        Literal::Integer(value) if value.suffix.is_none() => value
            .source_text()
            .parse::<u32>()
            .ok()
            .map(ConstHostPathArg::Index),
        Literal::String(value) => Some(ConstHostPathArg::Key(value.clone())),
        _ => None,
    }
}

fn dynamic_host_path_part(key_type: &str) -> DynamicHostPathPart {
    match key_type {
        "i64" => DynamicHostPathPart::Index,
        _ => DynamicHostPathPart::Key,
    }
}

fn syntax_host_expression_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}
