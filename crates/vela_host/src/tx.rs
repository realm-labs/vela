use std::collections::BTreeMap;

use vela_common::Span;

use crate::{
    HostError, HostErrorKind, HostPath, HostRef, HostResult, HostValue, Patch, PatchOp,
    ScriptStateAdapter, add_values, push_value, sub_values,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostObjectSnapshot {
    pub type_id: vela_common::HostTypeId,
    pub object_id: vela_common::HostObjectId,
    pub generation: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PatchTx {
    patches: Vec<Patch>,
    overlay: BTreeMap<HostPath, OverlayEntry>,
}

#[derive(Clone, Debug, PartialEq)]
enum OverlayEntry {
    Value(HostValue),
    Removed,
}

impl PatchTx {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn patches(&self) -> &[Patch] {
        &self.patches
    }

    #[must_use]
    pub fn read_overlay(&self, path: &HostPath) -> Option<&HostValue> {
        match self.overlay.get(path) {
            Some(OverlayEntry::Value(value)) => Some(value),
            Some(OverlayEntry::Removed) | None => None,
        }
    }

    pub fn read_path(
        &self,
        adapter: &(impl ScriptStateAdapter + ?Sized),
        path: &HostPath,
    ) -> HostResult<HostValue> {
        self.read_path_at(adapter, path, None)
    }

    pub fn read_path_at(
        &self,
        adapter: &(impl ScriptStateAdapter + ?Sized),
        path: &HostPath,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        match self.overlay.get(path) {
            Some(OverlayEntry::Value(value)) => Ok(value.clone()),
            Some(OverlayEntry::Removed) => {
                Err(
                    HostError::new(HostErrorKind::MissingPath { path: path.clone() })
                        .with_source_span(source_span),
                )
            }
            None => adapter
                .read_path(path)
                .map_err(|error| error.with_source_span_if_absent(source_span)),
        }
    }

    pub fn set_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.push_patch(path, PatchOp::Set(value), source_span)
    }

    pub fn add_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        base_value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let current = self.overlay_value_or_base(&path, base_value, source_span)?;
        let next = add_values(&current, &value).ok_or_else(|| {
            HostError::new(HostErrorKind::InvalidAdd { path: path.clone() })
                .with_source_span(source_span)
        })?;
        self.patches.push(Patch {
            path: path.clone(),
            op: PatchOp::Add(value),
            source_span,
        });
        self.overlay.insert(path, OverlayEntry::Value(next));
        Ok(())
    }

    pub fn sub_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        base_value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let current = self.overlay_value_or_base(&path, base_value, source_span)?;
        let next = sub_values(&current, &value).ok_or_else(|| {
            HostError::new(HostErrorKind::InvalidSub { path: path.clone() })
                .with_source_span(source_span)
        })?;
        self.patches.push(Patch {
            path: path.clone(),
            op: PatchOp::Sub(value),
            source_span,
        });
        self.overlay.insert(path, OverlayEntry::Value(next));
        Ok(())
    }

    pub fn push_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        base_value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let current = self.overlay_value_or_base(&path, base_value, source_span)?;
        let next = push_value(&current, value.clone()).ok_or_else(|| {
            HostError::new(HostErrorKind::InvalidPush { path: path.clone() })
                .with_source_span(source_span)
        })?;
        self.patches.push(Patch {
            path: path.clone(),
            op: PatchOp::Push(value),
            source_span,
        });
        self.overlay.insert(path, OverlayEntry::Value(next));
        Ok(())
    }

    pub fn remove_path(&mut self, path: HostPath, source_span: Option<Span>) -> HostResult<()> {
        self.patches.push(Patch {
            path: path.clone(),
            op: PatchOp::Remove,
            source_span,
        });
        self.overlay.insert(path, OverlayEntry::Removed);
        Ok(())
    }

    pub fn call_method(
        &mut self,
        path: HostPath,
        method: vela_common::HostMethodId,
        args: Vec<HostValue>,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.patches.push(Patch {
            path,
            op: PatchOp::CallHostMethod { method, args },
            source_span,
        });
        Ok(())
    }

    pub fn require_fresh_ref(host_ref: HostRef, snapshot: &HostObjectSnapshot) -> HostResult<()> {
        if host_ref.type_id != snapshot.type_id {
            return Err(HostError::new(HostErrorKind::TypeMismatch {
                expected: host_ref.type_id,
                actual: snapshot.type_id,
            }));
        }
        if host_ref.object_id != snapshot.object_id {
            return Err(HostError::new(HostErrorKind::ObjectMismatch {
                expected: host_ref.object_id,
                actual: snapshot.object_id,
            }));
        }
        if host_ref.generation != snapshot.generation {
            return Err(HostError::new(HostErrorKind::StaleGeneration {
                expected: host_ref.generation,
                actual: snapshot.generation,
            }));
        }
        Ok(())
    }

    pub fn apply(self, adapter: &mut impl ScriptStateAdapter) -> HostResult<()> {
        adapter.apply_patches(self.patches)
    }

    fn push_patch(
        &mut self,
        path: HostPath,
        op: PatchOp,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        if let PatchOp::Set(value) = &op {
            self.overlay
                .insert(path.clone(), OverlayEntry::Value(value.clone()));
        }
        self.patches.push(Patch {
            path,
            op,
            source_span,
        });
        Ok(())
    }

    fn overlay_value_or_base(
        &self,
        path: &HostPath,
        base_value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        match self.overlay.get(path) {
            Some(OverlayEntry::Value(value)) => Ok(value.clone()),
            Some(OverlayEntry::Removed) => {
                Err(
                    HostError::new(HostErrorKind::MissingPath { path: path.clone() })
                        .with_source_span(source_span),
                )
            }
            None => Ok(base_value),
        }
    }
}
