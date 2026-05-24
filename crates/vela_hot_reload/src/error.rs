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
