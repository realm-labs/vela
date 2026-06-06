use vela_common::Diagnostic;

use crate::verification::VerificationError;

#[derive(Clone, Debug, PartialEq)]
pub struct CompileError {
    pub kind: CompileErrorKind,
}

impl CompileError {
    pub(super) fn new(kind: CompileErrorKind) -> Self {
        Self { kind }
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
