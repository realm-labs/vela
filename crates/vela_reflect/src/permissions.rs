use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::candidates::name_candidates;
use crate::{FieldDesc, FunctionDesc, MethodDesc, ReflectError, ReflectErrorKind, ReflectResult};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReflectPermission {
    ReadTypeInfo,
    ReadValueFields,
    WriteValueFields,
    CallMethods,
    CallHostReadMethods,
    CallHostWriteMethods,
    CallEventMethods,
    AccessPrivate,
    InspectHostPath,
}

impl ReflectPermission {
    pub const ALL: &'static [Self] = &[
        Self::ReadTypeInfo,
        Self::ReadValueFields,
        Self::WriteValueFields,
        Self::CallMethods,
        Self::CallHostReadMethods,
        Self::CallHostWriteMethods,
        Self::CallEventMethods,
        Self::AccessPrivate,
        Self::InspectHostPath,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadTypeInfo => "reflect.read_type_info",
            Self::ReadValueFields => "reflect.read_value_fields",
            Self::WriteValueFields => "reflect.write_value_fields",
            Self::CallMethods => "reflect.call_methods",
            Self::CallHostReadMethods => "reflect.call_host_read_methods",
            Self::CallHostWriteMethods => "reflect.call_host_write_methods",
            Self::CallEventMethods => "reflect.call_event_methods",
            Self::AccessPrivate => "reflect.access_private",
            Self::InspectHostPath => "reflect.inspect_host_path",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|permission| permission.as_str() == name)
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
        ReflectPermission::ALL
            .iter()
            .copied()
            .fold(Self::new(), Self::with)
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

    pub fn iter(&self) -> impl Iterator<Item = ReflectPermission> + '_ {
        self.permissions.iter().copied()
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

#[must_use]
pub fn permission_names(policy: &ReflectPolicy) -> Vec<&'static str> {
    policy
        .permissions()
        .iter()
        .map(ReflectPermission::as_str)
        .collect()
}

