use std::fmt;

use vela_bytecode::compiler::{CompileError, CompileErrorKind};
use vela_common::{Diagnostic, Label, Span};

use crate::{AccessAbi, EffectAbi};

#[derive(Clone, Debug, PartialEq)]
pub struct HotReloadError {
    pub kind: HotReloadErrorKind,
}

impl HotReloadError {
    pub(crate) fn new(kind: HotReloadErrorKind) -> Self {
        Self { kind }
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        match &self.kind {
            HotReloadErrorKind::Compile(_) => "reload.compile",
            HotReloadErrorKind::DeletedFunctionParameters { .. } => {
                "reload.function.deleted_parameters"
            }
            HotReloadErrorKind::ChangedFunctionParameters { .. } => {
                "reload.function.changed_parameters"
            }
            HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { .. } => {
                "reload.function.required_added_parameters"
            }
            HotReloadErrorKind::AddedFunctionParametersDenied { .. } => {
                "reload.function.added_parameters_denied"
            }
            HotReloadErrorKind::NewFunctionDenied { .. } => "reload.function.new_denied",
            HotReloadErrorKind::RemovedFunction { .. } => "reload.function.removed",
            HotReloadErrorKind::RemovedSchema { .. } => "reload.schema.removed",
            HotReloadErrorKind::ChangedSchema { .. } => "reload.schema.changed",
            HotReloadErrorKind::RemovedFunctionAbi { .. } => "reload.function.removed_abi",
            HotReloadErrorKind::ChangedFunctionEffects { .. } => "reload.function.effects_changed",
            HotReloadErrorKind::ChangedFunctionAccess { .. } => "reload.function.access_changed",
            HotReloadErrorKind::RemovedMethodAbi { .. } => "reload.method.removed_abi",
            HotReloadErrorKind::ChangedMethodEffects { .. } => "reload.method.effects_changed",
            HotReloadErrorKind::ChangedMethodAccess { .. } => "reload.method.access_changed",
        }
    }

    #[must_use]
    pub fn target(&self) -> Option<String> {
        match &self.kind {
            HotReloadErrorKind::Compile(_) => None,
            HotReloadErrorKind::DeletedFunctionParameters { function, .. }
            | HotReloadErrorKind::ChangedFunctionParameters { function, .. }
            | HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { function, .. }
            | HotReloadErrorKind::AddedFunctionParametersDenied { function, .. }
            | HotReloadErrorKind::NewFunctionDenied { function }
            | HotReloadErrorKind::RemovedFunction { function }
            | HotReloadErrorKind::RemovedFunctionAbi { function, .. }
            | HotReloadErrorKind::ChangedFunctionEffects { function, .. }
            | HotReloadErrorKind::ChangedFunctionAccess { function, .. } => Some(function.clone()),
            HotReloadErrorKind::RemovedSchema { type_name, .. }
            | HotReloadErrorKind::ChangedSchema { type_name, .. } => Some(type_name.clone()),
            HotReloadErrorKind::RemovedMethodAbi {
                type_name, method, ..
            }
            | HotReloadErrorKind::ChangedMethodEffects {
                type_name, method, ..
            }
            | HotReloadErrorKind::ChangedMethodAccess {
                type_name, method, ..
            } => Some(format!("{type_name}.{method}")),
        }
    }

