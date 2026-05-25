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
    DuplicateNativeFunctionId { id: u64 },
    DuplicateNativeFunctionName { name: String },
    DuplicateTypeId { id: u32 },
    DuplicateTypeName { name: String },
    DuplicateHostTypeId { id: u32 },
    DuplicateHostMethodId { id: u32 },
    DuplicateHostMethodName { name: String },
    UnknownNativeMethodOwner { name: String },
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
            EngineErrorKind::DuplicateTypeId { id } => write!(formatter, "duplicate type id {id}"),
            EngineErrorKind::DuplicateTypeName { name } => {
                write!(formatter, "duplicate type name {name}")
            }
            EngineErrorKind::DuplicateHostTypeId { id } => {
                write!(formatter, "duplicate host type id {id}")
            }
            EngineErrorKind::DuplicateHostMethodId { id } => {
                write!(formatter, "duplicate host method id {id}")
            }
            EngineErrorKind::DuplicateHostMethodName { name } => {
                write!(formatter, "duplicate host method name {name}")
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
