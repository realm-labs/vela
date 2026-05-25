use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineError {
    pub kind: EngineErrorKind,
}

impl EngineError {
    #[must_use]
    pub const fn new(kind: EngineErrorKind) -> Self {
        Self { kind }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineErrorKind {
    DuplicateNativeFunctionId {
        id: u64,
    },
    DuplicateNativeFunctionName {
        name: String,
    },
    DuplicateNativeFunctionParamName {
        function: String,
        name: String,
    },
    DuplicateTypeId {
        id: u32,
    },
    DuplicateTypeName {
        name: String,
    },
    DuplicateHostTypeId {
        id: u32,
    },
    DuplicateFieldId {
        type_name: String,
        id: u32,
    },
    DuplicateFieldName {
        type_name: String,
        name: String,
    },
    DuplicateVariantId {
        type_name: String,
        id: u32,
    },
    DuplicateVariantName {
        type_name: String,
        name: String,
    },
    DuplicateVariantFieldId {
        type_name: String,
        variant: String,
        id: u32,
    },
    DuplicateVariantFieldName {
        type_name: String,
        variant: String,
        name: String,
    },
    DuplicateTraitId {
        type_name: String,
        id: u32,
    },
    DuplicateTraitName {
        type_name: String,
        name: String,
    },
    DuplicateTraitMethodId {
        type_name: String,
        trait_name: String,
        id: u32,
    },
    DuplicateTraitMethodName {
        type_name: String,
        trait_name: String,
        name: String,
    },
    DuplicateHostMethodId {
        id: u32,
    },
    DuplicateHostMethodName {
        name: String,
    },
    DuplicateHostMethodParamName {
        type_name: String,
        method: String,
        name: String,
    },
    DuplicateTraitMethodParamName {
        type_name: String,
        trait_name: String,
        method: String,
        name: String,
    },
    UnknownNativeMethodOwner {
        name: String,
    },
    RuntimeNotHotReloadEnabled,
}

pub type EngineResult<T> = Result<T, EngineError>;

impl fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            EngineErrorKind::DuplicateNativeFunctionId { id } => {
                write!(formatter, "duplicate native function id {id}")
            }
            EngineErrorKind::DuplicateNativeFunctionName { name } => {
                write!(formatter, "duplicate native function name {name}")
            }
            EngineErrorKind::DuplicateNativeFunctionParamName { function, name } => {
                write!(
                    formatter,
                    "duplicate parameter name {name} on native function {function}"
                )
            }
            EngineErrorKind::DuplicateTypeId { id } => write!(formatter, "duplicate type id {id}"),
            EngineErrorKind::DuplicateTypeName { name } => {
                write!(formatter, "duplicate type name {name}")
            }
            EngineErrorKind::DuplicateHostTypeId { id } => {
                write!(formatter, "duplicate host type id {id}")
            }
            EngineErrorKind::DuplicateFieldId { type_name, id } => {
                write!(formatter, "duplicate field id {id} on type {type_name}")
            }
            EngineErrorKind::DuplicateFieldName { type_name, name } => {
                write!(formatter, "duplicate field name {name} on type {type_name}")
            }
            EngineErrorKind::DuplicateVariantId { type_name, id } => {
                write!(formatter, "duplicate variant id {id} on type {type_name}")
            }
            EngineErrorKind::DuplicateVariantName { type_name, name } => {
                write!(
                    formatter,
                    "duplicate variant name {name} on type {type_name}"
                )
            }
            EngineErrorKind::DuplicateVariantFieldId {
                type_name,
                variant,
                id,
            } => {
                write!(
                    formatter,
                    "duplicate variant field id {id} on {type_name}.{variant}"
                )
            }
            EngineErrorKind::DuplicateVariantFieldName {
                type_name,
                variant,
                name,
            } => {
                write!(
                    formatter,
                    "duplicate variant field name {name} on {type_name}.{variant}"
                )
            }
            EngineErrorKind::DuplicateTraitId { type_name, id } => {
                write!(
                    formatter,
                    "duplicate implemented trait id {id} on type {type_name}"
                )
            }
            EngineErrorKind::DuplicateTraitName { type_name, name } => {
                write!(
                    formatter,
                    "duplicate implemented trait name {name} on type {type_name}"
                )
            }
            EngineErrorKind::DuplicateTraitMethodId {
                type_name,
                trait_name,
                id,
            } => {
                write!(
                    formatter,
                    "duplicate trait method id {id} on {type_name}.{trait_name}"
                )
            }
            EngineErrorKind::DuplicateTraitMethodName {
                type_name,
                trait_name,
                name,
            } => {
                write!(
                    formatter,
                    "duplicate trait method name {name} on {type_name}.{trait_name}"
                )
            }
            EngineErrorKind::DuplicateHostMethodId { id } => {
                write!(formatter, "duplicate host method id {id}")
            }
            EngineErrorKind::DuplicateHostMethodName { name } => {
                write!(formatter, "duplicate host method name {name}")
            }
            EngineErrorKind::DuplicateHostMethodParamName {
                type_name,
                method,
                name,
            } => {
                write!(
                    formatter,
                    "duplicate parameter name {name} on host method {type_name}.{method}"
                )
            }
            EngineErrorKind::DuplicateTraitMethodParamName {
                type_name,
                trait_name,
                method,
                name,
            } => {
                write!(
                    formatter,
                    "duplicate parameter name {name} on trait method {type_name}.{trait_name}.{method}"
                )
            }
            EngineErrorKind::UnknownNativeMethodOwner { name } => {
                write!(formatter, "unknown native method owner type {name}")
            }
            EngineErrorKind::RuntimeNotHotReloadEnabled => {
                write!(
                    formatter,
                    "runtime was not created from a hot reload version"
                )
            }
        }
    }
}

impl std::error::Error for EngineError {}
