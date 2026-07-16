use std::collections::{HashMap, HashSet};

use vela_common::HostMethodId;
use vela_def::FunctionId;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompilerOptions {
    pub(super) host_index_capabilities: HashMap<String, HostIndexCapabilityInfo>,
    pub(super) native_module_roots: HashSet<String>,
    pub(super) opaque_external_type_hints: HashSet<String>,
    pub(super) scoped_borrow_functions: HashSet<FunctionId>,
    pub(super) scoped_borrow_methods: HashSet<HostMethodId>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HostIndexCapabilityInfo {
    pub readable: bool,
    pub writable: bool,
    pub addable: bool,
    pub removable: bool,
    pub key_type: Option<String>,
    pub value_type: Option<String>,
}

impl CompilerOptions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_host_index_capability(
        mut self,
        type_name: impl Into<String>,
        capability: HostIndexCapabilityInfo,
    ) -> Self {
        self.host_index_capabilities
            .insert(type_name.into(), capability);
        self
    }

    #[must_use]
    pub fn with_native_module_root(mut self, root: impl Into<String>) -> Self {
        self.native_module_roots.insert(root.into());
        self
    }

    /// Allows an engine-owned native signature to name an opaque boundary type.
    #[must_use]
    pub fn with_opaque_external_type_hint(mut self, name: impl Into<String>) -> Self {
        self.opaque_external_type_hints.insert(name.into());
        self
    }

    #[must_use]
    pub fn host_index_capability(&self, type_name: &str) -> Option<&HostIndexCapabilityInfo> {
        self.host_index_capabilities.get(type_name)
    }

    #[must_use]
    pub fn with_scoped_borrow_function(mut self, function: FunctionId) -> Self {
        self.scoped_borrow_functions.insert(function);
        self
    }

    #[must_use]
    pub fn with_scoped_borrow_method(mut self, method: HostMethodId) -> Self {
        self.scoped_borrow_methods.insert(method);
        self
    }

    pub(super) fn is_scoped_borrow_function(&self, function: FunctionId) -> bool {
        self.scoped_borrow_functions.contains(&function)
    }

    pub(super) fn is_scoped_borrow_method(&self, method: HostMethodId) -> bool {
        self.scoped_borrow_methods.contains(&method)
    }

    pub(super) fn allows_opaque_external_type_hint(&self, name: &str) -> bool {
        self.opaque_external_type_hints.contains(name)
    }
}
