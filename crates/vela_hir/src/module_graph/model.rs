use std::collections::{BTreeMap, btree_map::Entry};

use vela_common::{SourceId, Span};
use vela_syntax::ast::Visibility;

use crate::ids::{HirDeclId, HirNodeId, ModuleId};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModulePath(Vec<String>);

impl ModulePath {
    #[must_use]
    pub fn new(segments: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(segments.into_iter().map(Into::into).collect())
    }

    #[must_use]
    pub fn from_qualified(path: &str) -> Self {
        Self::new(path.split("::").filter(|segment| !segment.is_empty()))
    }

    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.0
    }

    #[must_use]
    pub fn join(&self) -> String {
        self.0.join("::")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleSource {
    pub id: SourceId,
    pub path: ModulePath,
    pub text: String,
}

impl ModuleSource {
    #[must_use]
    pub fn new(id: SourceId, path: ModulePath, text: impl Into<String>) -> Self {
        Self {
            id,
            path,
            text: text.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    pub id: HirDeclId,
    pub node: HirNodeId,
    pub module: ModuleId,
    pub name: String,
    pub kind: DeclarationKind,
    pub visibility: Visibility,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationKind {
    Const,
    Global,
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Import {
    pub module: ModuleId,
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub span: Span,
    pub resolution: Option<ImportResolution>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportResolution {
    Declaration(HirDeclId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImport {
    pub path: Vec<String>,
    pub resolution: ImportResolution,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeclarationIndex {
    by_name: BTreeMap<String, HirDeclId>,
}

impl DeclarationIndex {
    #[must_use]
    pub fn get(&self, name: &str) -> Option<HirDeclId> {
        self.by_name.get(name).copied()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(String::as_str)
    }

    pub(super) fn insert(&mut self, name: String, id: HirDeclId) -> Option<HirDeclId> {
        match self.by_name.entry(name) {
            Entry::Vacant(entry) => {
                entry.insert(id);
                None
            }
            Entry::Occupied(entry) => Some(*entry.get()),
        }
    }
}
