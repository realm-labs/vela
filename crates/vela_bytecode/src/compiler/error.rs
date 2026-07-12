use vela_common::{Diagnostic, Span};

use crate::verification::VerificationError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompilationRequestError {
    EmptyModuleGraph,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirBackendFailureKind {
    MissingRoot,
    MissingFunction(vela_mir::MirFunctionId),
    MissingBlock(vela_mir::MirBlockId),
    MissingStatement,
    MissingDestination,
    MissingTarget(&'static str),
    DynamicHostArgumentOverflow,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MirBackendFailure {
    pub function: vela_mir::MirFunctionId,
    pub origin: vela_mir::MirSourceOrigin,
    pub kind: MirBackendFailureKind,
}

impl std::fmt::Display for MirBackendFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "backend failure in {} at {}:{}..{}: {:?}",
            self.function,
            self.origin.span.source.get(),
            self.origin.span.start,
            self.origin.span.end,
            self.kind
        )
    }
}

impl std::error::Error for MirBackendFailure {}

#[derive(Clone, Debug, PartialEq)]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub span: Option<Span>,
}

impl CompileError {
    pub(super) fn new(kind: CompileErrorKind) -> Self {
        Self { kind, span: None }
    }

    pub(super) fn with_span(mut self, span: Span) -> Self {
        if self.span.is_none() {
            self.span = Some(span);
        }
        self
    }

    #[must_use]
    pub fn to_diagnostic(&self) -> Option<Diagnostic> {
        let diagnostic = match &self.kind {
            CompileErrorKind::InvalidIntLiteral { literal, error } => {
                Diagnostic::error(format!("invalid integer literal `{literal}`: {error}"))
                    .with_code("compiler::invalid_int_literal")
            }
            CompileErrorKind::InvalidFloatLiteral { literal, error } => {
                Diagnostic::error(format!("invalid float literal `{literal}`: {error}"))
                    .with_code("compiler::invalid_float_literal")
            }
            CompileErrorKind::UnsupportedRecordPattern => {
                Diagnostic::error("record patterns must use an owner-qualified constructor path")
                    .with_code("compiler::unqualified_record_pattern")
            }
            CompileErrorKind::InvalidMirRootCount { count } => Diagnostic::error(format!(
                "selected function produced {count} verified MIR roots"
            ))
            .with_code("compiler::invalid_mir_root_count"),
            CompileErrorKind::InvalidCompilationRequest(error) => {
                Diagnostic::error(format!("invalid bytecode compilation request: {error:?}"))
                    .with_code("compiler::invalid_compilation_request")
            }
            CompileErrorKind::InvalidHirGraph(_)
            | CompileErrorKind::SemanticDiagnostics(_)
            | CompileErrorKind::UnknownLocal(_)
            | CompileErrorKind::UnsupportedSyntax(_) => return None,
            CompileErrorKind::RegisterOverflow => Diagnostic::error(
                "function requires more physical registers than the bytecode format supports",
            )
            .with_code("compiler::register_overflow"),
            CompileErrorKind::BytecodeVerification(error) => Diagnostic::error(format!(
                "bytecode verification failed in {}: {:?}",
                error.function, error.kind
            ))
            .with_code("compiler::bytecode_verification"),
            CompileErrorKind::MirVerification(error) => {
                Diagnostic::error(format!("MIR verification failed: {}", error))
                    .with_code("compiler::mir_verification")
            }
            CompileErrorKind::MirBackendHandoff(error) => {
                Diagnostic::error(error.to_string()).with_code("compiler::mir_backend_handoff")
            }
            CompileErrorKind::MirBackend(error) => Diagnostic::error(format!(
                "MIR backend failed in {}: {:?}",
                error.function, error.kind
            ))
            .with_code("compiler::mir_backend"),
            CompileErrorKind::MirInput(error) => {
                Diagnostic::error(format!("inconsistent compiler MIR input: {error}"))
                    .with_code("compiler::inconsistent_mir_input")
            }
            CompileErrorKind::RegistrySnapshot(message) => Diagnostic::error(format!(
                "invalid compile-target registry snapshot: {message}"
            ))
            .with_code("compiler::invalid_registry_snapshot"),
        };
        Some(match self.span {
            Some(span) => diagnostic.with_span(span),
            None => diagnostic,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CompileErrorKind {
    InvalidHirGraph(Vec<Diagnostic>),
    InvalidCompilationRequest(CompilationRequestError),
    SemanticDiagnostics(Vec<Diagnostic>),
    UnknownLocal(String),
    InvalidIntLiteral { literal: String, error: String },
    InvalidFloatLiteral { literal: String, error: String },
    RegisterOverflow,
    InvalidMirRootCount { count: usize },
    BytecodeVerification(VerificationError),
    MirVerification(Box<vela_mir::MirVerifyError>),
    MirBackendHandoff(vela_mir::MirBackendHandoffError),
    MirBackend(Box<MirBackendFailure>),
    UnsupportedSyntax(&'static str),
    UnsupportedRecordPattern,
    MirInput(Box<vela_mir::MirBuildError>),
    RegistrySnapshot(String),
}

pub type CompileResult<T> = Result<T, CompileError>;

#[cfg(test)]
mod tests {
    use vela_common::{SourceId, Span};

    use super::*;

    #[test]
    fn physical_register_overflow_has_a_stable_spanned_diagnostic() {
        let span = Span::new(SourceId::new(8), 10, 20);
        let error = CompileError::new(CompileErrorKind::RegisterOverflow).with_span(span);
        let diagnostic = error.to_diagnostic().expect("register diagnostic");

        assert_eq!(
            diagnostic.code.as_deref(),
            Some("compiler::register_overflow")
        );
        assert_eq!(diagnostic.span, Some(span));
    }
}
