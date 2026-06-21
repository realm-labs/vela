use std::collections::BTreeMap;

use crate::type_fact::TypeFact;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExprFactScope {
    paths: BTreeMap<Vec<String>, TypeFact>,
}

impl ExprFactScope {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_path(
        mut self,
        path: impl IntoIterator<Item = impl Into<String>>,
        fact: TypeFact,
    ) -> Self {
        self.insert_path(path, fact);
        self
    }

    pub fn insert_path(
        &mut self,
        path: impl IntoIterator<Item = impl Into<String>>,
        fact: TypeFact,
    ) {
        self.paths
            .insert(path.into_iter().map(Into::into).collect(), fact);
    }

    #[must_use]
    pub fn path_fact(&self, path: &[String]) -> Option<&TypeFact> {
        self.paths.get(path)
    }
}
