use std::collections::BTreeMap;

use vela_common::Span;

use crate::{
    HostError, HostErrorKind, HostPath, HostRef, HostResult, HostValue, Patch, PatchOp,
    ScriptStateAdapter, add_values, sub_values,
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
    overlay: BTreeMap<HostPath, HostValue>,
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
        self.overlay.get(path)
    }

    pub fn read_path(
        &self,
        adapter: &(impl ScriptStateAdapter + ?Sized),
        path: &HostPath,
    ) -> HostResult<HostValue> {
        self.overlay
            .get(path)
            .cloned()
            .map_or_else(|| adapter.read_path(path), Ok)
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
        let current = self.overlay.get(&path).cloned().unwrap_or(base_value);
        let next = add_values(&current, &value)
            .ok_or_else(|| HostError::new(HostErrorKind::InvalidAdd { path: path.clone() }))?;
        self.patches.push(Patch {
            path: path.clone(),
            op: PatchOp::Add(value),
            source_span,
        });
        self.overlay.insert(path, next);
        Ok(())
    }

    pub fn sub_path(
        &mut self,
        path: HostPath,
        value: HostValue,
        base_value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let current = self.overlay.get(&path).cloned().unwrap_or(base_value);
        let next = sub_values(&current, &value)
            .ok_or_else(|| HostError::new(HostErrorKind::InvalidSub { path: path.clone() }))?;
        self.patches.push(Patch {
            path: path.clone(),
            op: PatchOp::Sub(value),
            source_span,
        });
        self.overlay.insert(path, next);
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
        for patch in &self.patches {
            adapter.validate_patch(patch)?;
        }
        for patch in self.patches {
            adapter.apply_patch(patch)?;
        }
        Ok(())
    }

    fn push_patch(
        &mut self,
        path: HostPath,
        op: PatchOp,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        if let PatchOp::Set(value) = &op {
            self.overlay.insert(path.clone(), value.clone());
        }
        self.patches.push(Patch {
            path,
            op,
            source_span,
        });
        Ok(())
    }
}