pub fn has_permission(policy: &ReflectPolicy, permission: &str) -> ReflectResult<bool> {
    let Some(permission) = ReflectPermission::from_name(permission) else {
        return Err(ReflectError::new(ReflectErrorKind::UnknownPermission {
            permission: permission.to_owned(),
            candidates: name_candidates(
                permission,
                ReflectPermission::ALL
                    .iter()
                    .map(|permission| permission.as_str()),
            ),
        }));
    };
    Ok(policy.permissions().contains(permission))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReflectPolicy {
    permissions: ReflectPermissionSet,
    lookup_limit: Option<u64>,
    field_permissions: BTreeSet<String>,
    method_permissions: BTreeSet<String>,
    function_permissions: BTreeSet<String>,
}

impl ReflectPolicy {
    #[must_use]
    pub fn new(permissions: ReflectPermissionSet) -> Self {
        Self {
            permissions,
            lookup_limit: None,
            field_permissions: BTreeSet::new(),
            method_permissions: BTreeSet::new(),
            function_permissions: BTreeSet::new(),
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
    pub fn with_field_permission(mut self, permission: impl Into<String>) -> Self {
        self.field_permissions.insert(permission.into());
        self
    }

    #[must_use]
    pub fn with_field_permissions<'a>(
        mut self,
        permissions: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        self.field_permissions
            .extend(permissions.into_iter().map(str::to_owned));
        self
    }

    #[must_use]
    pub fn with_method_permission(mut self, permission: impl Into<String>) -> Self {
        self.method_permissions.insert(permission.into());
        self
    }

    #[must_use]
    pub fn with_method_permissions<'a>(
        mut self,
        permissions: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        self.method_permissions
            .extend(permissions.into_iter().map(str::to_owned));
        self
    }

    #[must_use]
    pub fn with_function_permission(mut self, permission: impl Into<String>) -> Self {
        self.function_permissions.insert(permission.into());
        self
    }

    #[must_use]
    pub fn with_function_permissions<'a>(
        mut self,
        permissions: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        self.function_permissions
            .extend(permissions.into_iter().map(str::to_owned));
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

    pub fn require_function_access(&self, function: &FunctionDesc) -> ReflectResult<()> {
        if !function.access.reflect_visible {
            return Err(ReflectError::new(
                ReflectErrorKind::FunctionNotReflectVisible {
                    function: function.name.clone(),
                },
            ));
        }
        if !function.access.public {
            self.require(ReflectPermission::AccessPrivate)?;
        }
        if let Some(permission) = function
            .access
            .required_permissions()
            .iter()
            .find(|permission| !self.function_permissions.contains(permission.as_str()))
        {
            return Err(ReflectError::new(
                ReflectErrorKind::FunctionPermissionDenied {
                    function: function.name.clone(),
                    permission: permission.clone(),
                },
            ));
        }
        Ok(())
    }

    pub fn require_function_call_access(&self, function: &FunctionDesc) -> ReflectResult<()> {
        self.require_function_access(function)?;
        if !function.access.reflect_callable {
            return Err(ReflectError::new(
                ReflectErrorKind::FunctionNotReflectCallable {
                    function: function.name.clone(),
                },
            ));
        }
        if let Some(permission) = missing_function_effect_permission(function, &self.permissions) {
            return Err(ReflectError::new(
                ReflectErrorKind::FunctionEffectPermissionDenied {
                    function: function.name.clone(),
                    permission,
                },
            ));
        }
        Ok(())
    }

    pub fn require_field_read_access(
        &self,
        type_name: &str,
        field: &FieldDesc,
    ) -> ReflectResult<()> {
        if !field.access.reflect_readable {
            return Err(ReflectError::new(
                ReflectErrorKind::FieldNotReflectReadable {
                    type_name: type_name.to_owned(),
                    field: field.name.clone(),
                },
            ));
        }
        self.require_field_permissions(type_name, field)
    }

    pub fn require_field_write_access(
        &self,
        type_name: &str,
        field: &FieldDesc,
    ) -> ReflectResult<()> {
        if !field.access.reflect_writable {
            return Err(ReflectError::new(
                ReflectErrorKind::FieldNotReflectWritable {
                    type_name: type_name.to_owned(),
                    field: field.name.clone(),
                },
            ));
        }
        self.require_field_permissions(type_name, field)
    }

    pub fn require_method_access(&self, type_name: &str, method: &MethodDesc) -> ReflectResult<()> {
        if !method.access.reflect_callable {
            return Err(ReflectError::new(
                ReflectErrorKind::MethodNotReflectCallable {
                    type_name: type_name.to_owned(),
                    method: method.name.clone(),
                },
            ));
        }
        if !method.access.public {
            self.require(ReflectPermission::AccessPrivate)?;
        }
        if let Some(permission) = method
            .access
            .required_permissions()
            .iter()
            .find(|permission| !self.method_permissions.contains(permission.as_str()))
        {
            return Err(ReflectError::new(
                ReflectErrorKind::MethodPermissionDenied {
                    method: method.name.clone(),
                    permission: permission.clone(),
                },
            ));
        }
        if let Some(permission) = missing_method_effect_permission(method, &self.permissions) {
            return Err(ReflectError::new(
                ReflectErrorKind::MethodEffectPermissionDenied {
                    method: method.name.clone(),
                    permission,
                },
            ));
        }
        Ok(())
    }

    pub fn require_field_permissions(
        &self,
        type_name: &str,
        field: &FieldDesc,
    ) -> ReflectResult<()> {
        if let Some(permission) = field
            .access
            .required_permissions()
            .iter()
            .find(|permission| !self.field_permissions.contains(permission.as_str()))
        {
            return Err(ReflectError::new(ReflectErrorKind::FieldPermissionDenied {
                type_name: type_name.to_owned(),
                field: field.name.clone(),
                permission: permission.clone(),
            }));
        }
        Ok(())
    }
}

fn missing_method_effect_permission(
    method: &MethodDesc,
    permissions: &ReflectPermissionSet,
) -> Option<ReflectPermission> {
    if method.effects.reads_host && !permissions.contains(ReflectPermission::CallHostReadMethods) {
        return Some(ReflectPermission::CallHostReadMethods);
    }
    if method.effects.writes_host && !permissions.contains(ReflectPermission::CallHostWriteMethods)
    {
        return Some(ReflectPermission::CallHostWriteMethods);
    }
    if method.effects.emits_events && !permissions.contains(ReflectPermission::CallEventMethods) {
        return Some(ReflectPermission::CallEventMethods);
    }
    None
}

fn missing_function_effect_permission(
    function: &FunctionDesc,
    permissions: &ReflectPermissionSet,
) -> Option<ReflectPermission> {
    if function.effects.reads_host && !permissions.contains(ReflectPermission::CallHostReadMethods)
    {
        return Some(ReflectPermission::CallHostReadMethods);
    }
    if function.effects.writes_host
        && !permissions.contains(ReflectPermission::CallHostWriteMethods)
    {
        return Some(ReflectPermission::CallHostWriteMethods);
    }
    if function.effects.emits_events && !permissions.contains(ReflectPermission::CallEventMethods) {
        return Some(ReflectPermission::CallEventMethods);
    }
    None
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
        assert!(
            permissions
                .require(ReflectPermission::CallHostWriteMethods)
                .is_err()
        );
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
    fn permission_metadata_reports_names_and_unknown_candidates() {
        let policy = ReflectPolicy::new(
            ReflectPermissionSet::new()
                .with(ReflectPermission::ReadTypeInfo)
                .with(ReflectPermission::InspectHostPath),
        );

        assert_eq!(
            permission_names(&policy),
            vec!["reflect.read_type_info", "reflect.inspect_host_path"]
        );
        assert_eq!(
            has_permission(&policy, "reflect.inspect_host_path"),
            Ok(true)
        );
        assert_eq!(
            has_permission(&policy, "reflect.write_value_fields"),
            Ok(false)
        );
        let error =
            has_permission(&policy, "reflect.inspect_host").expect_err("unknown permission");
        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownPermission {
                permission: "reflect.inspect_host".to_owned(),
                candidates: vec![
                    "reflect.inspect_host_path".to_owned(),
                    "reflect.call_methods".to_owned(),
                    "reflect.access_private".to_owned()
                ]
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
