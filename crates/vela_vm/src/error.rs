use std::fmt;
use std::sync::Arc;

use vela_bytecode::Register;
use vela_common::{Diagnostic, Span};
use vela_host::error::{HostError, HostErrorKind};
use vela_reflect::error::{ReflectError, ReflectErrorKind};

use crate::budget::ExecutionBudgetKind;

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

    #[must_use]
    pub fn to_diagnostic(&self) -> Diagnostic {
        let mut diagnostic = Diagnostic::error(self.kind.message()).with_code(self.kind.code());
        if let Some(span) = self.source_span {
            diagnostic = diagnostic.with_span(span).with_label(span, "runtime error");
        }

        if let VmErrorKind::Reflect(kind) = &self.kind {
            for (span, message) in kind.related_labels() {
                diagnostic = diagnostic.with_label(span, message);
            }
        }

        for frame in self.call_stack.iter() {
            if let Some(call_site) = frame.call_site {
                diagnostic = diagnostic
                    .with_label(call_site, format!("while executing `{}`", frame.function));
            }
        }
        diagnostic
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

impl VmErrorKind {
    fn code(&self) -> &'static str {
        match self {
            Self::RegisterOutOfBounds { .. } => "vm::register_out_of_bounds",
            Self::ConstantOutOfBounds { .. } => "vm::constant_out_of_bounds",
            Self::InstructionOutOfBounds { .. } => "vm::instruction_out_of_bounds",
            Self::TypeMismatch { .. } => "vm::type_mismatch",
            Self::DivisionByZero => "vm::division_by_zero",
            Self::UnknownNative { .. } => "vm::unknown_native",
            Self::PermissionDenied { .. } => "vm::permission_denied",
            Self::UnknownFunction { .. } => "vm::unknown_function",
            Self::UnknownMethod { .. } => "vm::unknown_method",
            Self::ArityMismatch { .. } => "vm::arity_mismatch",
            Self::Host(_) => "vm::host_error",
            Self::Reflect(kind) => kind.code(),
            Self::UnknownRecordField { .. } => "vm::unknown_record_field",
            Self::UnknownEnumField { .. } => "vm::unknown_enum_field",
            Self::IndexOutOfBounds { .. } => "vm::index_out_of_bounds",
            Self::UnknownMapKey { .. } => "vm::unknown_map_key",
            Self::BudgetExceeded { .. } => "vm::budget_exceeded",
            Self::MissingReturn => "vm::missing_return",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::RegisterOutOfBounds { register } => {
                format!("register {} is out of bounds", register.0)
            }
            Self::ConstantOutOfBounds { constant } => {
                format!("constant {constant} is out of bounds")
            }
            Self::InstructionOutOfBounds { offset } => {
                format!("instruction offset {offset} is out of bounds")
            }
            Self::TypeMismatch { operation } => {
                format!("type mismatch during `{operation}`")
            }
            Self::DivisionByZero => "division by zero".to_owned(),
            Self::UnknownNative { name } => format!("unknown native function `{name}`"),
            Self::PermissionDenied { native, permission } => {
                format!("native `{native}` requires permission `{permission}`")
            }
            Self::UnknownFunction { name } => format!("unknown function `{name}`"),
            Self::UnknownMethod { method } => format!("unknown method `{method}`"),
            Self::ArityMismatch {
                name,
                expected,
                actual,
            } => {
                format!("`{name}` expected {expected} arguments but got {actual}")
            }
            Self::Host(kind) => format!("host error: {kind:?}"),
            Self::Reflect(kind) => kind.message(),
            Self::UnknownRecordField { type_name, field } => {
                format!("unknown field `{field}` for record `{type_name}`")
            }
            Self::UnknownEnumField {
                enum_name,
                variant,
                field,
            } => {
                format!("unknown field `{field}` for enum variant `{enum_name}.{variant}`")
            }
            Self::IndexOutOfBounds { index, len } => {
                format!("index {index} is out of bounds for length {len}")
            }
            Self::UnknownMapKey { key } => format!("unknown map key `{key}`"),
            Self::BudgetExceeded { budget, limit } => {
                format!("execution budget exceeded for {budget:?} with limit {limit}")
            }
            Self::MissingReturn => "function completed without returning a value".to_owned(),
        }
    }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vela_common::diagnostic_render::{DiagnosticSource, render_diagnostic};
    use vela_common::{SourceId, Span};
    use vela_reflect::candidates::ReflectCandidate;
    use vela_reflect::error::ReflectErrorKind;

    use super::{VmError, VmErrorKind, VmStackFrame};

    #[test]
    fn diagnostic_includes_call_stack_labels() {
        let source = "\
fn main() {
    return middle();
}

fn middle() {
    return leaf();
}

fn leaf() {
    return 10 / 0;
}
";
        let leaf_call = Span::new(SourceId::new(1), 57, 63);
        let middle_call = Span::new(SourceId::new(1), 23, 31);
        let error = VmError {
            kind: VmErrorKind::DivisionByZero,
            source_span: Some(leaf_call),
            call_stack: Arc::from([
                VmStackFrame::new("leaf", Some(leaf_call)),
                VmStackFrame::new("middle", Some(middle_call)),
                VmStackFrame::new("main", None),
            ]),
        };

        let diagnostic = error.to_diagnostic();

        assert_eq!(diagnostic.code.as_deref(), Some("vm::division_by_zero"));
        assert_eq!(diagnostic.message, "division by zero");
        assert_eq!(diagnostic.span, Some(leaf_call));
        assert!(
            diagnostic
                .labels
                .iter()
                .any(|label| label.message == "while executing `middle`")
        );

        let rendered = render_diagnostic(
            &diagnostic,
            [DiagnosticSource::new(
                SourceId::new(1),
                "combat.vela",
                source,
            )],
        )
        .join("\n");
        assert!(rendered.contains("error[vm::division_by_zero]: division by zero"));
        assert!(rendered.contains("while executing `leaf`"));
        assert!(rendered.contains("while executing `middle`"));
    }

    #[test]
    fn diagnostic_describes_runtime_error_kinds() {
        let error = VmError::new(VmErrorKind::ArityMismatch {
            name: "reward.grant".to_owned(),
            expected: 2,
            actual: 1,
        });

        let diagnostic = error.to_diagnostic();

        assert_eq!(diagnostic.code.as_deref(), Some("vm::arity_mismatch"));
        assert_eq!(
            diagnostic.message,
            "`reward.grant` expected 2 arguments but got 1"
        );
        assert!(diagnostic.labels.is_empty());
    }

    #[test]
    fn diagnostic_preserves_reflection_error_candidates() {
        let call_span = Span::new(SourceId::new(1), 12, 39);
        let field_span = Span::new(SourceId::new(2), 16, 21);
        let error = VmError {
            kind: VmErrorKind::Reflect(ReflectErrorKind::UnknownField {
                type_name: "Player".to_owned(),
                field: "leve".to_owned(),
                candidates: vec!["level".to_owned()],
                related: vec![ReflectCandidate::new("level", Some(field_span))],
            }),
            source_span: Some(call_span),
            call_stack: Default::default(),
        };

        let diagnostic = error.to_diagnostic();

        assert_eq!(diagnostic.code.as_deref(), Some("reflect::unknown_field"));
        assert_eq!(
            diagnostic.message,
            "unknown reflected field `leve` on `Player`; candidates: level"
        );
        assert_eq!(diagnostic.span, Some(call_span));
        assert!(diagnostic.labels.iter().any(|label| {
            label.span == field_span && label.message == "candidate `level` is declared here"
        }));

        let rendered = render_diagnostic(
            &diagnostic,
            [
                DiagnosticSource::new(
                    SourceId::new(1),
                    "script.vela",
                    "fn main() { reflect.get(player, \"leve\") }",
                ),
                DiagnosticSource::new(SourceId::new(2), "schema.vela", "struct Player { level }"),
            ],
        )
        .join("\n");
        assert!(rendered.contains("error[reflect::unknown_field]"));
        assert!(rendered.contains("candidate `level` is declared here"));
    }
}
