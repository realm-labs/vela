use vela_common::Span;
use vela_common::{Diagnostic, HostTypeId};
use vela_def::FieldId;
use vela_syntax::ast::{Argument, Expr, ExprKind};

use crate::{CacheSiteId, Constant, HostTargetPlanId, Register, UnlinkedInstructionKind};
use vela_host::resolved::HostMutationOp;
use vela_host::target::HostTargetPlan;

use super::body_payloads::CompilerExpressionPayload;
use super::call_args::CallArgumentSyntax;
use super::{CompileError, CompileErrorKind, CompileResult, Compiler, reject_named_args};

pub(super) struct HostPath<'ast> {
    pub(super) root: HostPathRoot<'ast>,
    pub(super) segments: Vec<HostPathPart<'ast>>,
}

#[derive(Clone, Copy)]
pub(super) enum HostPathRoot<'ast> {
    Expr(&'ast Expr),
    LocalPath { name: &'ast str, span: Span },
}

pub(super) enum HostPathPart<'ast> {
    Field(FieldId),
    VariantField(FieldId),
    Value {
        expr: &'ast Expr,
        payload: Option<CompilerExpressionPayload<'ast>>,
        dynamic_kind: DynamicHostPathPart,
    },
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
        let mut path = self.host_field_path(expr)?;
        attach_dynamic_host_path_payloads(expr, payload.cloned(), &mut path.segments, &mut 0);
        Some(path)
    }

    pub(super) fn host_field_path_with_index_payloads<'ast>(
        &self,
        expr: &'ast Expr,
        base_payload: Option<&CompilerExpressionPayload<'ast>>,
        index_payload: Option<&CompilerExpressionPayload<'ast>>,
    ) -> Option<HostPath<'ast>> {
        let mut path = self.host_field_path(expr)?;
        let ExprKind::Index { base, index } = &expr.kind else {
            return Some(path);
        };
        let mut next_segment = 0;
        attach_dynamic_host_path_payloads(
            base,
            base_payload.cloned(),
            &mut path.segments,
            &mut next_segment,
        );
        attach_next_dynamic_host_path_payload(
            index.as_ref(),
            index_payload.cloned(),
            &mut path.segments,
            &mut next_segment,
        );
        Some(path)
    }

    pub(super) fn host_field_path_with_field_base_payload<'ast>(
        &self,
        expr: &'ast Expr,
        base_payload: Option<&CompilerExpressionPayload<'ast>>,
    ) -> Option<HostPath<'ast>> {
        let mut path = self.host_field_path(expr)?;
        let ExprKind::Field { base, .. } = &expr.kind else {
            return Some(path);
        };
        attach_dynamic_host_path_payloads(base, base_payload.cloned(), &mut path.segments, &mut 0);
        Some(path)
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
            ExprKind::Path(path) => self
                .host_field_path_parts(receiver.span, path)
                .or_else(|| self.host_index_root_path(receiver.span, path)),
            _ => None,
        }
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
        args: &[Argument],
        arg_syntax: CallArgumentSyntax<'_, '_>,
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
        let value = self.compile_call_argument_value(arg, arg_syntax)?;
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
        if let ExprKind::Field { base, name } = &callee.kind
            && name == "remove"
        {
            self.reject_terminal_host_index_access(base, HostIndexAccessKind::Remove)?;
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
                HostPathPart::Value {
                    expr,
                    payload,
                    dynamic_kind,
                } => {
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
                    let register = self.compile_expr_with_payload(expr, payload.as_ref())?;
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

    pub(super) fn reject_invalid_host_index_access(
        &self,
        expr: &Expr,
        base: &Expr,
        index: &Expr,
        kind: HostIndexAccessKind,
    ) -> CompileResult<()> {
        let Some(receiver_type) = self.host_index_receiver_type_name(base) else {
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
            && let Some(actual) = self.value_type_for_expr(index)
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

    fn reject_terminal_host_index_access(
        &self,
        expr: &Expr,
        kind: HostIndexAccessKind,
    ) -> CompileResult<()> {
        let ExprKind::Index { base, index } = &expr.kind else {
            return Ok(());
        };
        self.reject_invalid_host_index_access(expr, base, index, kind)
    }

    fn host_index_receiver_type_name(&self, receiver: &Expr) -> Option<String> {
        self.resolve_host_path_index_receiver(receiver)
            .and_then(|resolved| resolved.type_name)
            .or_else(|| {
                let type_name = self.script_type_for_expr(receiver)?;
                self.host_runtime_type_id(&type_name).map(|_| type_name)
            })
    }
}

fn attach_dynamic_host_path_payloads<'ast>(
    expr: &'ast Expr,
    payload: Option<CompilerExpressionPayload<'ast>>,
    segments: &mut [HostPathPart<'ast>],
    next_segment: &mut usize,
) {
    match &expr.kind {
        ExprKind::Field { base, .. } => {
            attach_dynamic_host_path_payloads(
                base,
                payload.and_then(|payload| payload.field_base_payload()),
                segments,
                next_segment,
            );
        }
        ExprKind::Index { base, index } => {
            let operands = payload.and_then(|payload| payload.index_operand_payloads());
            let (base_payload, index_payload) =
                operands.map_or((None, None), |(base, index)| (Some(base), Some(index)));
            attach_dynamic_host_path_payloads(base, base_payload, segments, next_segment);
            attach_next_dynamic_host_path_payload(
                index.as_ref(),
                index_payload,
                segments,
                next_segment,
            );
        }
        _ => {}
    }
}

fn attach_next_dynamic_host_path_payload<'ast>(
    index: &'ast Expr,
    index_payload: Option<CompilerExpressionPayload<'ast>>,
    segments: &mut [HostPathPart<'ast>],
    next_segment: &mut usize,
) {
    while let Some(segment) = segments.get_mut(*next_segment) {
        *next_segment += 1;
        if let HostPathPart::Value {
            expr: segment_expr,
            payload,
            ..
        } = segment
            && std::ptr::eq::<Expr>(*segment_expr, index)
        {
            *payload = index_payload;
            break;
        }
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

fn const_host_path_arg(expr: &Expr) -> Option<ConstHostPathArg> {
    match &expr.kind {
        ExprKind::Literal(vela_syntax::ast::Literal::Integer(value)) if value.suffix.is_none() => {
            value
                .source_text()
                .parse::<u32>()
                .ok()
                .map(ConstHostPathArg::Index)
        }
        ExprKind::Literal(vela_syntax::ast::Literal::String(value)) => {
            Some(ConstHostPathArg::Key(value.clone()))
        }
        _ => None,
    }
}

fn dynamic_host_path_part(key_type: &str) -> DynamicHostPathPart {
    match key_type {
        "i64" => DynamicHostPathPart::Index,
        _ => DynamicHostPathPart::Key,
    }
}