    #[must_use]
    pub fn reason(&self) -> String {
        match &self.kind {
            HotReloadErrorKind::Compile(_) => "updated source failed to compile".to_owned(),
            HotReloadErrorKind::DeletedFunctionParameters { function, .. } => {
                format!("function `{function}` deleted existing parameters")
            }
            HotReloadErrorKind::ChangedFunctionParameters { function, .. } => {
                format!("function `{function}` changed existing parameter names or order")
            }
            HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { function, .. } => {
                format!("function `{function}` added required parameters")
            }
            HotReloadErrorKind::AddedFunctionParametersDenied { function, .. } => {
                format!("function `{function}` added parameters denied by reload policy")
            }
            HotReloadErrorKind::NewFunctionDenied { function } => {
                format!("new function `{function}` is denied by reload policy")
            }
            HotReloadErrorKind::RemovedFunction { function } => {
                format!("function `{function}` was removed from the update source")
            }
            HotReloadErrorKind::RemovedSchema { type_name, .. } => {
                format!("schema `{type_name}` was removed")
            }
            HotReloadErrorKind::ChangedSchema { type_name, .. } => {
                format!("schema `{type_name}` changed incompatibly")
            }
            HotReloadErrorKind::RemovedFunctionAbi { function, .. } => {
                format!("function `{function}` was removed from the hot-reload ABI")
            }
            HotReloadErrorKind::ChangedFunctionEffects { function, .. } => {
                format!("function `{function}` changed effect ABI")
            }
            HotReloadErrorKind::ChangedFunctionAccess { function, .. } => {
                format!("function `{function}` changed reflective access ABI")
            }
            HotReloadErrorKind::RemovedMethodAbi {
                type_name, method, ..
            } => {
                format!("method `{type_name}.{method}` was removed from the hot-reload ABI")
            }
            HotReloadErrorKind::ChangedMethodEffects {
                type_name, method, ..
            } => {
                format!("method `{type_name}.{method}` changed effect ABI")
            }
            HotReloadErrorKind::ChangedMethodAccess {
                type_name, method, ..
            } => {
                format!("method `{type_name}.{method}` changed reflective access ABI")
            }
        }
    }

