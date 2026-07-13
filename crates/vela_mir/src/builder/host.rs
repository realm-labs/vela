use vela_analysis::literals::ResolvedLiteralFact;
use vela_hir::body::{HirAssignOp, HirCall, HirExprKind, HirLiteral};
use vela_hir::ids::HirExprId;

use crate::{
    CompileCallArguments, CompileCallTarget, CompileCalleeTarget, CompileHostPathSegment,
    CompileHostPathTarget, CompileMemberTarget, CompileMethodClass, CompileTypeClass,
    HostMethodTarget, HostTypeTarget, MirBuildError, MirEffect, MirHostMutation, MirHostOperation,
    MirHostPath, MirHostPathSegment, MirImmediate, MirOperand, MirPlace, MirSafepoint,
    MirSourceOrigin, MirStatement, MirStatementKind, MirTypeContract, MirValueType,
};

use super::core::{FunctionBuilder, value_type};

enum PreparedHostPath {
    Ready { root: MirOperand, path: MirHostPath },
    Diverged,
}

#[derive(Clone, Debug)]
pub(super) struct PreparedHostValueWriteback {
    root: MirOperand,
    path: MirHostPath,
}

impl FunctionBuilder<'_> {
    /// Read a host-owned value that will be rebuilt by ordinary MIR and then
    /// written back through the exact same HostAccess prefix.
    ///
    /// This is used for immutable tuple projections below a host path. The
    /// prefix remains an explicit HostAccess operation and never becomes a
    /// MIR place or a dereferenced host reference.
    pub(super) fn prepare_host_value_writeback(
        &mut self,
        expression: HirExprId,
        target: &CompileHostPathTarget,
        origin: MirSourceOrigin,
    ) -> Result<Option<(MirOperand, PreparedHostValueWriteback)>, MirBuildError> {
        if target.segments.is_empty() {
            return Err(self.inconsistent(
                origin,
                "host tuple assignment prefix has an empty HostAccess path",
            ));
        }
        self.validate_host_read_target(expression, target, origin)?;
        self.validate_host_value_writeback_access(target, origin)?;
        let PreparedHostPath::Ready { root, path } = self.prepare_host_path(target)? else {
            return Ok(None);
        };
        let result_type = self.host_read_result_type(expression, target, origin)?;
        let value = self.append_host_value(
            origin,
            result_type,
            MirHostOperation::Read {
                root: root.clone(),
                path: path.clone(),
            },
            MirEffect::host_read(),
        )?;
        Ok(Some((value, PreparedHostValueWriteback { root, path })))
    }

    pub(super) fn write_host_value_back(
        &mut self,
        target: PreparedHostValueWriteback,
        value: MirOperand,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        self.append_host_effect(
            origin,
            MirHostOperation::Write {
                root: target.root,
                path: target.path,
                value,
            },
            MirEffect::host_write(),
        )
    }

    /// Lower an exact host field or index read as one call-scoped HostAccess
    /// operation. Path prefixes are target-plan structure, not independently
    /// evaluated reads, and no host value becomes a MIR place.
    pub(super) fn lower_host_read(
        &mut self,
        expression: HirExprId,
        target: &CompileHostPathTarget,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        if target.segments.is_empty() {
            return Err(
                self.inconsistent(origin, "host field/index read has an empty HostAccess path")
            );
        }
        self.validate_host_read_target(expression, target, origin)?;
        let PreparedHostPath::Ready { root, path } = self.prepare_host_path(target)? else {
            return Ok(unit());
        };
        let result_type = self.host_read_result_type(expression, target, origin)?;
        self.append_host_value(
            origin,
            result_type,
            MirHostOperation::Read { root, path },
            MirEffect::host_read(),
        )
    }

    /// Lower a host write or adapter-defined scalar mutation. The root and
    /// every dynamic path argument are captured before the RHS is evaluated;
    /// aliases therefore observe the same read-modify-write order as the
    /// source program without exposing a HostRef dereference place.
    pub(super) fn lower_host_assignment(
        &mut self,
        _expression: HirExprId,
        operation: HirAssignOp,
        target_expression: HirExprId,
        value_expression: HirExprId,
        target: &CompileHostPathTarget,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        if target.segments.is_empty() {
            return Err(self.inconsistent(origin, "host assignment has an empty HostAccess path"));
        }
        let target_placement = self
            .input
            .targets()
            .host_path(target_expression)
            .ok_or_else(|| {
                self.inconsistent(
                    origin,
                    "host assignment target has no exact host-path placement",
                )
            })?;
        if target_placement != target {
            return Err(self.inconsistent(
                origin,
                "host assignment placement disagrees with its HIR target path",
            ));
        }
        self.validate_host_path_expression(target_expression, target, origin)?;

        let PreparedHostPath::Ready { root, path } = self.prepare_host_path(target)? else {
            return Ok(unit());
        };
        let value_origin = self.host_expression_origin(value_expression)?;
        let value = self.lower_expression(value_expression)?;
        if self.current_is_terminated()? {
            return Ok(unit());
        }
        let value = self.capture_operand(value, value_origin)?;
        let host = match operation {
            HirAssignOp::Set => MirHostOperation::Write {
                root,
                path,
                value: value.clone(),
            },
            HirAssignOp::Add
            | HirAssignOp::Sub
            | HirAssignOp::Mul
            | HirAssignOp::Div
            | HirAssignOp::Rem => MirHostOperation::Mutate {
                root,
                path,
                operation: host_mutation(operation),
                value: value.clone(),
            },
        };
        self.append_host_effect(origin, host, MirEffect::host_write())?;

        // The existing language contract returns the evaluated RHS from an
        // assignment expression. HostAccess performs its mutation immediately
        // and does not synthesize a host read of the new value.
        Ok(value)
    }

    /// Lower host methods and the adapter-defined `remove`/`push` intrinsics.
    /// Receiver root/path arguments are evaluated before source call arguments
    /// and all operands are captured before crossing the host boundary.
    pub(super) fn lower_host_call(
        &mut self,
        expression: HirExprId,
        call: &HirCall,
        placed: &CompileCallTarget,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let field = self.host_call_field(expression, call, origin)?;
        match &placed.callee {
            CompileCalleeTarget::HostMethod(method) => {
                let path = self.host_call_path(field.receiver, None, origin)?;
                self.validate_host_method(&field.name, method, &path, origin)?;
                let PreparedHostPath::Ready {
                    root,
                    path: mir_path,
                } = self.prepare_host_path(&path)?
                else {
                    return Ok(unit());
                };
                let arguments =
                    self.lower_external_arguments(&placed.arguments, &method.signature, origin)?;
                if self.current_is_terminated()? {
                    return Ok(unit());
                }
                let result_type = self
                    .host_call_result_type(expression, method.signature.return_contract.as_ref());
                self.append_host_value(
                    origin,
                    result_type,
                    MirHostOperation::Call {
                        root,
                        path: mir_path,
                        target: Box::new(method.clone()),
                        arguments,
                    },
                    MirEffect::host_call().union(method.signature.effect),
                )
            }
            CompileCalleeTarget::HostRemove { path } => {
                if field.name != "remove"
                    || !matches!(
                        self.body
                            .expression(field.receiver)
                            .map(|value| &value.kind),
                        Some(HirExprKind::Index(_))
                    )
                {
                    return Err(self
                        .inconsistent(origin, "host remove target disagrees with the HIR callee"));
                }
                self.require_host_positional_arity(&placed.arguments, 0, origin, "remove")?;
                let path = self.host_call_path(field.receiver, Some(path), origin)?;
                let PreparedHostPath::Ready { root, path } = self.prepare_host_path(&path)? else {
                    return Ok(unit());
                };
                self.append_host_effect(
                    origin,
                    MirHostOperation::Remove { root, path },
                    MirEffect::host_write(),
                )?;
                Ok(unit())
            }
            CompileCalleeTarget::HostPush { path } => {
                if field.name != "push" || path.segments.is_empty() {
                    return Err(
                        self.inconsistent(origin, "host push target disagrees with the HIR callee")
                    );
                }
                let values =
                    self.require_host_positional_arity(&placed.arguments, 1, origin, "push")?;
                let path = self.host_call_path(field.receiver, Some(path), origin)?;
                let PreparedHostPath::Ready { root, path } = self.prepare_host_path(&path)? else {
                    return Ok(unit());
                };
                let value_expression = values[0];
                let value_origin = self.host_expression_origin(value_expression)?;
                let value = self.lower_expression(value_expression)?;
                if self.current_is_terminated()? {
                    return Ok(unit());
                }
                let value = self.capture_operand(value, value_origin)?;
                self.append_host_effect(
                    origin,
                    MirHostOperation::Mutate {
                        root,
                        path,
                        operation: MirHostMutation::Push,
                        value,
                    },
                    MirEffect::host_write(),
                )?;
                Ok(unit())
            }
            CompileCalleeTarget::ScriptFunction { .. }
            | CompileCalleeTarget::ScriptMethod { .. }
            | CompileCalleeTarget::Local(_)
            | CompileCalleeTarget::Lambda(_)
            | CompileCalleeTarget::NativeFunction { .. }
            | CompileCalleeTarget::StdlibFunction { .. }
            | CompileCalleeTarget::ValueMethod { .. }
            | CompileCalleeTarget::Reflection { .. }
            | CompileCalleeTarget::SetFromArray { .. }
            | CompileCalleeTarget::DynamicCallable
            | CompileCalleeTarget::DynamicMethod(_) => {
                Err(self.inconsistent(origin, "non-host call target reached HostAccess lowering"))
            }
        }
    }

    fn prepare_host_path(
        &mut self,
        target: &CompileHostPathTarget,
    ) -> Result<PreparedHostPath, MirBuildError> {
        let root_origin = self.host_expression_origin(target.root)?;
        let root = self.lower_expression(target.root)?;
        if self.current_is_terminated()? {
            return Ok(PreparedHostPath::Diverged);
        }
        let root = self.capture_operand(root, root_origin)?;
        let mut segments = Vec::with_capacity(target.segments.len());
        for segment in &target.segments {
            let segment = match segment {
                CompileHostPathSegment::Field(field) => MirHostPathSegment::Field(field.clone()),
                CompileHostPathSegment::ConstantIndex { value, capability } => {
                    MirHostPathSegment::ConstantIndex {
                        value: *value,
                        capability: capability.clone(),
                    }
                }
                CompileHostPathSegment::ConstantKey { value, capability } => {
                    MirHostPathSegment::ConstantKey {
                        value: value.clone(),
                        capability: capability.clone(),
                    }
                }
                CompileHostPathSegment::DynamicIndex {
                    expression,
                    capability,
                } => {
                    let value = self.lower_host_path_argument(*expression)?;
                    let Some(value) = value else {
                        return Ok(PreparedHostPath::Diverged);
                    };
                    MirHostPathSegment::Index {
                        value,
                        capability: capability.clone(),
                    }
                }
                CompileHostPathSegment::DynamicKey {
                    expression,
                    capability,
                } => {
                    let value = self.lower_host_path_argument(*expression)?;
                    let Some(value) = value else {
                        return Ok(PreparedHostPath::Diverged);
                    };
                    MirHostPathSegment::Key {
                        value,
                        capability: capability.clone(),
                    }
                }
                CompileHostPathSegment::VariantField(field) => {
                    MirHostPathSegment::VariantField(field.clone())
                }
            };
            segments.push(segment);
        }
        Ok(PreparedHostPath::Ready {
            root,
            path: MirHostPath {
                root_type: target.root_type,
                segments,
            },
        })
    }

    fn lower_host_path_argument(
        &mut self,
        expression: HirExprId,
    ) -> Result<Option<MirOperand>, MirBuildError> {
        let origin = self.host_expression_origin(expression)?;
        let value = self.lower_expression(expression)?;
        if self.current_is_terminated()? {
            return Ok(None);
        }
        self.capture_operand(value, origin).map(Some)
    }

    fn validate_host_read_target(
        &self,
        expression: HirExprId,
        target: &CompileHostPathTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        self.validate_host_path_expression(expression, target, origin)
    }

    fn validate_host_value_writeback_access(
        &self,
        target: &CompileHostPathTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let (readable, writable) = match target.segments.last() {
            Some(CompileHostPathSegment::Field(field)) => {
                (field.access.readable, field.access.writable)
            }
            Some(CompileHostPathSegment::VariantField(field)) => (field.access.readable, true),
            Some(CompileHostPathSegment::ConstantIndex { capability, .. })
            | Some(CompileHostPathSegment::ConstantKey { capability, .. })
            | Some(CompileHostPathSegment::DynamicIndex { capability, .. })
            | Some(CompileHostPathSegment::DynamicKey { capability, .. }) => {
                (capability.readable, capability.writable)
            }
            None => (false, false),
        };
        if !readable {
            return Err(self.inconsistent(origin, "host tuple assignment prefix is not readable"));
        }
        if !writable {
            return Err(self.inconsistent(origin, "host tuple assignment prefix is not writable"));
        }
        Ok(())
    }

    fn validate_host_path_expression(
        &self,
        expression: HirExprId,
        target: &CompileHostPathTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let record = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(origin, "host path references a missing HIR expression")
        })?;
        match &record.kind {
            HirExprKind::Paren {
                expression: Some(inner),
            } => {
                let inner_target = self.input.targets().host_path(*inner).ok_or_else(|| {
                    self.inconsistent(origin, "parenthesized host path has no inner placement")
                })?;
                if inner_target != target {
                    return Err(self.inconsistent(
                        origin,
                        "parenthesized host path disagrees with its inner placement",
                    ));
                }
                self.validate_host_path_expression(*inner, inner_target, origin)
            }
            HirExprKind::Paren { expression: None } => {
                Err(self.inconsistent(origin, "host path is an empty parenthesized expression"))
            }
            HirExprKind::Path(_) => {
                if target.root == expression && target.segments.is_empty() {
                    Ok(())
                } else {
                    Err(self.inconsistent(
                        origin,
                        "host root placement disagrees with its HIR path expression",
                    ))
                }
            }
            HirExprKind::Field(field) => {
                self.validate_host_path_prefix(field.receiver, target, origin)?;
                let Some(CompileMemberTarget::HostField(member)) =
                    self.input.targets().member(expression)
                else {
                    return Err(self
                        .inconsistent(origin, "host field path has no exact host member target"));
                };
                let member_matches = matches!(
                    target.segments.last(),
                    Some(CompileHostPathSegment::Field(field)
                        | CompileHostPathSegment::VariantField(field)) if field == member
                );
                if !member_matches {
                    return Err(self.inconsistent(
                        origin,
                        "host field member target disagrees with its host path",
                    ));
                }
                Ok(())
            }
            HirExprKind::Index(index) => {
                self.validate_host_path_prefix(index.receiver, target, origin)?;
                match target.segments.last() {
                    Some(CompileHostPathSegment::ConstantIndex { value, .. }) => {
                        self.validate_constant_host_index(index.index, *value, origin)
                    }
                    Some(CompileHostPathSegment::ConstantKey { value, .. }) => {
                        self.validate_constant_host_key(index.index, value, origin)
                    }
                    Some(
                        CompileHostPathSegment::DynamicIndex { expression, .. }
                        | CompileHostPathSegment::DynamicKey { expression, .. },
                    ) if *expression == index.index => Ok(()),
                    Some(
                        CompileHostPathSegment::DynamicIndex { .. }
                        | CompileHostPathSegment::DynamicKey { .. },
                    ) => Err(self.inconsistent(
                        origin,
                        "host index path argument disagrees with its HIR index expression",
                    )),
                    _ => Err(self
                        .inconsistent(origin, "host index path does not end in an index segment")),
                }
            }
            _ => Err(self.inconsistent(
                origin,
                "host path placement is not attached to a field or index expression",
            )),
        }
    }

    fn validate_constant_host_index(
        &self,
        expression: HirExprId,
        expected: u32,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let Some(HirExprKind::Literal(HirLiteral::Integer(_))) = self
            .body
            .expression(expression)
            .map(|expression| &expression.kind)
        else {
            return Err(self.inconsistent(
                origin,
                "constant host index placement is not attached to an integer literal",
            ));
        };
        let analysis = self.input.analysis();
        let Some(Ok(ResolvedLiteralFact::Scalar(value))) = analysis.literal(expression) else {
            return Err(self.inconsistent(
                origin,
                "constant host index has no resolved scalar literal fact",
            ));
        };
        if host_index_value(value.value()) != Some(expected) {
            return Err(self.inconsistent(
                origin,
                "constant host index value disagrees with its HIR literal",
            ));
        }
        Ok(())
    }

    fn validate_constant_host_key(
        &self,
        expression: HirExprId,
        expected: &str,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let Some(HirExprKind::Literal(HirLiteral::String(actual))) = self
            .body
            .expression(expression)
            .map(|expression| &expression.kind)
        else {
            return Err(self.inconsistent(
                origin,
                "constant host key placement is not attached to a string literal",
            ));
        };
        if actual != expected {
            return Err(self.inconsistent(
                origin,
                "constant host key value disagrees with its HIR literal",
            ));
        }
        Ok(())
    }

    fn validate_host_path_prefix(
        &self,
        receiver: HirExprId,
        target: &CompileHostPathTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let receiver_target =
            self.input.targets().host_path(receiver).ok_or_else(|| {
                self.inconsistent(origin, "host path prefix has no exact placement")
            })?;
        let Some((_, target_prefix)) = target.segments.split_last() else {
            return Err(self.inconsistent(origin, "host path child has no terminal segment"));
        };
        if receiver_target.root != target.root
            || receiver_target.root_type != target.root_type
            || receiver_target.segments.as_slice() != target_prefix
        {
            return Err(self.inconsistent(
                origin,
                "host path prefix disagrees with its receiver placement",
            ));
        }
        self.validate_host_path_expression(receiver, receiver_target, origin)
    }

    fn host_call_field(
        &self,
        expression: HirExprId,
        call: &HirCall,
        origin: MirSourceOrigin,
    ) -> Result<vela_hir::body::HirField, MirBuildError> {
        if call.expression != expression {
            return Err(self.inconsistent(
                origin,
                "host call record expression identity disagrees with its HIR arena key",
            ));
        }
        let Some(HirExprKind::Field(field)) =
            self.body.expression(call.callee).map(|value| &value.kind)
        else {
            return Err(self.inconsistent(origin, "HostAccess call has no field callee"));
        };
        Ok(field.clone())
    }

    fn host_call_path(
        &self,
        receiver: HirExprId,
        expected: Option<&CompileHostPathTarget>,
        origin: MirSourceOrigin,
    ) -> Result<CompileHostPathTarget, MirBuildError> {
        let placed = self
            .input
            .targets()
            .host_path(receiver)
            .cloned()
            .ok_or_else(|| {
                self.inconsistent(
                    origin,
                    "HostAccess call receiver has no host-path placement",
                )
            })?;
        if expected.is_some_and(|expected| expected != &placed) {
            return Err(self.inconsistent(
                origin,
                "HostAccess call path disagrees with its receiver placement",
            ));
        }
        self.validate_host_path_expression(receiver, &placed, origin)?;
        Ok(placed)
    }

    fn validate_host_method(
        &self,
        source_name: &str,
        target: &HostMethodTarget,
        path: &CompileHostPathTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let descriptor = self
            .input
            .targets()
            .method_descriptor(target.owner.semantic, target.semantic)
            .ok_or_else(|| self.inconsistent(origin, "host method target has no descriptor"))?;
        if descriptor.member_name != source_name
            || !matches!(
                descriptor.class,
                CompileMethodClass::Host { runtime } if runtime == target.runtime
            )
            || descriptor.signature != target.signature
            || descriptor.access != target.access
        {
            return Err(self.inconsistent(
                origin,
                "host method descriptor disagrees with the placed callee target",
            ));
        }
        let receiver = self.host_path_result_contract(path, origin)?;
        if receiver.as_ref() != Some(&MirTypeContract::Host(target.owner)) {
            return Err(self.inconsistent(
                origin,
                "host method receiver path lacks its exact terminal host type",
            ));
        }
        Ok(())
    }

    fn require_host_positional_arity<'a>(
        &self,
        arguments: &'a CompileCallArguments,
        arity: usize,
        origin: MirSourceOrigin,
        operation: &str,
    ) -> Result<&'a [HirExprId], MirBuildError> {
        let CompileCallArguments::Positional(values) = arguments else {
            return Err(self.inconsistent(
                origin,
                format!("host {operation} must use canonical positional arguments"),
            ));
        };
        if values.len() != arity {
            return Err(self.inconsistent(
                origin,
                format!("host {operation} placement must contain {arity} arguments"),
            ));
        }
        Ok(values)
    }

    fn host_read_result_type(
        &self,
        expression: HirExprId,
        path: &CompileHostPathTarget,
        origin: MirSourceOrigin,
    ) -> Result<MirValueType, MirBuildError> {
        let from_contract = self
            .host_path_result_contract(path, origin)?
            .map(|contract| self.host_contract_value_type(&contract));
        let from_analysis = value_type(self.input.analysis().expression(expression));
        Ok(match from_contract {
            Some(MirValueType::Dynamic) | None => from_analysis,
            Some(value_type) => value_type,
        })
    }

    fn host_call_result_type(
        &self,
        expression: HirExprId,
        contract: Option<&MirTypeContract>,
    ) -> MirValueType {
        let from_analysis = value_type(self.input.analysis().expression(expression));
        match contract.map(|contract| self.host_contract_value_type(contract)) {
            Some(MirValueType::Dynamic) | None => from_analysis,
            Some(value_type) => value_type,
        }
    }

    fn host_path_result_contract(
        &self,
        path: &CompileHostPathTarget,
        origin: MirSourceOrigin,
    ) -> Result<Option<MirTypeContract>, MirBuildError> {
        let Some(segment) = path.segments.last() else {
            return Ok(Some(MirTypeContract::Host(path.root_type)));
        };
        match segment {
            CompileHostPathSegment::Field(field) | CompileHostPathSegment::VariantField(field) => {
                self.input
                    .targets()
                    .field_descriptor(field.semantic)
                    .map(|descriptor| descriptor.contract.clone())
                    .ok_or_else(|| {
                        self.inconsistent(origin, "host path field has no compile descriptor")
                    })
            }
            CompileHostPathSegment::ConstantIndex { capability, .. }
            | CompileHostPathSegment::ConstantKey { capability, .. }
            | CompileHostPathSegment::DynamicIndex { capability, .. }
            | CompileHostPathSegment::DynamicKey { capability, .. } => Ok(capability.value.clone()),
        }
    }

    fn host_contract_value_type(&self, contract: &MirTypeContract) -> MirValueType {
        match contract {
            MirTypeContract::Primitive(vela_common::PrimitiveTag::Unit) => MirValueType::Unit,
            MirTypeContract::Primitive(primitive) => MirValueType::Primitive(*primitive),
            MirTypeContract::Range => MirValueType::Range,
            MirTypeContract::Iterator(_) => MirValueType::Iterator,
            MirTypeContract::Tuple(elements) => {
                MirValueType::Tuple(u32::try_from(elements.len()).unwrap_or(u32::MAX))
            }
            MirTypeContract::Callable { .. } => MirValueType::Callable,
            MirTypeContract::Shape { type_id, shape } => MirValueType::ScriptType {
                type_id: *type_id,
                shape: *shape,
            },
            MirTypeContract::Variant { type_id, .. } => MirValueType::Enum(*type_id),
            MirTypeContract::Host(target) => MirValueType::Host(*target),
            MirTypeContract::Definition(type_id) => {
                let Some(descriptor) = self.input.targets().type_descriptor(*type_id) else {
                    return MirValueType::Dynamic;
                };
                match descriptor.class {
                    CompileTypeClass::ScriptRecord => {
                        descriptor.shape.map_or(MirValueType::Dynamic, |shape| {
                            MirValueType::ScriptType {
                                type_id: *type_id,
                                shape,
                            }
                        })
                    }
                    CompileTypeClass::ScriptEnum => MirValueType::Enum(*type_id),
                    CompileTypeClass::Host { runtime } => MirValueType::Host(HostTypeTarget {
                        semantic: *type_id,
                        runtime,
                    }),
                    CompileTypeClass::OpaqueExternal
                    | CompileTypeClass::Registry
                    | CompileTypeClass::Standard => MirValueType::Dynamic,
                }
            }
            MirTypeContract::Any
            | MirTypeContract::Array(_)
            | MirTypeContract::Map { .. }
            | MirTypeContract::Set(_)
            | MirTypeContract::Option(_)
            | MirTypeContract::Result { .. } => MirValueType::Dynamic,
        }
    }

    fn append_host_value(
        &mut self,
        origin: MirSourceOrigin,
        value_type: MirValueType,
        operation: MirHostOperation,
        effect: MirEffect,
    ) -> Result<MirOperand, MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(unit());
        }
        let destination = self.function.add_temp(value_type, origin);
        let safepoint = effect
            .requires_safepoint()
            .then(|| self.function.add_safepoint(MirSafepoint::new(origin)));
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(destination)),
                MirStatementKind::Host(operation),
                effect,
                safepoint,
            ),
        )?;
        Ok(MirOperand::Temp(destination))
    }

    fn append_host_effect(
        &mut self,
        origin: MirSourceOrigin,
        operation: MirHostOperation,
        effect: MirEffect,
    ) -> Result<(), MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(());
        }
        let safepoint = effect
            .requires_safepoint()
            .then(|| self.function.add_safepoint(MirSafepoint::new(origin)));
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                None,
                MirStatementKind::Host(operation),
                effect,
                safepoint,
            ),
        )?;
        Ok(())
    }

    fn host_expression_origin(
        &self,
        expression: HirExprId,
    ) -> Result<MirSourceOrigin, MirBuildError> {
        let record = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(
                MirSourceOrigin::body(self.body.id, self.body.origin.span),
                format!("missing HIR expression {expression:?}"),
            )
        })?;
        Ok(MirSourceOrigin::expression(
            self.body.id,
            expression,
            record.origin.span,
        ))
    }
}

