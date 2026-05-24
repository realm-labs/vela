//! Host reference, path, and patch transaction model.

use std::collections::BTreeMap;
use std::fmt;

use vela_common::{FieldId, HostObjectId, HostTypeId, MethodId, Span, Symbol};

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
        method: MethodId,
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
    InvalidAdd {
        path: HostPath,
    },
}

pub type HostResult<T> = Result<T, HostError>;

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
}
