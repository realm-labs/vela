use vela_analysis::literals::DeferredNumericLiteral;
use vela_common::ShapeId;
use vela_common::{ServiceCallMode, ServiceId, ServiceMethodId};
use vela_def::{FieldId, FunctionId, MethodId, StateId, TypeId, VariantId};

use crate::input::{
    CompileHostIndexCapability, CompileParameterDefault, CompilePositionalPolicy, CompileSignature,
    DynamicMethodTarget, HostFieldTarget, HostMethodTarget,
};
use crate::{
    HostTypeTarget, MirEffect, MirEvaluatedConstant, MirFunctionId, MirGuardId, MirOperand,
    MirPlace, MirRvalue, MirSafepointId, MirSourceOrigin,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirDynamicUnaryOp {
    Negate,
    Not,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirDynamicBinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirIdentityOp {
    Equal,
    NotEqual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirContextualBinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirLiteralSide {
    Left,
    Right,
}

/// A contextual numeric literal validated by analysis before MIR.
#[derive(Clone, Eq, PartialEq)]
pub struct MirContextualNumericLiteral(DeferredNumericLiteral);

impl std::fmt::Debug for MirContextualNumericLiteral {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl MirContextualNumericLiteral {
    #[must_use]
    pub fn text(&self) -> &str {
        self.0.text()
    }

    #[must_use]
    pub fn is_float(&self) -> bool {
        matches!(
            self.0.kind(),
            vela_analysis::literals::NumericLiteralKind::Float
        )
    }
}

impl From<DeferredNumericLiteral> for MirContextualNumericLiteral {
    fn from(value: DeferredNumericLiteral) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirFieldTarget {
    RecordSlot {
        type_id: TypeId,
        shape: ShapeId,
        field: FieldId,
    },
    VariantSlot {
        type_id: TypeId,
        variant: VariantId,
        field: FieldId,
    },
    DynamicRecord {
        name: String,
    },
    DynamicVariant {
        name: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirAggregate {
    Tuple(Vec<MirOperand>),
    Array(Vec<MirOperand>),
    Map(Vec<(String, MirOperand)>),
    SetFromArray {
        source: MirOperand,
    },
    Record {
        type_id: TypeId,
        shape: ShapeId,
        fields: Vec<(FieldId, MirOperand)>,
    },
    /// An unregistered record whose evaluated fields remain in source order.
    DynamicRecord {
        type_name: String,
        fields: Vec<(String, MirOperand)>,
    },
    Enum {
        type_id: TypeId,
        variant: VariantId,
        fields: Vec<(FieldId, MirOperand)>,
    },
    /// An unregistered enum variant whose evaluated fields remain in source
    /// order.
    DynamicVariant {
        owner_name: String,
        variant_name: String,
        fields: Vec<(String, MirOperand)>,
    },
    Closure {
        function: MirFunctionId,
        captures: Vec<MirOperand>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct MirScriptArgument {
    pub parameter: u32,
    pub value: Option<MirOperand>,
}

impl MirScriptArgument {
    #[must_use]
    pub const fn placed(parameter: u32, value: MirOperand) -> Self {
        Self {
            parameter,
            value: Some(value),
        }
    }

    #[must_use]
    pub const fn missing(parameter: u32) -> Self {
        Self {
            parameter,
            value: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MirDynamicArgument {
    pub name: Option<String>,
    pub value: MirOperand,
}

impl MirDynamicArgument {
    #[must_use]
    pub const fn dynamic_positional(value: MirOperand) -> Self {
        Self { name: None, value }
    }

    #[must_use]
    pub fn dynamic_named(name: impl Into<String>, value: MirOperand) -> Self {
        Self {
            name: Some(name.into()),
            value,
        }
    }
}

/// Whether a direct script-function call may skip the callee's parameter
/// contract guards.
///
/// This is a semantic call-site proof, not a bytecode selection hint. Calls
/// with dynamic arguments or omitted typed defaults must retain callee checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirScriptParameterGuardMode {
    ProvenAtCallSite,
    CheckCalleeParameterContracts,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirCall {
    ScriptFunction {
        function: FunctionId,
        debug_name: String,
        signature: CompileSignature,
        arguments: Vec<MirScriptArgument>,
        parameter_guards: MirScriptParameterGuardMode,
    },
    ScriptMethod {
        target: crate::MethodExecutableTarget,
        debug_name: String,
        receiver: MirOperand,
        signature: CompileSignature,
        arguments: Vec<MirScriptArgument>,
    },
    CallableValue {
        callee: MirOperand,
        arguments: Vec<MirOperand>,
    },
    DynamicCallable {
        callee: MirOperand,
        arguments: Vec<MirDynamicArgument>,
    },
    NativeFunction {
        function: FunctionId,
        debug_name: String,
        signature: CompileSignature,
        arguments: Vec<MirOperand>,
        scoped_borrow_return: bool,
    },
    StdlibFunction {
        function: FunctionId,
        debug_name: String,
        signature: CompileSignature,
        arguments: Vec<MirOperand>,
    },
    ValueMethod {
        owner: TypeId,
        method: MethodId,
        debug_name: String,
        receiver: MirOperand,
        signature: CompileSignature,
        arguments: Vec<MirOperand>,
    },
    Service {
        mode: ServiceCallMode,
        service: ServiceId,
        method: ServiceMethodId,
        debug_name: String,
        signature: CompileSignature,
        arguments: Vec<MirOperand>,
    },
    DynamicMethod {
        target: DynamicMethodTarget,
        receiver: MirOperand,
        arguments: Vec<MirDynamicArgument>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirHostPathSegment {
    Field(HostFieldTarget),
    ConstantIndex {
        value: u32,
        capability: CompileHostIndexCapability,
    },
    ConstantKey {
        value: String,
        capability: CompileHostIndexCapability,
    },
    Index {
        value: MirOperand,
        capability: CompileHostIndexCapability,
    },
    Key {
        value: MirOperand,
        capability: CompileHostIndexCapability,
    },
    VariantField(HostFieldTarget),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MirHostPath {
    pub root_type: HostTypeTarget,
    pub segments: Vec<MirHostPathSegment>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirHostMutation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Push,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirHostOperation {
    ReleaseBorrowLease {
        root: MirOperand,
    },
    TryReleaseBorrowLease {
        root: MirOperand,
    },
    Read {
        root: MirOperand,
        path: MirHostPath,
    },
    Write {
        root: MirOperand,
        path: MirHostPath,
        value: MirOperand,
    },
    Mutate {
        root: MirOperand,
        path: MirHostPath,
        operation: MirHostMutation,
        value: MirOperand,
    },
    Remove {
        root: MirOperand,
        path: MirHostPath,
    },
    Call {
        root: MirOperand,
        path: MirHostPath,
        target: Box<HostMethodTarget>,
        arguments: Vec<MirOperand>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirStateOperation {
    ReadVmState { state: StateId },
    WriteVmState { state: StateId, value: MirOperand },
    ReadExternState { state: StateId },
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirReflectionOperation {
    Read {
        function: FunctionId,
        target: MirOperand,
        member: MirOperand,
    },
    Write {
        function: FunctionId,
        target: MirOperand,
        member: MirOperand,
        value: MirOperand,
    },
    /// Preserve `reflect::call` after its first evaluated operand exactly.
    /// Runtime dispatch first treats `target` as a callable; otherwise the
    /// first tail operand is the evaluated dynamic method name.
    Call {
        function: FunctionId,
        target: MirOperand,
        tail: Vec<MirOperand>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirIndexOperation {
    Read {
        receiver: MirOperand,
        index: MirIndexKey,
    },
    Write {
        receiver: MirOperand,
        index: MirIndexKey,
        value: MirOperand,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirIndexKey {
    Value(MirOperand),
    ConstantString(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirIteratorOperation {
    Create { iterable: MirOperand },
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirFormatPart {
    Text(String),
    Value(MirOperand),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MirTaskContinuation {
    pub function: FunctionId,
    pub debug_name: String,
    pub signature: CompileSignature,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MirTaskOperation {
    pub worker: FunctionId,
    pub worker_debug_name: String,
    pub worker_signature: CompileSignature,
    pub arguments: Vec<MirScriptArgument>,
    pub parameter_guards: MirScriptParameterGuardMode,
    pub continuation: Option<MirTaskContinuation>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirStatementKind {
    Assign(MirRvalue),
    Unary {
        operation: crate::MirUnaryOp,
        operand: MirOperand,
    },
    Binary {
        operation: crate::MirBinaryOp,
        left: MirOperand,
        right: MirOperand,
    },
    DynamicUnary {
        operation: MirDynamicUnaryOp,
        operand: MirOperand,
    },
    DynamicBinary {
        operation: MirDynamicBinaryOp,
        left: MirOperand,
        right: MirOperand,
    },
    ContextualNumericBinary {
        operation: MirContextualBinaryOp,
        value: MirOperand,
        literal: MirContextualNumericLiteral,
        literal_side: MirLiteralSide,
    },
    IdentityCompare {
        operation: MirIdentityOp,
        left: MirOperand,
        right: MirOperand,
    },
    TupleField {
        tuple: MirOperand,
        index: u32,
    },
    ReadField {
        receiver: MirOperand,
        target: MirFieldTarget,
    },
    WriteField {
        receiver: MirOperand,
        target: MirFieldTarget,
        value: MirOperand,
    },
    Index(MirIndexOperation),
    State(MirStateOperation),
    Allocate(MirAggregate),
    FormatString {
        parts: Vec<MirFormatPart>,
    },
    MaterializeConstant(MirEvaluatedConstant),
    MakeRange {
        start: MirOperand,
        end: MirOperand,
        inclusive: bool,
    },
    Call(MirCall),
    Task(MirTaskOperation),
    Host(MirHostOperation),
    Reflect(MirReflectionOperation),
    GuardTrap {
        value: MirOperand,
        guard: MirGuardId,
    },
    Iterator(MirIteratorOperation),
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirAwaitOperation {
    Call(MirCall),
    Host(MirHostOperation),
    Reflect(MirReflectionOperation),
}

impl MirAwaitOperation {
    pub(crate) const fn minimum_effect(&self) -> MirEffect {
        match self {
            Self::Call(call) => call.minimum_effect(),
            Self::Host(
                MirHostOperation::ReleaseBorrowLease { .. }
                | MirHostOperation::TryReleaseBorrowLease { .. },
            ) => MirEffect::PURE,
            Self::Host(MirHostOperation::Read { .. }) => MirEffect::host_read(),
            Self::Host(
                MirHostOperation::Write { .. }
                | MirHostOperation::Mutate { .. }
                | MirHostOperation::Remove { .. },
            ) => MirEffect::host_write(),
            Self::Host(MirHostOperation::Call { target, .. }) => {
                MirEffect::host_call().union(target.signature.effect)
            }
            Self::Reflect(MirReflectionOperation::Read { .. }) => MirEffect::reflection_read(),
            Self::Reflect(MirReflectionOperation::Write { .. }) => MirEffect::reflection_write(),
            Self::Reflect(MirReflectionOperation::Call { .. }) => MirEffect::reflection_call(),
        }
    }

    pub(crate) fn has_valid_call_contract(&self) -> bool {
        match self {
            Self::Call(call) => call.has_valid_contract(),
            Self::Host(MirHostOperation::Call {
                target, arguments, ..
            }) => external_arguments_match(&target.signature, arguments.len()),
            Self::Host(_) | Self::Reflect(_) => true,
        }
    }
}

impl MirStatementKind {
    pub(crate) fn has_valid_call_contract(&self) -> bool {
        match self {
            Self::Call(call) => call.has_valid_contract(),
            Self::Task(task) => script_arguments_match(
                &task.worker_signature,
                &task.arguments,
                Some(task.parameter_guards),
            ),
            Self::Host(MirHostOperation::Call {
                target, arguments, ..
            }) => external_arguments_match(&target.signature, arguments.len()),
            _ => true,
        }
    }

    pub(crate) const fn minimum_effect(&self) -> MirEffect {
        match self {
            Self::Assign(_) => MirEffect::PURE,
            Self::Unary { .. }
            | Self::Binary { .. }
            | Self::DynamicUnary { .. }
            | Self::ContextualNumericBinary { .. }
            | Self::IdentityCompare { .. }
            | Self::TupleField { .. }
            | Self::ReadField { .. }
            | Self::WriteField { .. }
            | Self::GuardTrap { .. } => MirEffect::may_trap(),
            Self::DynamicBinary {
                operation:
                    MirDynamicBinaryOp::Equal
                    | MirDynamicBinaryOp::NotEqual
                    | MirDynamicBinaryOp::Less
                    | MirDynamicBinaryOp::LessEqual
                    | MirDynamicBinaryOp::Greater
                    | MirDynamicBinaryOp::GreaterEqual,
                ..
            } => MirEffect::dynamic_call(),
            Self::DynamicBinary { .. } => MirEffect::may_trap(),
            Self::Index(MirIndexOperation::Write { .. }) => MirEffect::allocation(),
            Self::Index(MirIndexOperation::Read { .. }) => MirEffect::may_trap(),
            Self::State(
                MirStateOperation::ReadVmState { .. } | MirStateOperation::ReadExternState { .. },
            ) => MirEffect::state_read(),
            Self::State(MirStateOperation::WriteVmState { .. }) => MirEffect::state_write(),
            Self::Allocate(_) | Self::FormatString { .. } | Self::Iterator(_) => {
                MirEffect::allocation()
            }
            Self::MaterializeConstant(value) => {
                if value.requires_allocation() {
                    MirEffect::allocation()
                } else {
                    MirEffect::PURE
                }
            }
            Self::MakeRange { .. } => MirEffect::may_trap(),
            Self::Call(call) => call.minimum_effect(),
            Self::Task(_) => MirEffect::task_spawn(),
            Self::Host(operation) => match operation {
                MirHostOperation::ReleaseBorrowLease { .. }
                | MirHostOperation::TryReleaseBorrowLease { .. } => MirEffect::PURE,
                MirHostOperation::Read { .. } => MirEffect::host_read(),
                MirHostOperation::Write { .. }
                | MirHostOperation::Mutate { .. }
                | MirHostOperation::Remove { .. } => MirEffect::host_write(),
                MirHostOperation::Call { target, .. } => {
                    MirEffect::host_call().union(target.signature.effect)
                }
            },
            Self::Reflect(operation) => match operation {
                MirReflectionOperation::Read { .. } => MirEffect::reflection_read(),
                MirReflectionOperation::Write { .. } => MirEffect::reflection_write(),
                MirReflectionOperation::Call { .. } => MirEffect::reflection_call(),
            },
        }
    }

    pub(crate) const fn destination_requirement(&self) -> MirDestinationRequirement {
        match self {
            Self::Assign(_)
            | Self::Unary { .. }
            | Self::Binary { .. }
            | Self::DynamicUnary { .. }
            | Self::DynamicBinary { .. }
            | Self::ContextualNumericBinary { .. }
            | Self::IdentityCompare { .. }
            | Self::TupleField { .. }
            | Self::ReadField { .. }
            | Self::Index(MirIndexOperation::Read { .. })
            | Self::State(
                MirStateOperation::ReadVmState { .. } | MirStateOperation::ReadExternState { .. },
            )
            | Self::Allocate(_)
            | Self::FormatString { .. }
            | Self::MaterializeConstant(_)
            | Self::MakeRange { .. }
            | Self::Call(_)
            | Self::Task(_)
            | Self::Host(
                MirHostOperation::ReleaseBorrowLease { .. }
                | MirHostOperation::TryReleaseBorrowLease { .. }
                | MirHostOperation::Read { .. }
                | MirHostOperation::Call { .. },
            )
            | Self::Reflect(
                MirReflectionOperation::Read { .. }
                | MirReflectionOperation::Write { .. }
                | MirReflectionOperation::Call { .. },
            )
            | Self::Iterator(_) => MirDestinationRequirement::Required,
            Self::WriteField { .. }
            | Self::State(MirStateOperation::WriteVmState { .. })
            | Self::Index(MirIndexOperation::Write { .. })
            | Self::Host(
                MirHostOperation::Write { .. }
                | MirHostOperation::Mutate { .. }
                | MirHostOperation::Remove { .. },
            )
            | Self::GuardTrap { .. } => MirDestinationRequirement::Forbidden,
        }
    }

    pub(crate) const fn requires_safepoint(&self) -> bool {
        matches!(
            self,
            Self::Allocate(_)
                | Self::Call(_)
                | Self::Task(_)
                | Self::Host(MirHostOperation::Call { .. })
                | Self::Reflect(MirReflectionOperation::Call { .. })
        )
    }
}

impl MirCall {
    pub(crate) fn has_valid_contract(&self) -> bool {
        match self {
            Self::ScriptFunction {
                signature,
                arguments,
                parameter_guards,
                ..
            } => script_arguments_match(signature, arguments, Some(*parameter_guards)),
            Self::ScriptMethod {
                signature,
                arguments,
                ..
            } => script_arguments_match(signature, arguments, None),
            Self::CallableValue { .. } => true,
            Self::DynamicCallable { arguments, .. } => dynamic_arguments_are_ordered(arguments),
            Self::NativeFunction {
                signature,
                arguments,
                ..
            }
            | Self::StdlibFunction {
                signature,
                arguments,
                ..
            }
            | Self::ValueMethod {
                signature,
                arguments,
                ..
            }
            | Self::Service {
                signature,
                arguments,
                ..
            } => external_arguments_match(signature, arguments.len()),
            Self::DynamicMethod {
                target, arguments, ..
            } => dynamic_arguments_match(target, arguments),
        }
    }

    const fn minimum_effect(&self) -> MirEffect {
        match self {
            Self::ScriptFunction { signature, .. } | Self::ScriptMethod { signature, .. } => {
                MirEffect::script_call().union(signature.effect)
            }
            Self::CallableValue { .. }
            | Self::DynamicCallable { .. }
            | Self::DynamicMethod { .. } => MirEffect::dynamic_call(),
            Self::NativeFunction { signature, .. }
            | Self::StdlibFunction { signature, .. }
            | Self::ValueMethod { signature, .. }
            | Self::Service { signature, .. } => MirEffect::external_call().union(signature.effect),
        }
    }

    pub(crate) const fn known_asyncness(&self) -> Option<vela_common::CallableAsyncness> {
        match self {
            Self::ScriptFunction { signature, .. }
            | Self::ScriptMethod { signature, .. }
            | Self::NativeFunction { signature, .. }
            | Self::StdlibFunction { signature, .. }
            | Self::ValueMethod { signature, .. }
            | Self::Service { signature, .. } => Some(signature.asyncness),
            Self::CallableValue { .. }
            | Self::DynamicCallable { .. }
            | Self::DynamicMethod { .. } => None,
        }
    }
}

fn script_arguments_match(
    signature: &CompileSignature,
    arguments: &[MirScriptArgument],
    parameter_guards: Option<MirScriptParameterGuardMode>,
) -> bool {
    if signature.positional != CompilePositionalPolicy::ExactOrTrailingDefaults
        || arguments.len() != signature.parameters.len()
    {
        return false;
    }
    for (index, (argument, parameter)) in arguments.iter().zip(&signature.parameters).enumerate() {
        if argument.parameter != index as u32 {
            return false;
        }
        match (argument.value.is_none(), parameter.default) {
            (true, CompileParameterDefault::HirBody(_))
            | (false, CompileParameterDefault::Required | CompileParameterDefault::HirBody(_)) => {}
            (true | false, CompileParameterDefault::RuntimeProvided)
            | (true, CompileParameterDefault::Required) => return false,
        }
    }
    if parameter_guards == Some(MirScriptParameterGuardMode::ProvenAtCallSite)
        && arguments
            .iter()
            .zip(&signature.parameters)
            .any(|(argument, parameter)| argument.value.is_none() && parameter.contract.is_some())
    {
        return false;
    }
    true
}

fn external_arguments_match(signature: &CompileSignature, provided: usize) -> bool {
    match signature.positional {
        CompilePositionalPolicy::RuntimeChecked => true,
        CompilePositionalPolicy::Variadic { minimum } => {
            usize::try_from(minimum).is_ok_and(|minimum| provided >= minimum)
        }
        CompilePositionalPolicy::ExactOrTrailingDefaults => {
            if provided > signature.parameters.len()
                || signature.parameters.iter().any(|parameter| {
                    matches!(parameter.default, CompileParameterDefault::HirBody(_))
                })
            {
                return false;
            }
            signature.parameters[provided..].iter().all(|parameter| {
                matches!(parameter.default, CompileParameterDefault::RuntimeProvided)
            })
        }
    }
}

fn dynamic_arguments_are_ordered(arguments: &[MirDynamicArgument]) -> bool {
    let mut saw_named = false;
    for argument in arguments {
        if argument.name.is_some() {
            saw_named = true;
        } else if saw_named {
            return false;
        }
    }
    true
}

fn dynamic_arguments_match(target: &DynamicMethodTarget, arguments: &[MirDynamicArgument]) -> bool {
    if !dynamic_arguments_are_ordered(arguments) {
        return false;
    }
    let positional = arguments
        .iter()
        .take_while(|argument| argument.name.is_none())
        .count();
    let names = arguments
        .iter()
        .filter_map(|argument| argument.name.as_deref())
        .collect::<Vec<_>>();
    usize::try_from(target.positional_arity) == Ok(positional)
        && names
            .iter()
            .copied()
            .eq(target.named_arguments.iter().map(String::as_str))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MirDestinationRequirement {
    Required,
    Forbidden,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MirStatement {
    pub origin: MirSourceOrigin,
    pub destination: Option<MirPlace>,
    pub kind: MirStatementKind,
    pub effect: MirEffect,
    pub safepoint: Option<MirSafepointId>,
}

impl MirStatement {
    #[must_use]
    pub const fn new(
        origin: MirSourceOrigin,
        destination: Option<MirPlace>,
        kind: MirStatementKind,
        effect: MirEffect,
        safepoint: Option<MirSafepointId>,
    ) -> Self {
        Self {
            origin,
            destination,
            kind,
            effect,
            safepoint,
        }
    }

    #[must_use]
    pub fn assign(origin: MirSourceOrigin, destination: MirPlace, value: MirRvalue) -> Self {
        Self::new(
            origin,
            Some(destination),
            MirStatementKind::Assign(value),
            MirEffect::PURE,
            None,
        )
    }
}
