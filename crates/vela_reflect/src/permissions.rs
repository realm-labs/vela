use std::collections::BTreeSet;

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
}
