use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{ReflectError, ReflectErrorKind, ReflectResult};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReflectPermission {
    ReadTypeInfo,
    ReadValueFields,
    WriteValueFields,
    CallMethods,
    InspectHostPath,
}

impl ReflectPermission {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadTypeInfo => "reflect.read_type_info",
            Self::ReadValueFields => "reflect.read_value_fields",
            Self::WriteValueFields => "reflect.write_value_fields",
            Self::CallMethods => "reflect.call_methods",
            Self::InspectHostPath => "reflect.inspect_host_path",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReflectPermissionSet {
    permissions: BTreeSet<ReflectPermission>,
}

impl ReflectPermissionSet {
    #[must_use]
    pub fn new() -> Self {
        Self {
            permissions: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn all() -> Self {
        Self::new()
            .with(ReflectPermission::ReadTypeInfo)
            .with(ReflectPermission::ReadValueFields)
            .with(ReflectPermission::WriteValueFields)
            .with(ReflectPermission::CallMethods)
            .with(ReflectPermission::InspectHostPath)
    }

    #[must_use]
    pub fn read_only() -> Self {
        Self::new()
            .with(ReflectPermission::ReadTypeInfo)
            .with(ReflectPermission::ReadValueFields)
    }

    #[must_use]
    pub fn with(mut self, permission: ReflectPermission) -> Self {
        self.insert(permission);
        self
    }

    pub fn insert(&mut self, permission: ReflectPermission) {
        self.permissions.insert(permission);
    }

    #[must_use]
    pub fn contains(&self, permission: ReflectPermission) -> bool {
        self.permissions.contains(&permission)
    }

    pub fn require(&self, permission: ReflectPermission) -> ReflectResult<()> {
        if self.contains(permission) {
            Ok(())
        } else {
            Err(ReflectError::new(ReflectErrorKind::PermissionDenied {
                permission,
            }))
        }
    }
}

impl Default for ReflectPermissionSet {
    fn default() -> Self {
        Self::all()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReflectPolicy {
    permissions: ReflectPermissionSet,
    lookup_limit: Option<u64>,
}

impl ReflectPolicy {
    #[must_use]
    pub fn new(permissions: ReflectPermissionSet) -> Self {
        Self {
            permissions,
            lookup_limit: None,
        }
    }

    #[must_use]
    pub fn all() -> Self {
        Self::new(ReflectPermissionSet::all())
    }

    #[must_use]
    pub fn read_only() -> Self {
        Self::new(ReflectPermissionSet::read_only())
    }

    #[must_use]
    pub fn with_permissions(mut self, permissions: ReflectPermissionSet) -> Self {
        self.permissions = permissions;
        self
    }

    #[must_use]
    pub fn with_lookup_limit(mut self, limit: u64) -> Self {
        self.lookup_limit = Some(limit);
        self
    }

    #[must_use]
    pub fn permissions(&self) -> &ReflectPermissionSet {
        &self.permissions
    }

    #[must_use]
    pub const fn lookup_limit(&self) -> Option<u64> {
        self.lookup_limit
    }

    pub fn require(&self, permission: ReflectPermission) -> ReflectResult<()> {
        self.permissions.require(permission)
    }
}

impl Default for ReflectPolicy {
    fn default() -> Self {
        Self::all()
    }
}

#[derive(Debug)]
pub struct ReflectLookupBudget {
    limit: Option<u64>,
    remaining: AtomicU64,
}

impl ReflectLookupBudget {
    #[must_use]
    pub fn new(limit: Option<u64>) -> Self {
        Self {
            remaining: AtomicU64::new(limit.unwrap_or(u64::MAX)),
            limit,
        }
    }

    pub fn consume(&self) -> ReflectResult<()> {
        let Some(limit) = self.limit else {
            return Ok(());
        };
        self.remaining
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                remaining.checked_sub(1)
            })
            .map(|_| ())
            .map_err(|_| ReflectError::new(ReflectErrorKind::LookupBudgetExceeded { limit }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_sets_report_missing_permissions() {
        let permissions = ReflectPermissionSet::read_only();

        assert!(permissions.require(ReflectPermission::ReadTypeInfo).is_ok());
        let error = permissions
            .require(ReflectPermission::WriteValueFields)
            .expect_err("write should be denied");
        assert_eq!(
            error.kind,
            ReflectErrorKind::PermissionDenied {
                permission: ReflectPermission::WriteValueFields
            }
        );
    }

    #[test]
    fn lookup_budget_reports_exhaustion() {
        let budget = ReflectLookupBudget::new(Some(1));

        budget.consume().expect("first lookup");
        let error = budget.consume().expect_err("budget exhausted");
        assert_eq!(
            error.kind,
            ReflectErrorKind::LookupBudgetExceeded { limit: 1 }
        );
    }
}
