use vela_common::{Diagnostic, Span};

use crate::verification::VerificationError;

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
            CompileErrorKind::SyntaxDiagnostics(_)
            | CompileErrorKind::SemanticDiagnostics(_)
            | CompileErrorKind::FunctionNotFound(_)
            | CompileErrorKind::UnknownLocal(_)
            | CompileErrorKind::RegisterOverflow
            | CompileErrorKind::BytecodeVerification(_)
            | CompileErrorKind::UnsupportedSyntax(_) => return None,
        };
        Some(match self.span {
            Some(span) => diagnostic.with_span(span),
            None => diagnostic,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CompileErrorKind {
    SyntaxDiagnostics(Vec<Diagnostic>),
    SemanticDiagnostics(Vec<Diagnostic>),
    FunctionNotFound(String),
    UnknownLocal(String),
    InvalidIntLiteral { literal: String, error: String },
    InvalidFloatLiteral { literal: String, error: String },
    RegisterOverflow,
    BytecodeVerification(VerificationError),
    UnsupportedSyntax(&'static str),
}

pub type CompileResult<T> = Result<T, CompileError>;
