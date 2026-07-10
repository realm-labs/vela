use super::{CompileError, CompileErrorKind, CompileResult, Compiler};
use crate::{CacheSiteId, HostTargetPlanId, Register, UnlinkedInstructionKind};
use vela_common::HostTypeId;
use vela_common::Span;
use vela_def::FieldId;
use vela_hir::body::HirExprKind;
use vela_hir::ids::HirExprId;
use vela_host::resolved::HostMutationOp;
use vela_host::target::HostTargetPlan;
pub(super) struct HostPath {
    pub(super) root: HostPathRoot,
    pub(super) segments: Vec<HostPathPart>,
}
#[derive(Clone)]
pub(super) enum HostPathRoot {
    LocalPath {
        name: String,
        expression: HirExprId,
        span: Span,
    },
}
pub(super) enum HostPathPart {
    Field(FieldId),
    VariantField(FieldId),
    DynamicValue {
        expression: HirExprId,
        dynamic_kind: DynamicHostPathPart,
    },
}
#[derive(Clone, Copy)]
pub(super) enum DynamicHostPathPart {
    Index,
    Key,
}
impl HostPath {
    pub(super) fn requires_path_instruction(&self) -> bool {
        !matches!(self.segments.as_slice(), [HostPathPart::Field(_)])
    }
}
impl Compiler<'_, '_> {
    pub(super) fn host_field_path_parts(
        &self,
        expression: HirExprId,
        span: Span,
        path: &[String],
    ) -> Option<ResolvedHostPath> {
        if path.len() < 2 {
            return None;
        }
        let root = path.first()?;
        let root_expression = self
            .hir_value_path_root_expression(expression)
            .unwrap_or(expression);
        let root_span = self.expression_span(root_expression).unwrap_or(span);
        let mut current_type = self.host_local_type_name(root, root_expression);
        let mut segments = Vec::with_capacity(path.len() - 1);
        for segment in &path[1..] {
            let field = self.host_path_field_part(current_type.as_deref(), segment)?;
            segments.push(field.part);
            current_type = field.type_hint;
        }
        Some(ResolvedHostPath {
            path: HostPath {
                root: HostPathRoot::LocalPath {
                    name: root.clone(),
                    expression: root_expression,
                    span: root_span,
                },
                segments,
            },
            type_name: current_type,
        })
    }
    pub(super) fn owned_host_field_path_parts(
        &self,
        expression: HirExprId,
        span: Span,
        path: &[String],
    ) -> Option<ResolvedHostPath> {
        if path.len() < 2 {
            return None;
        }
        let root = path.first()?.clone();
        let root_expression = self
            .hir_value_path_root_expression(expression)
            .unwrap_or(expression);
        let root_span = self.expression_span(root_expression).unwrap_or(span);
        let mut current_type = self.host_local_type_name(&root, root_expression);
        let mut segments = Vec::with_capacity(path.len() - 1);
        for segment in &path[1..] {
            let field = self.host_path_field_part(current_type.as_deref(), segment)?;
            segments.push(field.part);
            current_type = field.type_hint;
        }
        Some(ResolvedHostPath {
            path: HostPath {
                root: HostPathRoot::LocalPath {
                    name: root,
                    expression: root_expression,
                    span: root_span,
                },
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
        path: HostPath,
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
    pub(super) fn emit_compiled_host_write(
        &mut self,
        root: Register,
        target: CompiledHostTarget,
        src: Register,
        span: Span,
    ) {
        self.emit_spanned(
            UnlinkedInstructionKind::HostWrite {
                root,
                target: target.target,
                dynamic_args: target.dynamic_args,
                src,
                cache_site: CacheSiteId::new(0),
            },
            span,
        );
    }
    pub(super) fn emit_host_mutate(
        &mut self,
        root: Register,
        path: HostPath,
        op: HostMutationOp,
        rhs: Register,
        span: Span,
    ) -> CompileResult<()> {
        let target = self.compile_host_target(path)?;
        self.emit_compiled_host_mutate(root, target, op, rhs, span);
        Ok(())
    }
    pub(super) fn emit_compiled_host_mutate(
        &mut self,
        root: Register,
        target: CompiledHostTarget,
        op: HostMutationOp,
        rhs: Register,
        span: Span,
    ) {
        self.emit_spanned(
            UnlinkedInstructionKind::HostMutate {
                root,
                target: target.target,
                dynamic_args: target.dynamic_args,
                op,
                rhs,
                cache_site: CacheSiteId::new(0),
            },
            span,
        );
    }
    pub(super) fn emit_host_remove(
        &mut self,
        root: Register,
        path: HostPath,
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
        path: HostPath,
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
    pub(super) fn compile_host_path_root(
        &mut self,
        root: &HostPathRoot,
    ) -> CompileResult<Register> {
        match root {
            HostPathRoot::LocalPath {
                name,
                expression,
                span,
            } => self.required_local_register_for_hir_expression(*expression, *span, name),
        }
    }
    pub(super) fn compile_host_target(
        &mut self,
        path: HostPath,
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
                HostPathPart::DynamicValue {
                    expression,
                    dynamic_kind,
                } => {
                    let arg = u8::try_from(dynamic_args.len()).map_err(|_| {
                        CompileError::new(CompileErrorKind::UnsupportedSyntax(
                            "host path dynamic argument count",
                        ))
                    })?;
                    let register = self.compile_hir_expression(expression)?;
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
    fn host_path_root_type(&self, root: HostPathRoot) -> HostTypeId {
        self.host_path_root_type_name(root)
            .and_then(|type_name| self.host_runtime_type_id(&type_name))
            .unwrap_or_else(|| HostTypeId::new(0))
    }
    fn host_path_root_type_name(&self, root: HostPathRoot) -> Option<String> {
        match root {
            HostPathRoot::LocalPath {
                name, expression, ..
            } => self.host_local_type_name(&name, expression),
        }
    }
    pub(super) fn host_local_type_name(&self, name: &str, expression: HirExprId) -> Option<String> {
        self.local_for_expression(expression)
            .and_then(|local| self.script_types.local(local))
            .or_else(|| self.global_type_for_expression(expression))
            .or_else(|| self.script_types.name(name))
            .or_else(|| self.global_type_named(name))
    }
    fn hir_host_index_path(&self, expression: HirExprId) -> Option<ResolvedHostPath> {
        let index = self.hir_index_for_expression(expression)?;
        let mut resolved = self.hir_host_path(index.receiver)?;
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
        let dynamic_kind = self.host_index_dynamic_kind(receiver_type.as_deref());
        resolved.path.segments.push(HostPathPart::DynamicValue {
            expression: index.index,
            dynamic_kind,
        });
        resolved.type_name = self.host_index_value_type(receiver_type.as_deref());
        Some(resolved)
    }
    fn host_index_dynamic_kind(&self, receiver_type: Option<&str>) -> DynamicHostPathPart {
        receiver_type
            .and_then(|type_name| self.facts.options.host_index_capability(type_name))
            .and_then(|capability| capability.key_type.as_deref())
            .map_or(DynamicHostPathPart::Key, dynamic_host_path_part)
    }
    fn host_index_value_type(&self, receiver_type: Option<&str>) -> Option<String> {
        receiver_type.and_then(|type_name| {
            self.facts
                .options
                .host_index_capability(type_name)
                .and_then(|capability| capability.value_type.clone())
        })
    }
    pub(in crate::compiler) fn hir_host_path(
        &self,
        expression: HirExprId,
    ) -> Option<ResolvedHostPath> {
        let kind = self
            .hir_bodies
            .iter()
            .find_map(|body| body.expression(expression))?
            .kind
            .clone();
        match kind {
            HirExprKind::Paren {
                expression: Some(inner),
            } => self.hir_host_path(inner),
            HirExprKind::Index(_) => self.hir_host_index_path(expression),
            HirExprKind::Field(field) => {
                let mut resolved = self.hir_host_path(field.receiver)?;
                let field_part =
                    self.host_path_field_part(resolved.type_name.as_deref(), &field.name)?;
                resolved.path.segments.push(field_part.part);
                resolved.type_name = field_part.type_hint;
                Some(resolved)
            }
            _ => self.hir_host_value_path(expression),
        }
    }

    fn hir_host_value_path(&self, expression: HirExprId) -> Option<ResolvedHostPath> {
        let path = self.hir_value_path_for_expression(expression)?;
        let span = self.expression_span(expression)?;
        let root_expression = self
            .hir_value_path_root_expression(expression)
            .unwrap_or(expression);
        let root_span = self.expression_span(root_expression).unwrap_or(span);
        if path.len() == 1 {
            let name = path.into_iter().next()?;
            let type_name = self.host_local_type_name(&name, root_expression);
            return Some(ResolvedHostPath {
                path: HostPath {
                    root: HostPathRoot::LocalPath {
                        name,
                        expression: root_expression,
                        span: root_span,
                    },
                    segments: Vec::new(),
                },
                type_name,
            });
        }
        self.owned_host_field_path_parts(root_expression, span, &path)
    }
}
pub(super) struct ResolvedHostPath {
    pub(super) path: HostPath,
    pub(super) type_name: Option<String>,
}
struct ResolvedHostPathField {
    part: HostPathPart,
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
