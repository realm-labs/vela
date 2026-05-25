use crate::{
    AccessAbi, EffectAbi, HotReloadError, HotReloadErrorKind, ModuleExportAbi, ParamAbi, SchemaAbi,
    TraitMethodAbi,
};

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
    FunctionReturnAbi {
        old: Option<String>,
        new: Option<String>,
    },
    AddedFunctionParameters {
        added: Vec<String>,
    },
    SchemaHash {
        old_hash: u64,
        new_hash: Option<u64>,
    },
    SchemaMemberAbi {
        old: Box<SchemaAbi>,
        new: Box<SchemaAbi>,
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
    MethodReturnAbi {
        old: Option<String>,
        new: Option<String>,
    },
    MethodAccessAbi {
        old: AccessAbi,
        new: AccessAbi,
    },
    TraitMethodAbiList {
        old: Vec<TraitMethodAbi>,
        new: Vec<TraitMethodAbi>,
    },
    ModuleExportAbiList {
        old: Vec<ModuleExportAbi>,
        new: Vec<ModuleExportAbi>,
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
            | HotReloadErrorKind::RemovedMethodAbi { .. }
            | HotReloadErrorKind::RemovedTraitAbi { .. }
            | HotReloadErrorKind::RemovedModuleAbi { .. } => None,
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
            HotReloadErrorKind::ChangedFunctionReturnAbi { old, new, .. } => {
                Some(Self::FunctionReturnAbi {
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
            HotReloadErrorKind::ChangedSchemaAbi { old, new, .. } => Some(Self::SchemaMemberAbi {
                old: old.clone(),
                new: new.clone(),
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
            HotReloadErrorKind::ChangedMethodReturnAbi { old, new, .. } => {
                Some(Self::MethodReturnAbi {
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
            HotReloadErrorKind::ChangedTraitAbi { old, new, .. } => {
                Some(Self::TraitMethodAbiList {
                    old: old.clone(),
                    new: new.clone(),
                })
            }
            HotReloadErrorKind::ChangedModuleAbi { old, new, .. } => {
                Some(Self::ModuleExportAbiList {
                    old: old.clone(),
                    new: new.clone(),
                })
            }
        }
    }
}
