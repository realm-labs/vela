use std::collections::{BTreeMap, BTreeSet};

use vela_common::{HostMethodId, HostObjectId, HostTypeId};

use crate::{
    access::{HostAccess, HostObjectSnapshot},
    adapter::ScriptStateAdapter,
    add_values, div_values,
    error::{HostError, HostErrorKind, HostResult},
    mul_values,
    path::{HostPath, HostRef},
    rem_values,
    resolved::{HostAccessSpec, HostMutationOp, HostSchemaEpoch, ResolvedHostAccess},
    sub_values,
    target::{HostPathArgOwned, HostTargetInstance, HostTargetPlan},
    value::HostValue,
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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MockValueKey {
    pub root: HostRef,
    pub target: HostTargetPlan,
    pub args: Vec<HostPathArgOwned>,
}

impl MockValueKey {
    #[must_use]
    pub fn new(root: HostRef, target: HostTargetPlan, args: Vec<HostPathArgOwned>) -> Self {
        Self { root, target, args }
    }

    #[must_use]
    pub fn from_path(path: &HostPath) -> Self {
        Self::new(path.root, HostTargetPlan::from(path), Vec::new())
    }

    fn from_instance(target: HostTargetInstance<'_>) -> Self {
        Self::new(
            target.root,
            target.plan.clone(),
            target.args.iter().map(|arg| arg.to_owned_arg()).collect(),
        )
    }

    fn diagnostic_path(&self) -> HostPath {
        let args = self
            .args
            .iter()
            .map(crate::target::HostPathArg::from)
            .collect::<Vec<_>>();
        HostTargetInstance::new(self.root, &self.target, &args)
            .to_diagnostic_path()
            .to_host_path()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MockMethodCall {
    pub target: MockValueKey,
    pub method: HostMethodId,
    pub args: Vec<HostValue>,
}

impl MockMethodCall {
    #[must_use]
    pub fn diagnostic_path(&self) -> HostPath {
        self.target.diagnostic_path()
    }
}

impl PartialEq<(HostPath, HostMethodId, Vec<HostValue>)> for MockMethodCall {
    fn eq(&self, other: &(HostPath, HostMethodId, Vec<HostValue>)) -> bool {
        self.diagnostic_path() == other.0 && self.method == other.1 && self.args == other.2
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MockStateAdapter {
    objects: BTreeMap<HostObjectKey, u32>,
    values: BTreeMap<MockValueKey, HostValue>,
    method_returns: BTreeMap<HostMethodId, HostValue>,
    method_calls: Vec<MockMethodCall>,
    denied_reads: BTreeSet<MockValueKey>,
    denied_writes: BTreeSet<MockValueKey>,
    denied_calls: BTreeSet<MockValueKey>,
    schema_epoch: HostSchemaEpoch,
}

impl Default for MockStateAdapter {
    fn default() -> Self {
        Self {
            objects: BTreeMap::new(),
            values: BTreeMap::new(),
            method_returns: BTreeMap::new(),
            method_calls: Vec::new(),
            denied_reads: BTreeSet::new(),
            denied_writes: BTreeSet::new(),
            denied_calls: BTreeSet::new(),
            schema_epoch: HostSchemaEpoch::new(0),
        }
    }
}

impl MockStateAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_value(&mut self, path: HostPath, value: HostValue) {
        self.insert_value_key(MockValueKey::from_path(&path), value);
    }

    pub fn insert_value_key(&mut self, key: MockValueKey, value: HostValue) {
        self.insert_object(key.root);
        self.values.insert(key, value);
    }

    pub fn insert_object(&mut self, host_ref: HostRef) {
        self.objects
            .insert(HostObjectKey::from_ref(host_ref), host_ref.generation);
    }

    pub fn insert_method_return(&mut self, method: HostMethodId, value: HostValue) {
        self.method_returns.insert(method, value);
    }

    pub fn deny_read(&mut self, path: HostPath) {
        self.denied_reads.insert(MockValueKey::from_path(&path));
    }

    pub fn deny_write(&mut self, path: HostPath) {
        self.denied_writes.insert(MockValueKey::from_path(&path));
    }

    pub fn deny_call(&mut self, path: HostPath) {
        self.denied_calls.insert(MockValueKey::from_path(&path));
    }

    #[must_use]
    pub fn method_calls(&self) -> &[MockMethodCall] {
        &self.method_calls
    }

    pub fn read_path(&self, path: &HostPath) -> HostResult<HostValue> {
        let plan = HostTargetPlan::from(path);
        let target = HostTargetInstance::new(path.root, &plan, &[]);
        let access = ResolvedHostAccess::generic_path(self.schema_epoch);
        self.read_host(access, target)
    }

    fn validate_key(&self, key: &MockValueKey) -> HostResult<()> {
        self.validate_root(key, false)
    }

    fn validate_writable_key(&self, key: &MockValueKey) -> HostResult<()> {
        self.validate_root(key, true)
    }

    fn validate_root(&self, key: &MockValueKey, allow_unknown: bool) -> HostResult<()> {
        let object = HostObjectKey::from_ref(key.root);
        let Some(generation) = self.objects.get(&object).copied() else {
            if allow_unknown {
                return Ok(());
            }
            return Err(HostError::new(HostErrorKind::MissingPath {
                path: key.diagnostic_path(),
            }));
        };
        let snapshot = HostObjectSnapshot {
            type_id: key.root.type_id,
            object_id: key.root.object_id,
            generation,
        };
        HostAccess::require_fresh_ref(key.root, &snapshot)
    }

    fn ensure_object(&mut self, host_ref: HostRef) {
        self.objects
            .entry(HostObjectKey::from_ref(host_ref))
            .or_insert(host_ref.generation);
    }

    fn validate_access(&self, key: &MockValueKey, action: &'static str) -> HostResult<()> {
        let denied = match action {
            "read" => self.denied_reads.contains(key),
            "write" => self.denied_writes.contains(key),
            "call" => self.denied_calls.contains(key),
            _ => false,
        };
        if denied {
            Err(HostError::new(HostErrorKind::PermissionDenied {
                path: key.diagnostic_path(),
                action,
            }))
        } else {
            Ok(())
        }
    }

    fn invalid_mutation_error(op: HostMutationOp, path: HostPath) -> HostError {
        match op {
            HostMutationOp::Add => HostError::new(HostErrorKind::InvalidAdd { path }),
            HostMutationOp::Sub => HostError::new(HostErrorKind::InvalidSub { path }),
            HostMutationOp::Mul => HostError::new(HostErrorKind::InvalidMul { path }),
            HostMutationOp::Div => HostError::new(HostErrorKind::InvalidDiv { path }),
            HostMutationOp::Rem => HostError::new(HostErrorKind::InvalidRem { path }),
            HostMutationOp::Push => HostError::new(HostErrorKind::InvalidPush { path }),
        }
    }
}

impl ScriptStateAdapter for MockStateAdapter {
    fn host_schema_epoch(&self) -> HostSchemaEpoch {
        self.schema_epoch
    }

    fn resolve_host_access(&self, _spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess> {
        Ok(ResolvedHostAccess::generic_path(self.schema_epoch))
    }

    fn read_host(
        &self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        let key = MockValueKey::from_instance(target);
        self.validate_key(&key)?;
        self.validate_access(&key, "read")?;
        self.values.get(&key).cloned().ok_or_else(|| {
            HostError::new(HostErrorKind::MissingPath {
                path: key.diagnostic_path(),
            })
        })
    }

    fn write_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        let key = MockValueKey::from_instance(target);
        self.validate_access(&key, "write")?;
        self.validate_writable_key(&key)?;
        self.ensure_object(key.root);
        self.values.insert(key, value);
        Ok(())
    }

    fn mutate_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()> {
        let key = MockValueKey::from_instance(target);
        self.validate_access(&key, "write")?;
        self.validate_key(&key)?;
        let current = self.values.get(&key).cloned().ok_or_else(|| {
            HostError::new(HostErrorKind::MissingPath {
                path: key.diagnostic_path(),
            })
        })?;
        let next = match op {
            HostMutationOp::Add => add_values(&current, &rhs),
            HostMutationOp::Sub => sub_values(&current, &rhs),
            HostMutationOp::Mul => mul_values(&current, &rhs),
            HostMutationOp::Div => div_values(&current, &rhs),
            HostMutationOp::Rem => rem_values(&current, &rhs),
            HostMutationOp::Push => None,
        }
        .ok_or_else(|| Self::invalid_mutation_error(op, key.diagnostic_path()))?;
        self.values.insert(key, next);
        Ok(())
    }

    fn remove_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
    ) -> HostResult<()> {
        let key = MockValueKey::from_instance(target);
        self.validate_access(&key, "write")?;
        self.validate_key(&key)?;
        if self.values.remove(&key).is_some() {
            Ok(())
        } else {
            Err(HostError::new(HostErrorKind::MissingPath {
                path: key.diagnostic_path(),
            }))
        }
    }

    fn call_host(
        &mut self,
        _access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        let key = MockValueKey::from_instance(target);
        self.validate_access(&key, "call")?;
        self.validate_writable_key(&key)?;
        let value = self
            .method_returns
            .get(&method)
            .cloned()
            .unwrap_or(HostValue::Null);
        self.ensure_object(key.root);
        self.method_calls.push(MockMethodCall {
            target: key,
            method,
            args: args.to_vec(),
        });
        Ok(value)
    }
}
