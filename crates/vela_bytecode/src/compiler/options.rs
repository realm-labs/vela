use std::collections::HashSet;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompilerOptions {
    pub(super) native_module_roots: HashSet<String>,
}

impl CompilerOptions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_native_module_root(mut self, root: impl Into<String>) -> Self {
        self.native_module_roots.insert(root.into());
        self
    }
}
