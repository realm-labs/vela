use std::collections::BTreeMap;

use vela_common::Span;

use crate::{
    error::{HostError, HostErrorKind, HostResult},
    path::{HostPath, HostPathKey},
    value::HostValue,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PatchOverlay {
    entries: BTreeMap<HostPathKey, OverlayEntry>,
}

#[derive(Clone, Debug, PartialEq)]
enum OverlayEntry {
    Value(HostValue),
    Removed,
}

impl PatchOverlay {
    pub(crate) fn read(&self, path: &HostPath) -> Option<&HostValue> {
        let key = path.path_key();
        self.read_key(&key)
    }

    pub(crate) fn read_key(&self, key: &HostPathKey) -> Option<&HostValue> {
        match self.entries.get(key) {
            Some(OverlayEntry::Value(value)) => Some(value),
            Some(OverlayEntry::Removed) | None => None,
        }
    }

    pub(crate) fn read_or_base_key(
        &self,
        key: &HostPathKey,
        path: &HostPath,
        base_value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        match self.entries.get(key) {
            Some(OverlayEntry::Value(value)) => Ok(value.clone()),
            Some(OverlayEntry::Removed) => Err(missing_path(path, source_span)),
            None => Ok(base_value),
        }
    }

    pub(crate) fn overlaid_value(
        &self,
        path: &HostPath,
        source_span: Option<Span>,
    ) -> HostResult<Option<HostValue>> {
        let key = path.path_key();
        self.overlaid_value_key(&key, path, source_span)
    }

    pub(crate) fn overlaid_value_key(
        &self,
        key: &HostPathKey,
        path: &HostPath,
        source_span: Option<Span>,
    ) -> HostResult<Option<HostValue>> {
        match self.entries.get(key) {
            Some(OverlayEntry::Value(value)) => Ok(Some(value.clone())),
            Some(OverlayEntry::Removed) => Err(missing_path(path, source_span)),
            None => Ok(None),
        }
    }

    pub(crate) fn expected_base_key(
        &self,
        key: &HostPathKey,
        base_value: &HostValue,
    ) -> Option<HostValue> {
        (!self.entries.contains_key(key)).then(|| base_value.clone())
    }

    pub(crate) fn set_value_key(&mut self, key: HostPathKey, value: HostValue) {
        self.entries.insert(key, OverlayEntry::Value(value));
    }

    pub(crate) fn remove_key(&mut self, key: HostPathKey) {
        self.entries.insert(key, OverlayEntry::Removed);
    }
}

fn missing_path(path: &HostPath, source_span: Option<Span>) -> HostError {
    HostError::new(HostErrorKind::MissingPath { path: path.clone() }).with_source_span(source_span)
}
