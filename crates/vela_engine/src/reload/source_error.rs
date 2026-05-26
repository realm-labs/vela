use std::fmt;

use vela_hot_reload::HotReloadError;

use crate::source::EngineSourceError;

#[derive(Clone, Debug, PartialEq)]
pub struct EngineHotReloadSourceError {
    pub kind: EngineHotReloadSourceErrorKind,
}

impl EngineHotReloadSourceError {
    pub(crate) fn source(error: EngineSourceError) -> Self {
        Self {
            kind: EngineHotReloadSourceErrorKind::Source(error),
        }
    }

    pub(crate) fn hot_reload(error: HotReloadError) -> Self {
        Self {
            kind: EngineHotReloadSourceErrorKind::HotReload(error),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EngineHotReloadSourceErrorKind {
    Source(EngineSourceError),
    HotReload(HotReloadError),
}

impl fmt::Display for EngineHotReloadSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            EngineHotReloadSourceErrorKind::Source(error) => write!(formatter, "{error}"),
            EngineHotReloadSourceErrorKind::HotReload(error) => write!(formatter, "{error:?}"),
        }
    }
}

impl std::error::Error for EngineHotReloadSourceError {}

pub type EngineHotReloadSourceResult<T> = Result<T, EngineHotReloadSourceError>;
