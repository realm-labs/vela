use super::QueryContext;

impl QueryContext<'_> {
    /// Expand one visible import, preserving declarations and rejecting an
    /// ambiguous binding or a local that owns the path's first segment.
    pub(crate) fn expand_import_path(&self, path: &[String]) -> Option<Vec<String>> {
        let first = path.first()?;
        if self
            .local_bindings_before_cursor()
            .any(|binding| binding.name == *first)
        {
            return None;
        }
        let graph = self.graph?;
        let module = graph.module_id(self.module_key()?)?;
        graph.expand_import_path(module, path)
    }
}
