use std::collections::{BTreeMap, btree_map::Entry};

use vela_common::{SourceId, Span};
use vela_package::{ModulePath, PackageId};

use crate::ids::{HirDeclId, HirNodeId, ModuleId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleSource {
    pub id: SourceId,
    pub package: PackageId,
    pub path: ModulePath,
    pub text: String,
}

impl ModuleSource {
    #[must_use]
    pub fn new(
        id: SourceId,
        package: PackageId,
        path: ModulePath,
        text: impl Into<String>,
    ) -> Self {
        Self {
            id,
            package,
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
    pub name_span: Span,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Visibility {
    Private,
    Public,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeclarationKind {
    Const,
    State,
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
    pub path_spans: Vec<Span>,
    pub alias: Option<String>,
    pub alias_span: Option<Span>,
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
