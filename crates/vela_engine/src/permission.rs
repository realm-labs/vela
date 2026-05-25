use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PermissionSet {
    permissions: BTreeSet<String>,
}

impl PermissionSet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn gameplay() -> Self {
        Self::new().with(crate::clock::CONTEXT_TIME_PERMISSION)
    }

    #[must_use]
    pub fn with(mut self, permission: impl Into<String>) -> Self {
        self.insert(permission);
        self
    }

    pub fn insert(&mut self, permission: impl Into<String>) {
        self.permissions.insert(permission.into());
    }

    #[must_use]
    pub fn contains(&self, permission: &str) -> bool {
        self.permissions.contains(permission)
    }

    #[must_use]
    pub fn missing_required<'a>(&self, required_permissions: &'a PermissionSet) -> Option<&'a str> {
        required_permissions
            .iter()
            .find(|permission| !self.contains(permission))
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.permissions.iter().map(String::as_str)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.permissions.is_empty()
    }
}
