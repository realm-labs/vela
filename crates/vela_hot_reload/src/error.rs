use std::fmt;

use vela_bytecode::compiler::CompileError;

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
            HotReloadErrorKind::RemovedSchema { type_name, .. } => {
                format!("schema `{type_name}` was removed")
            }
            HotReloadErrorKind::ChangedSchema { type_name, .. } => {
                format!("schema `{type_name}` changed incompatibly")
            }
            HotReloadErrorKind::ChangedFunctionEffects { function, .. } => {
                format!("function `{function}` changed effect ABI")
            }
            HotReloadErrorKind::ChangedFunctionAccess { function, .. } => {
                format!("function `{function}` changed reflective access ABI")
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
            HotReloadErrorKind::RemovedSchema { .. } => {
                Some("restore the schema or restart with an explicit migration".to_owned())
            }
            HotReloadErrorKind::ChangedSchema { .. } => {
                Some("keep the existing schema hash stable or restart with an explicit migration".to_owned())
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
    RemovedSchema {
        type_name: String,
        old_hash: u64,
    },
    ChangedSchema {
        type_name: String,
        old_hash: u64,
        new_hash: u64,
    },
    ChangedFunctionEffects {
        function: String,
        old: EffectAbi,
        new: EffectAbi,
    },
    ChangedFunctionAccess {
        function: String,
        old: AccessAbi,
        new: AccessAbi,
    },
    ChangedMethodEffects {
        type_name: String,
        method: String,
        old: EffectAbi,
        new: EffectAbi,
    },
    ChangedMethodAccess {
        type_name: String,
        method: String,
        old: AccessAbi,
        new: AccessAbi,
    },
}

pub type HotReloadResult<T> = Result<T, HotReloadError>;
