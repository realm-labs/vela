use std::fmt;
use std::sync::Arc;

use vela_bytecode::Register;
use vela_common::Span;
use vela_host::{HostError, HostErrorKind};
use vela_reflect::{ReflectError, ReflectErrorKind};

use crate::ExecutionBudgetKind;

#[derive(Clone, Debug, PartialEq)]
pub struct VmError {
    pub kind: VmErrorKind,
    pub source_span: Option<Span>,
    pub call_stack: Arc<[VmStackFrame]>,
}

impl VmError {
    pub(crate) fn new(kind: VmErrorKind) -> Self {
        Self {
            kind,
            source_span: None,
            call_stack: Default::default(),
        }
    }

    pub(crate) fn with_call_frame(mut self, frame: VmStackFrame) -> Self {
        let mut call_stack = self.call_stack.iter().cloned().collect::<Vec<_>>();
        call_stack.push(frame);
        self.call_stack = Arc::from(call_stack.into_boxed_slice());
        self
    }

    pub(crate) fn with_source_span_if_absent(mut self, source_span: Option<Span>) -> Self {
        if self.source_span.is_none() {
            self.source_span = source_span;
        }
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VmStackFrame {
    pub function: String,
    pub call_site: Option<Span>,
}

impl VmStackFrame {
    pub(crate) fn new(function: impl Into<String>, call_site: Option<Span>) -> Self {
        Self {
            function: function.into(),
            call_site,
        }
    }
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl std::error::Error for VmError {}

#[derive(Clone, Debug, PartialEq)]
pub enum VmErrorKind {
    RegisterOutOfBounds {
        register: Register,
    },
    ConstantOutOfBounds {
        constant: usize,
    },
    InstructionOutOfBounds {
        offset: usize,
    },
    TypeMismatch {
        operation: &'static str,
    },
    DivisionByZero,
    UnknownNative {
        name: String,
    },
    PermissionDenied {
        native: String,
        permission: String,
    },
    UnknownFunction {
        name: String,
    },
    UnknownMethod {
        method: String,
    },
    ArityMismatch {
        name: String,
        expected: usize,
        actual: usize,
    },
    Host(HostErrorKind),
    Reflect(ReflectErrorKind),
    UnknownRecordField {
        type_name: String,
        field: String,
    },
    UnknownEnumField {
        enum_name: String,
        variant: String,
        field: String,
    },
    IndexOutOfBounds {
        index: i64,
        len: usize,
    },
    UnknownMapKey {
        key: String,
    },
    BudgetExceeded {
        budget: ExecutionBudgetKind,
        limit: u64,
    },
    MissingReturn,
}

pub type VmResult<T> = Result<T, VmError>;

impl From<HostError> for VmError {
    fn from(value: HostError) -> Self {
        Self {
            kind: VmErrorKind::Host(value.kind),
            source_span: value.source_span,
            call_stack: Default::default(),
        }
    }
}

impl From<ReflectError> for VmError {
    fn from(value: ReflectError) -> Self {
        Self::new(VmErrorKind::Reflect(value.kind))
    }
}
