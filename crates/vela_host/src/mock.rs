use std::collections::{BTreeMap, BTreeSet};

use vela_common::{HostMethodId, HostObjectId, HostTypeId};

use crate::{
    HostError, HostErrorKind, HostObjectSnapshot, HostPath, HostRef, HostResult, HostValue, Patch,
    PatchOp, PatchTx, ScriptStateAdapter, add_values, push_value, sub_values,
};

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MockStateAdapter {
    objects: BTreeMap<HostObjectKey, u32>,
    values: BTreeMap<HostPath, HostValue>,
    method_returns: BTreeMap<HostMethodId, HostValue>,
    method_calls: Vec<(HostPath, HostMethodId, Vec<HostValue>)>,
    denied_reads: BTreeSet<HostPath>,
    denied_writes: BTreeSet<HostPath>,
    denied_calls: BTreeSet<HostPath>,
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

    pub fn deny_read(&mut self, path: HostPath) {
        self.denied_reads.insert(path);
    }

    pub fn deny_write(&mut self, path: HostPath) {
        self.denied_writes.insert(path);
    }

    pub fn deny_call(&mut self, path: HostPath) {
        self.denied_calls.insert(path);
    }

    #[must_use]
    pub fn method_calls(&self) -> &[(HostPath, HostMethodId, Vec<HostValue>)] {
        &self.method_calls
    }

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

    fn validate_access(&self, path: &HostPath, action: &'static str) -> HostResult<()> {
        let denied = match action {
            "read" => self.denied_reads.contains(path),
            "write" => self.denied_writes.contains(path),
            "call" => self.denied_calls.contains(path),
            _ => false,
        };
        if denied {
            Err(HostError::new(HostErrorKind::PermissionDenied {
                path: path.clone(),
                action,
            }))
        } else {
            Ok(())
        }
    }

    fn validate_expected_base(&self, patch: &Patch) -> HostResult<()> {
        let Some(expected) = &patch.expected_base else {
            return Ok(());
        };
        let actual = self.values.get(&patch.path);
        if actual == Some(expected) {
            Ok(())
        } else {
            Err(HostError::new(HostErrorKind::PatchConflict {
                path: patch.path.clone(),
                expected: Box::new(expected.clone()),
                actual: actual.cloned().map(Box::new),
            }))
        }
    }
}

impl ScriptStateAdapter for MockStateAdapter {
    fn read_path(&self, path: &HostPath) -> HostResult<HostValue> {
        self.validate_path(path)?;
        self.validate_access(path, "read")?;
        self.values
            .get(path)
            .cloned()
            .ok_or_else(|| HostError::new(HostErrorKind::MissingPath { path: path.clone() }))
    }

    fn write_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()> {
        self.validate_path(path)?;
        self.validate_access(path, "write")?;
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
        self.validate_access(path, "call")?;
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
        let result = self
            .validate_path(&patch.path)
            .and_then(|()| match &patch.op {
                PatchOp::Set(_)
                | PatchOp::Add(_)
                | PatchOp::Sub(_)
                | PatchOp::Push(_)
                | PatchOp::Remove => self.validate_access(&patch.path, "write"),
                PatchOp::CallHostMethod { method, .. }
                    if self.method_returns.contains_key(method) =>
                {
                    self.validate_access(&patch.path, "call")
                }
                PatchOp::CallHostMethod { method, .. } => {
                    Err(HostError::new(HostErrorKind::UnsupportedMethod {
                        method: *method,
                    }))
                }
            })
            .and_then(|()| self.validate_expected_base(patch));
        result.map_err(|error| error.with_source_span_if_absent(patch.source_span))
    }

    fn apply_patch(&mut self, patch: Patch) -> HostResult<()> {
        let source_span = patch.source_span;
        let result = (|| {
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
                PatchOp::Sub(value) => {
                    let current = self.read_path(&patch.path)?;
                    let next = sub_values(&current, &value).ok_or_else(|| {
                        HostError::new(HostErrorKind::InvalidSub {
                            path: patch.path.clone(),
                        })
                    })?;
                    self.write_path(&patch.path, next)
                }
                PatchOp::Remove => {
                    self.read_path(&patch.path)?;
                    self.values.remove(&patch.path);
                    Ok(())
                }
                PatchOp::Push(value) => {
                    let current = self.read_path(&patch.path)?;
                    let next = push_value(&current, value).ok_or_else(|| {
                        HostError::new(HostErrorKind::InvalidPush {
                            path: patch.path.clone(),
                        })
                    })?;
                    self.write_path(&patch.path, next)
                }
                PatchOp::CallHostMethod { method, args } => {
                    self.call_method(&patch.path, method, &args).map(|_| ())
                }
            }
        })();
        result.map_err(|error| error.with_source_span_if_absent(source_span))
    }

    fn apply_patches(&mut self, patches: Vec<Patch>) -> HostResult<()> {
        for patch in &patches {
            self.validate_patch(patch)?;
        }

        let snapshot = self.clone();
        for patch in patches {
            if let Err(error) = self.apply_patch(patch) {
                *self = snapshot;
                return Err(error);
            }
        }
        Ok(())
    }
}