fn host_mutation(operation: HirAssignOp) -> MirHostMutation {
    match operation {
        HirAssignOp::Add => MirHostMutation::Add,
        HirAssignOp::Sub => MirHostMutation::Subtract,
        HirAssignOp::Mul => MirHostMutation::Multiply,
        HirAssignOp::Div => MirHostMutation::Divide,
        HirAssignOp::Rem => MirHostMutation::Remainder,
        HirAssignOp::Set => unreachable!("set assignment is a HostAccess write"),
    }
}

const fn unit() -> MirOperand {
    MirOperand::Immediate(MirImmediate::Unit)
}

fn host_index_value(value: vela_common::ScalarValue) -> Option<u32> {
    match value {
        vela_common::ScalarValue::I8(value) => u32::try_from(value).ok(),
        vela_common::ScalarValue::I16(value) => u32::try_from(value).ok(),
        vela_common::ScalarValue::I32(value) => u32::try_from(value).ok(),
        vela_common::ScalarValue::I64(value) => u32::try_from(value).ok(),
        vela_common::ScalarValue::U8(value) => Some(u32::from(value)),
        vela_common::ScalarValue::U16(value) => Some(u32::from(value)),
        vela_common::ScalarValue::U32(value) => Some(value),
        vela_common::ScalarValue::U64(value) => u32::try_from(value).ok(),
        vela_common::ScalarValue::F32(_) | vela_common::ScalarValue::F64(_) => None,
    }
}
