use std::collections::{HashMap, HashSet};

use vela_common::{FieldId, HostMethodId};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompilerOptions {
    pub(super) host_fields: HashMap<String, FieldId>,
    pub(super) host_variant_fields: HashMap<String, FieldId>,
    pub(super) host_methods: HashMap<String, HostMethodId>,
    pub(super) host_methods_by_type: HashMap<(String, String), HostMethodId>,
    pub(super) host_types: HashSet<String>,
    pub(super) native_module_roots: HashSet<String>,
}

impl CompilerOptions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_host_field(mut self, name: impl Into<String>, field: FieldId) -> Self {
        self.host_fields.insert(name.into(), field);
        self
    }

    #[must_use]
    pub fn with_host_variant_field(mut self, name: impl Into<String>, field: FieldId) -> Self {
        self.host_variant_fields.insert(name.into(), field);
        self
    }

    #[must_use]
    pub fn with_host_method(mut self, name: impl Into<String>, method: HostMethodId) -> Self {
        self.host_methods.insert(name.into(), method);
        self
    }

    #[must_use]
    pub fn with_host_type(mut self, type_name: impl Into<String>) -> Self {
        self.host_types.insert(type_name.into());
        self
    }

    #[must_use]
    pub fn with_native_module_root(mut self, root: impl Into<String>) -> Self {
        self.native_module_roots.insert(root.into());
        self
    }

    #[must_use]
    pub fn with_host_method_for_type(
        mut self,
        type_name: impl Into<String>,
        name: impl Into<String>,
        method: HostMethodId,
    ) -> Self {
        let type_name = type_name.into();
        self.host_types.insert(type_name.clone());
        self.host_methods_by_type
            .insert((type_name, name.into()), method);
        self
    }

    pub(super) fn host_method(
        &self,
        receiver_type: Option<&str>,
        name: &str,
    ) -> Option<HostMethodId> {
        receiver_type
            .and_then(|type_name| {
                self.host_methods_by_type
                    .get(&(type_name.to_owned(), name.to_owned()))
            })
            .copied()
            .or_else(|| self.host_methods.get(name).copied())
    }

    pub(super) fn is_native_module_root(&self, root: &str) -> bool {
        self.native_module_roots.contains(root)
    }
}
