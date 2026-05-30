use std::collections::BTreeMap;

use vela_common::Span;

use crate::{HostError, HostErrorKind, HostPath, HostResult, HostValue};

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PatchOverlay {
    entries: BTreeMap<HostPath, OverlayEntry>,
}

#[derive(Clone, Debug, PartialEq)]
enum OverlayEntry {
    Value(HostValue),
    Removed,
}

impl PatchOverlay {
    pub(crate) fn read(&self, path: &HostPath) -> Option<&HostValue> {
        match self.entries.get(path) {
            Some(OverlayEntry::Value(value)) => Some(value),
            Some(OverlayEntry::Removed) | None => None,
        }
    }

    pub(crate) fn read_or_base(
        &self,
        path: &HostPath,
        base_value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        match self.entries.get(path) {
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
        match self.entries.get(path) {
            Some(OverlayEntry::Value(value)) => Ok(Some(value.clone())),
            Some(OverlayEntry::Removed) => Err(missing_path(path, source_span)),
            None => Ok(None),
        }
    }

    pub(crate) fn expected_base(
        &self,
        path: &HostPath,
        base_value: &HostValue,
    ) -> Option<HostValue> {
        (!self.entries.contains_key(path)).then(|| base_value.clone())
    }

    pub(crate) fn set_value(&mut self, path: HostPath, value: HostValue) {
        self.entries.insert(path, OverlayEntry::Value(value));
    }

    pub(crate) fn remove(&mut self, path: HostPath) {
        self.entries.insert(path, OverlayEntry::Removed);
    }
}

fn missing_path(path: &HostPath, source_span: Option<Span>) -> HostError {
    HostError::new(HostErrorKind::MissingPath { path: path.clone() }).with_source_span(source_span)
}
