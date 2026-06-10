use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompilerOptions {
    pub(super) host_index_capabilities: HashMap<String, HostIndexCapabilityInfo>,
    pub(super) native_module_roots: HashSet<String>,
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

    pub(super) fn is_native_module_root(&self, root: &str) -> bool {
        self.native_module_roots.contains(root)
    }

    #[must_use]
    pub fn host_index_capability(&self, type_name: &str) -> Option<&HostIndexCapabilityInfo> {
        self.host_index_capabilities.get(type_name)
    }
}
