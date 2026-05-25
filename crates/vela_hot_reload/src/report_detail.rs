use crate::{AccessAbi, EffectAbi, HotReloadError, HotReloadErrorKind, ParamAbi};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HotReloadDiagnosticDetail {
    FunctionParameterList {
        old: Vec<String>,
        new: Vec<String>,
    },
    FunctionParameterAbiList {
        old: Vec<ParamAbi>,
        new: Vec<ParamAbi>,
    },
    AddedFunctionParameters {
        added: Vec<String>,
    },
    SchemaHash {
        old_hash: u64,
        new_hash: Option<u64>,
    },
    FunctionEventAbi {
        old: Option<String>,
        new: Option<String>,
    },
    FunctionEffectAbi {
        old: EffectAbi,
        new: EffectAbi,
    },
    FunctionAccessAbi {
        old: AccessAbi,
        new: AccessAbi,
    },
    MethodEffectAbi {
        old: EffectAbi,
        new: EffectAbi,
    },
    MethodParameterAbiList {
        old: Vec<ParamAbi>,
        new: Vec<ParamAbi>,
    },
    MethodAccessAbi {
        old: AccessAbi,
        new: AccessAbi,
    },
}

impl HotReloadDiagnosticDetail {
    #[must_use]
    pub fn from_error(error: &HotReloadError) -> Option<Self> {
        match &error.kind {
            HotReloadErrorKind::Compile(_)
            | HotReloadErrorKind::NewFunctionDenied { .. }
            | HotReloadErrorKind::RemovedFunction { .. }
            | HotReloadErrorKind::RemovedFunctionAbi { .. }
            | HotReloadErrorKind::RemovedMethodAbi { .. } => None,
            HotReloadErrorKind::DeletedFunctionParameters { old, new, .. }
            | HotReloadErrorKind::ChangedFunctionParameters { old, new, .. } => {
                Some(Self::FunctionParameterList {
                    old: old.clone(),
                    new: new.clone(),
                })
            }
            HotReloadErrorKind::ChangedFunctionParameterAbi { old, new, .. } => {
                Some(Self::FunctionParameterAbiList {
                    old: old.clone(),
                    new: new.clone(),
                })
            }
            HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { added, .. }
            | HotReloadErrorKind::AddedFunctionParametersDenied { added, .. } => {
                Some(Self::AddedFunctionParameters {
                    added: added.clone(),
                })
            }
            HotReloadErrorKind::RemovedSchema { old_hash, .. } => Some(Self::SchemaHash {
                old_hash: *old_hash,
                new_hash: None,
            }),
            HotReloadErrorKind::ChangedSchema {
                old_hash, new_hash, ..
            } => Some(Self::SchemaHash {
                old_hash: *old_hash,
                new_hash: Some(*new_hash),
            }),
            HotReloadErrorKind::ChangedFunctionEvent { old, new, .. } => {
                Some(Self::FunctionEventAbi {
                    old: old.clone(),
                    new: new.clone(),
                })
            }
            HotReloadErrorKind::ChangedFunctionEffects { old, new, .. } => {
                Some(Self::FunctionEffectAbi {
                    old: old.clone(),
                    new: new.clone(),
                })
            }
            HotReloadErrorKind::ChangedFunctionAccess { old, new, .. } => {
                Some(Self::FunctionAccessAbi {
                    old: old.clone(),
                    new: new.clone(),
                })
            }
            HotReloadErrorKind::ChangedMethodEffects { old, new, .. } => {
                Some(Self::MethodEffectAbi {
                    old: old.clone(),
                    new: new.clone(),
                })
            }
            HotReloadErrorKind::ChangedMethodParameterAbi { old, new, .. } => {
                Some(Self::MethodParameterAbiList {
                    old: old.clone(),
                    new: new.clone(),
                })
            }
            HotReloadErrorKind::ChangedMethodAccess { old, new, .. } => {
                Some(Self::MethodAccessAbi {
                    old: old.clone(),
                    new: new.clone(),
                })
            }
        }
    }
}
