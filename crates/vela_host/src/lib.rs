//! Host reference, path, and patch transaction model.

use std::collections::BTreeMap;
use std::fmt;

use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, Span, Symbol};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostRef {
    pub type_id: HostTypeId,
    pub object_id: HostObjectId,
    pub generation: u32,
}

impl HostRef {
    #[must_use]
    pub fn new(type_id: HostTypeId, object_id: HostObjectId, generation: u32) -> Self {
        Self {
            type_id,
            object_id,
            generation,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostPath {
    pub root: HostRef,
    pub segments: Vec<PathSegment>,
}

impl HostPath {
    #[must_use]
    pub fn new(root: HostRef) -> Self {
        Self {
            root,
            segments: Vec::new(),
        }
    }

    #[must_use]
    pub fn field(mut self, field: FieldId) -> Self {
        self.segments.push(PathSegment::Field(field));
        self
    }

    #[must_use]
    pub fn index(mut self, index: u32) -> Self {
        self.segments.push(PathSegment::Index(index));
        self
    }

    #[must_use]
    pub fn key(mut self, key: Symbol) -> Self {
        self.segments.push(PathSegment::Key(key));
        self
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PathSegment {
    Field(FieldId),
    Index(u32),
    Key(Symbol),
    VariantField(FieldId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub path: HostPath,
    pub op: PatchOp,
    pub source_span: Option<Span>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PatchOp {
    Set(HostValue),
    Add(HostValue),
    Sub(HostValue),
    Remove,
    Push(HostValue),
    CallHostMethod {
        method: HostMethodId,
        args: Vec<HostValue>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum HostValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostObjectSnapshot {
    pub type_id: HostTypeId,
    pub object_id: HostObjectId,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct HostObjectKey {
    type_id: HostTypeId,
    object_id: HostObjectId,
}

impl HostObjectKey {
    fn from_ref(host_ref: HostRef) -> Self {
        Self {
            type_id: host_ref.type_id,
            object_id: host_ref.object_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostError {
    pub kind: HostErrorKind,
}

impl HostError {
    fn new(kind: HostErrorKind) -> Self {
        Self { kind }
    }
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl std::error::Error for HostError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostErrorKind {
    StaleGeneration {
        expected: u32,
        actual: u32,
    },
    ObjectMismatch {
        expected: HostObjectId,
        actual: HostObjectId,
    },
    TypeMismatch {
        expected: HostTypeId,
        actual: HostTypeId,
    },
    MissingOverlay {
        path: HostPath,
    },
    MissingPath {
        path: HostPath,
    },
    InvalidAdd {
        path: HostPath,
    },
    UnsupportedPatch {
        op: &'static str,
    },
    UnsupportedMethod {
        method: HostMethodId,
    },
}

pub type HostResult<T> = Result<T, HostError>;

pub trait ScriptStateAdapter {
    fn read_path(&self, path: &HostPath) -> HostResult<HostValue>;

    fn write_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()>;

    fn call_method(
        &mut self,
        path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue>;

    fn validate_patch(&self, patch: &Patch) -> HostResult<()>;

    fn apply_patch(&mut self, patch: Patch) -> HostResult<()>;
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

    pub fn call_method(
        &mut self,
        path: HostPath,
        method: HostMethodId,
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MockStateAdapter {
    objects: BTreeMap<HostObjectKey, u32>,
    values: BTreeMap<HostPath, HostValue>,
    method_returns: BTreeMap<HostMethodId, HostValue>,
    method_calls: Vec<(HostPath, HostMethodId, Vec<HostValue>)>,
}

impl MockStateAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_value(&mut self, path: HostPath, value: HostValue) {
        self.objects
            .insert(HostObjectKey::from_ref(path.root), path.root.generation);
        self.values.insert(path, value);
    }

    pub fn insert_method_return(&mut self, method: HostMethodId, value: HostValue) {
        self.method_returns.insert(method, value);
    }

    #[must_use]
    pub fn method_calls(&self) -> &[(HostPath, HostMethodId, Vec<HostValue>)] {
        &self.method_calls
    }
}

impl ScriptStateAdapter for MockStateAdapter {
    fn read_path(&self, path: &HostPath) -> HostResult<HostValue> {
        self.validate_path(path)?;
        self.values
            .get(path)
            .cloned()
            .ok_or_else(|| HostError::new(HostErrorKind::MissingPath { path: path.clone() }))
    }

    fn write_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()> {
        self.validate_path(path)?;
        self.values.insert(path.clone(), value);
        Ok(())
    }

    fn call_method(
        &mut self,
        path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        self.validate_path(path)?;
        let value = self
            .method_returns
            .get(&method)
            .cloned()
            .ok_or_else(|| HostError::new(HostErrorKind::UnsupportedMethod { method }))?;
        self.method_calls
            .push((path.clone(), method, args.to_vec()));
        Ok(value)
    }

    fn validate_patch(&self, patch: &Patch) -> HostResult<()> {
        self.validate_path(&patch.path)?;
        match &patch.op {
            PatchOp::Set(_) | PatchOp::Add(_) => Ok(()),
            PatchOp::Sub(_) => Err(HostError::new(HostErrorKind::UnsupportedPatch {
                op: "sub",
            })),
            PatchOp::Remove => Err(HostError::new(HostErrorKind::UnsupportedPatch {
                op: "remove",
            })),
            PatchOp::Push(_) => Err(HostError::new(HostErrorKind::UnsupportedPatch {
                op: "push",
            })),
            PatchOp::CallHostMethod { method, .. } if self.method_returns.contains_key(method) => {
                Ok(())
            }
            PatchOp::CallHostMethod { method, .. } => {
                Err(HostError::new(HostErrorKind::UnsupportedMethod {
                    method: *method,
                }))
            }
        }
    }

    fn apply_patch(&mut self, patch: Patch) -> HostResult<()> {
        self.validate_patch(&patch)?;
        match patch.op {
            PatchOp::Set(value) => self.write_path(&patch.path, value),
            PatchOp::Add(value) => {
                let current = self.read_path(&patch.path)?;
                let next = add_values(&current, &value).ok_or_else(|| {
                    HostError::new(HostErrorKind::InvalidAdd {
                        path: patch.path.clone(),
                    })
                })?;
                self.write_path(&patch.path, next)
            }
            PatchOp::Sub(_) => Err(HostError::new(HostErrorKind::UnsupportedPatch {
                op: "sub",
            })),
            PatchOp::Remove => Err(HostError::new(HostErrorKind::UnsupportedPatch {
                op: "remove",
            })),
            PatchOp::Push(_) => Err(HostError::new(HostErrorKind::UnsupportedPatch {
                op: "push",
            })),
            PatchOp::CallHostMethod { method, args } => {
                self.call_method(&patch.path, method, &args).map(|_| ())
            }
        }
    }
}

impl MockStateAdapter {
    fn validate_path(&self, path: &HostPath) -> HostResult<()> {
        let key = HostObjectKey::from_ref(path.root);
        let Some(generation) = self.objects.get(&key).copied() else {
            return Err(HostError::new(HostErrorKind::MissingPath {
                path: path.clone(),
            }));
        };
        let snapshot = HostObjectSnapshot {
            type_id: path.root.type_id,
            object_id: path.root.object_id,
            generation,
        };
        PatchTx::require_fresh_ref(path.root, &snapshot)
    }
}

fn add_values(lhs: &HostValue, rhs: &HostValue) -> Option<HostValue> {
    match (lhs, rhs) {
        (HostValue::Int(lhs), HostValue::Int(rhs)) => Some(HostValue::Int(lhs + rhs)),
        (HostValue::Float(lhs), HostValue::Float(rhs)) => Some(HostValue::Float(lhs + rhs)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player_ref(generation: u32) -> HostRef {
        HostRef::new(HostTypeId::new(1), HostObjectId::new(7), generation)
    }

    fn level_path() -> HostPath {
        HostPath::new(player_ref(3)).field(FieldId::new(2))
    }

    #[test]
    fn set_path_records_patch_and_overlay_value() {
        let mut tx = PatchTx::new();
        let path = level_path();

        tx.set_path(path.clone(), HostValue::Int(10), None)
            .expect("set path");

        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
        assert_eq!(tx.read_overlay(&path), Some(&HostValue::Int(10)));
    }

    #[test]
    fn add_path_records_patch_and_updates_overlay_from_base() {
        let mut tx = PatchTx::new();
        let path = level_path();

        tx.add_path(path.clone(), HostValue::Int(1), HostValue::Int(9), None)
            .expect("add path");

        assert_eq!(tx.patches().len(), 1);
        assert_eq!(tx.patches()[0].op, PatchOp::Add(HostValue::Int(1)));
        assert_eq!(tx.read_overlay(&path), Some(&HostValue::Int(10)));
    }

    #[test]
    fn add_path_uses_previous_overlay_value() {
        let mut tx = PatchTx::new();
        let path = level_path();

        tx.set_path(path.clone(), HostValue::Int(10), None)
            .expect("set path");
        tx.add_path(path.clone(), HostValue::Int(5), HostValue::Int(0), None)
            .expect("add path");

        assert_eq!(tx.read_overlay(&path), Some(&HostValue::Int(15)));
    }

    #[test]
    fn stale_generation_reports_error() {
        let host_ref = player_ref(3);
        let snapshot = HostObjectSnapshot {
            type_id: host_ref.type_id,
            object_id: host_ref.object_id,
            generation: 4,
        };

        let error = PatchTx::require_fresh_ref(host_ref, &snapshot).expect_err("stale ref");

        assert_eq!(
            error.kind,
            HostErrorKind::StaleGeneration {
                expected: 3,
                actual: 4
            }
        );
    }

    #[test]
    fn transaction_read_prefers_overlay_before_adapter_snapshot() {
        let mut adapter = MockStateAdapter::new();
        let path = level_path();
        adapter.insert_value(path.clone(), HostValue::Int(9));
        let mut tx = PatchTx::new();

        assert_eq!(tx.read_path(&adapter, &path), Ok(HostValue::Int(9)));

        tx.set_path(path.clone(), HostValue::Int(10), None)
            .expect("set path");

        assert_eq!(tx.read_path(&adapter, &path), Ok(HostValue::Int(10)));
        assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(9)));
    }

    #[test]
    fn apply_commits_set_and_add_at_safe_point() {
        let mut adapter = MockStateAdapter::new();
        let path = level_path();
        adapter.insert_value(path.clone(), HostValue::Int(9));
        let mut tx = PatchTx::new();

        tx.set_path(path.clone(), HostValue::Int(10), None)
            .expect("set path");
        tx.add_path(path.clone(), HostValue::Int(2), HostValue::Int(0), None)
            .expect("add path");
        assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(9)));

        tx.apply(&mut adapter).expect("apply transaction");

        assert_eq!(adapter.read_path(&path), Ok(HostValue::Int(12)));
    }

    #[test]
    fn adapter_rejects_stale_generation_on_read_and_apply() {
        let mut adapter = MockStateAdapter::new();
        let fresh_path = level_path();
        adapter.insert_value(fresh_path, HostValue::Int(9));
        let stale_path = HostPath::new(player_ref(2)).field(FieldId::new(2));
        let mut tx = PatchTx::new();

        let read_error = adapter
            .read_path(&stale_path)
            .expect_err("stale read should fail");
        assert_eq!(
            read_error.kind,
            HostErrorKind::StaleGeneration {
                expected: 2,
                actual: 3
            }
        );

        tx.set_path(stale_path, HostValue::Int(10), None)
            .expect("patch recording does not touch adapter");
        let apply_error = tx.apply(&mut adapter).expect_err("stale apply should fail");
        assert_eq!(
            apply_error.kind,
            HostErrorKind::StaleGeneration {
                expected: 2,
                actual: 3
            }
        );
    }

    #[test]
    fn call_method_patch_applies_at_safe_point() {
        let mut adapter = MockStateAdapter::new();
        let path = level_path();
        let method = HostMethodId::new(4);
        adapter.insert_value(path.clone(), HostValue::Int(9));
        adapter.insert_method_return(method, HostValue::Null);
        let mut tx = PatchTx::new();

        tx.call_method(
            path.clone(),
            method,
            vec![HostValue::String("gold".into())],
            None,
        )
        .expect("record method call");
        assert!(adapter.method_calls().is_empty());

        tx.apply(&mut adapter).expect("apply method call");

        assert_eq!(
            adapter.method_calls(),
            &[(path, method, vec![HostValue::String("gold".into())])]
        );
    }
}
