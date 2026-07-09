use super::{CompileError, CompileErrorKind, CompileResult, Compiler};
use crate::{CacheSiteId, HostTargetPlanId, Register, UnlinkedInstructionKind};
use vela_common::HostTypeId;
use vela_common::SourceId;
use vela_common::Span;
use vela_def::FieldId;
use vela_host::resolved::HostMutationOp;
use vela_host::target::HostTargetPlan;
use vela_syntax::ast::{AstNode, SyntaxExpression};
pub(super) struct HostPath<'ast> {
    pub(super) root: HostPathRoot<'ast>,
    pub(super) segments: Vec<HostPathPart<'ast>>,
}
#[derive(Clone)]
pub(super) enum HostPathRoot<'ast> {
    LocalPath { name: &'ast str, span: Span },
    OwnedLocalPath { name: String, span: Span },
}
pub(super) enum HostPathPart<'ast> {
    Field(FieldId),
    VariantField(FieldId),
    SyntaxValue {
        source: SourceId,
        expression: SyntaxExpression,
        dynamic_kind: DynamicHostPathPart,
        _ast: std::marker::PhantomData<&'ast ()>,
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
    fn host_path_field_part(
        &self,
        receiver_type: Option<&str>,
        name: &str,
    ) -> Option<ResolvedHostPathField> {
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
    pub(super) fn compile_host_path_root<'expr>(
        &mut self,
        root: &HostPathRoot<'expr>,
    ) -> CompileResult<Register> {
        match root {
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
                HostPathPart::SyntaxValue {
                    source,
                    expression,
                    dynamic_kind,
                    ..
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
            HostPathRoot::LocalPath { name, span } => self.host_local_type_name(name, span),
            HostPathRoot::OwnedLocalPath { name, span } => self.host_local_type_name(&name, span),
        }
    }
    pub(super) fn host_local_type_name(&self, name: &str, span: Span) -> Option<String> {
        let expression = self.expression_at_span(span);
        expression
            .and_then(|expression| self.local_for_expression(expression))
            .and_then(|local| self.script_types.local(local))
            .or_else(|| {
                expression.and_then(|expression| self.global_type_for_expression(expression))
            })
            .or_else(|| self.script_types.name(name))
            .or_else(|| self.global_type_named(name))
    }
    pub(in crate::compiler) fn syntax_root_host_index_path(
        &self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> Option<HostPath<'static>> {
        self.syntax_host_index_path(source, expression)
            .map(|resolved| resolved.path)
    }
    fn syntax_host_index_path(
        &self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> Option<ResolvedHostPath<'static>> {
        let index = expression.as_index()?;
        let receiver = index.receiver()?;
        let index_expression = index.index()?;
        let mut resolved = self.syntax_host_path(source, &receiver)?;
        if resolved.path.segments.is_empty()
            && resolved.type_name.as_deref().is_none_or(|type_name| {
                self.facts
                    .options
                    .host_index_capability(type_name)
                    .is_none()
            })
        {
            return None;
        }
        let receiver_type = resolved.type_name.clone();
        let dynamic_kind = self.syntax_host_index_dynamic_kind(receiver_type.as_deref());
        resolved.path.segments.push(HostPathPart::SyntaxValue {
            source,
            expression: index_expression,
            dynamic_kind,
            _ast: std::marker::PhantomData,
        });
        resolved.type_name = self.syntax_host_index_value_type(receiver_type.as_deref());
        Some(resolved)
    }
    fn syntax_host_index_dynamic_kind(&self, receiver_type: Option<&str>) -> DynamicHostPathPart {
        receiver_type
            .and_then(|type_name| self.facts.options.host_index_capability(type_name))
            .and_then(|capability| capability.key_type.as_deref())
            .map_or(DynamicHostPathPart::Key, dynamic_host_path_part)
    }
    fn syntax_host_index_value_type(&self, receiver_type: Option<&str>) -> Option<String> {
        receiver_type.and_then(|type_name| {
            self.facts
                .options
                .host_index_capability(type_name)
                .and_then(|capability| capability.value_type.clone())
        })
    }
    fn syntax_host_path(
        &self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> Option<ResolvedHostPath<'static>> {
        if let Some(inner) = expression.as_paren().and_then(|paren| paren.expression()) {
            return self.syntax_host_path(source, &inner);
        }
        if expression.as_index().is_some() {
            return self.syntax_host_index_path(source, expression);
        }
        let span = syntax_host_expression_span(source, expression);
        if let Some(resolved) = self.hir_host_value_path(span) {
            return Some(resolved);
        }
        let field = expression.as_field()?;
        let receiver = field.receiver()?;
        let name = field.name_text()?;
        let mut resolved = self.syntax_host_path(source, &receiver)?;
        let field = self.host_path_field_part(resolved.type_name.as_deref(), &name)?;
        resolved.path.segments.push(field.part);
        resolved.type_name = field.type_hint;
        Some(resolved)
    }
    fn hir_host_value_path(&self, span: Span) -> Option<ResolvedHostPath<'static>> {
        let path = self.hir_value_path_for_span(span)?;
        if path.len() == 1 {
            let name = path.into_iter().next()?;
            let root_span = self.hir_value_path_root_span_for_span(span).unwrap_or(span);
            let type_name = self.host_local_type_name(&name, root_span);
            return Some(ResolvedHostPath {
                path: HostPath {
                    root: HostPathRoot::OwnedLocalPath {
                        name,
                        span: root_span,
                    },
                    segments: Vec::new(),
                },
                type_name,
            });
        }
        let root_span = self.hir_value_path_root_span_for_span(span).unwrap_or(span);
        self.owned_host_field_path_parts(root_span, &path)
    }
    pub(in crate::compiler) fn syntax_host_field_path(
        &self,
        source: SourceId,
        expression: &SyntaxExpression,
    ) -> Option<ResolvedHostPath<'static>> {
        self.syntax_host_path(source, expression)
    }
}
pub(super) struct ResolvedHostPath<'ast> {
    pub(super) path: HostPath<'ast>,
    pub(super) type_name: Option<String>,
}
struct ResolvedHostPathField {
    part: HostPathPart<'static>,
    type_hint: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompiledHostTarget {
    pub(super) target: HostTargetPlanId,
    pub(super) dynamic_args: Vec<Register>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HostIndexAccessKind {
    Read,
    Write,
    Mutate,
    Remove,
}
impl HostIndexAccessKind {
    pub(super) fn allowed_by(
        self,
        capability: &crate::compiler::options::HostIndexCapabilityInfo,
    ) -> bool {
        match self {
            Self::Read => capability.readable,
            Self::Write => capability.writable,
            Self::Mutate => capability.addable,
            Self::Remove => capability.removable,
        }
    }
    pub(super) const fn denied_code(self) -> &'static str {
        match self {
            Self::Read => "analysis::host_index_not_readable",
            Self::Write => "analysis::host_index_not_writable",
            Self::Mutate => "analysis::host_index_not_mutable",
            Self::Remove => "analysis::host_index_not_removable",
        }
    }
    pub(super) const fn access_name(self) -> &'static str {
        match self {
            Self::Read => "reads",
            Self::Write => "writes",
            Self::Mutate => "mutations",
            Self::Remove => "removals",
        }
    }
    pub(super) const fn capability_label(self) -> &'static str {
        match self {
            Self::Read => "host index capability is not readable",
            Self::Write => "host index capability is not writable",
            Self::Mutate => "host index capability is not addable",
            Self::Remove => "host index capability is not removable",
        }
    }
    pub(super) const fn enable_label(self) -> &'static str {
        match self {
            Self::Read => "enable readable host index access for this type",
            Self::Write => "enable writable host index access for this type",
            Self::Mutate => "enable addable host index access for this type",
            Self::Remove => "enable removable host index access for this type",
        }
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