    #[must_use]
    pub fn repair_hint(&self) -> Option<String> {
        match &self.kind {
            HotReloadErrorKind::Compile(_) => Some("fix compile diagnostics and retry".to_owned()),
            HotReloadErrorKind::DeletedFunctionParameters { .. } => {
                Some("restore the previous parameter prefix or add a compatibility wrapper".to_owned())
            }
            HotReloadErrorKind::ChangedFunctionParameters { .. } => {
                Some("preserve existing parameter names and order".to_owned())
            }
            HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { .. } => {
                Some("give every appended parameter a default value".to_owned())
            }
            HotReloadErrorKind::AddedFunctionParametersDenied { .. } => {
                Some("enable defaulted parameter additions in HotReloadPolicy or remove the new parameters".to_owned())
            }
            HotReloadErrorKind::NewFunctionDenied { .. } => {
                Some("enable new functions in HotReloadPolicy or remove the new declaration".to_owned())
            }
            HotReloadErrorKind::RemovedFunction { .. } => {
                Some("keep the function declaration or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::RemovedSchema { .. } => {
                Some("restore the schema or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::ChangedSchema { .. } => {
                Some("keep the existing schema hash stable or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::RemovedFunctionAbi { .. } => {
                Some("restore the function ABI entry or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::RemovedMethodAbi { .. } => {
                Some("restore the method ABI entry or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::ChangedFunctionEffects { .. }
            | HotReloadErrorKind::ChangedMethodEffects { .. } => {
                Some("preserve the previous effect set or require host approval before reloading".to_owned())
            }
            HotReloadErrorKind::ChangedFunctionAccess { .. }
            | HotReloadErrorKind::ChangedMethodAccess { .. } => {
                Some("preserve reflective access metadata or require host approval before reloading".to_owned())
            }
        }
    }

    #[must_use]
    pub fn source_span(&self) -> Option<Span> {
        match &self.kind {
            HotReloadErrorKind::Compile(error) => compile_diagnostics(error)
                .and_then(|diagnostics| diagnostics.iter().find_map(|diagnostic| diagnostic.span)),
            HotReloadErrorKind::RemovedSchema { source_span, .. }
            | HotReloadErrorKind::ChangedSchema { source_span, .. }
            | HotReloadErrorKind::RemovedFunctionAbi { source_span, .. }
            | HotReloadErrorKind::ChangedFunctionEffects { source_span, .. }
            | HotReloadErrorKind::ChangedFunctionAccess { source_span, .. }
            | HotReloadErrorKind::RemovedMethodAbi { source_span, .. }
            | HotReloadErrorKind::ChangedMethodEffects { source_span, .. }
            | HotReloadErrorKind::ChangedMethodAccess { source_span, .. } => {
                source_span.as_deref().copied()
            }
            HotReloadErrorKind::DeletedFunctionParameters { .. }
            | HotReloadErrorKind::ChangedFunctionParameters { .. }
            | HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { .. }
            | HotReloadErrorKind::AddedFunctionParametersDenied { .. }
            | HotReloadErrorKind::NewFunctionDenied { .. }
            | HotReloadErrorKind::RemovedFunction { .. } => None,
        }
    }

    #[must_use]
    pub fn labels(&self) -> Vec<Label> {
        let HotReloadErrorKind::Compile(error) = &self.kind else {
            return Vec::new();
        };
        compile_diagnostics(error)
            .into_iter()
            .flat_map(|diagnostics| diagnostics.iter())
            .flat_map(|diagnostic| diagnostic.labels.iter().cloned())
            .collect()
    }

    #[must_use]
    pub fn source_diagnostics(&self) -> Vec<Diagnostic> {
        let HotReloadErrorKind::Compile(error) = &self.kind else {
            return Vec::new();
        };
        compile_diagnostics(error).map_or_else(Vec::new, |diagnostics| diagnostics.to_vec())
    }
}

fn compile_diagnostics(error: &CompileError) -> Option<&[Diagnostic]> {
    match &error.kind {
        CompileErrorKind::SyntaxDiagnostics(diagnostics)
        | CompileErrorKind::SemanticDiagnostics(diagnostics) => Some(diagnostics),
        CompileErrorKind::FunctionNotFound(_)
        | CompileErrorKind::UnknownLocal(_)
        | CompileErrorKind::InvalidIntLiteral { .. }
        | CompileErrorKind::InvalidFloatLiteral { .. }
        | CompileErrorKind::RegisterOverflow
        | CompileErrorKind::UnsupportedSyntax(_) => None,
    }
}

impl fmt::Display for HotReloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl std::error::Error for HotReloadError {}

#[derive(Clone, Debug, PartialEq)]
pub enum HotReloadErrorKind {
    Compile(CompileError),
    DeletedFunctionParameters {
        function: String,
        old: Vec<String>,
        new: Vec<String>,
    },
    ChangedFunctionParameters {
        function: String,
        old: Vec<String>,
        new: Vec<String>,
    },
    AddedFunctionParametersWithoutDefaults {
        function: String,
        added: Vec<String>,
    },
    AddedFunctionParametersDenied {
        function: String,
        added: Vec<String>,
    },
    NewFunctionDenied {
        function: String,
    },
    RemovedFunction {
        function: String,
    },
    RemovedSchema {
        type_name: String,
        old_hash: u64,
        source_span: Option<Box<Span>>,
    },
    ChangedSchema {
        type_name: String,
        old_hash: u64,
        new_hash: u64,
        source_span: Option<Box<Span>>,
    },
    RemovedFunctionAbi {
        function: String,
        source_span: Option<Box<Span>>,
    },
    ChangedFunctionEffects {
        function: String,
        old: EffectAbi,
        new: EffectAbi,
        source_span: Option<Box<Span>>,
    },
    ChangedFunctionAccess {
        function: String,
        old: AccessAbi,
        new: AccessAbi,
        source_span: Option<Box<Span>>,
    },
    RemovedMethodAbi {
        type_name: String,
        method: String,
        source_span: Option<Box<Span>>,
    },
    ChangedMethodEffects {
        type_name: String,
        method: String,
        old: EffectAbi,
        new: EffectAbi,
        source_span: Option<Box<Span>>,
    },
    ChangedMethodAccess {
        type_name: String,
        method: String,
        old: AccessAbi,
        new: AccessAbi,
        source_span: Option<Box<Span>>,
    },
}

pub type HotReloadResult<T> = Result<T, HotReloadError>;
