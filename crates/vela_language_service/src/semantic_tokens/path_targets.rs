//! Scoped source ownership shared by import and value-path projections.
use std::collections::BTreeSet;

use vela_hir::{
    ids::ModuleId,
    module_graph::{DeclarationKind, ImportResolution, Visibility},
};

use super::{
    SemanticTokenClassification as C, SemanticTokenModifiers as M, SemanticTokenType as T,
    declaration_use_classification,
};
use crate::LanguageServiceDatabases;

pub(super) struct Targets<'a> {
    db: &'a LanguageServiceDatabases,
    module: ModuleId,
    builtin_names: BTreeSet<&'static str>,
}

impl<'a> Targets<'a> {
    pub(super) fn new(db: &'a LanguageServiceDatabases, module: ModuleId) -> Self {
        Self {
            db,
            module,
            builtin_names: vela_analysis::stdlib::stdlib_function_completion_facts()
                .into_iter()
                .map(|fact| fact.name)
                .collect(),
        }
    }

    fn source(&self, path: &[String]) -> Option<C> {
        let graph = self.db.hir_db().graph();
        let current = graph.module_key(self.module)?;
        for kind in [
            DeclarationKind::Function,
            DeclarationKind::Const,
            DeclarationKind::State,
            DeclarationKind::Struct,
            DeclarationKind::Enum,
            DeclarationKind::Trait,
        ] {
            if let Some(declaration) = graph.declaration_by_type_path(path, current, kind) {
                return Some(
                    if declaration.module == self.module
                        || declaration.visibility == Visibility::Public
                    {
                        declaration_use_classification(declaration)
                    } else {
                        C::new(T::UnresolvedReference, M::UNRESOLVED)
                    },
                );
            }
        }
        // A source declaration owns its descendants even when the requested
        // member is absent. Registry metadata must not repair an invalid import.
        for length in (1..path.len()).rev() {
            let parent = &path[..length];
            for kind in [
                DeclarationKind::Function,
                DeclarationKind::Const,
                DeclarationKind::State,
                DeclarationKind::Struct,
                DeclarationKind::Enum,
                DeclarationKind::Trait,
            ] {
                if let Some(declaration) = graph.declaration_by_type_path(parent, current, kind) {
                    let visible = declaration.module == self.module
                        || declaration.visibility == Visibility::Public;
                    let variant = visible
                        && kind == DeclarationKind::Enum
                        && length + 1 == path.len()
                        && graph.enum_shape(declaration.id).is_some_and(|shape| {
                            shape
                                .variants
                                .iter()
                                .any(|entry| entry.name == path[length])
                        });
                    return Some(if variant {
                        C::new(T::EnumMember, M::SOURCE)
                    } else {
                        C::new(T::UnresolvedReference, M::UNRESOLVED)
                    });
                }
            }
        }
        None
    }

    fn function(&self, path: &[String]) -> Option<C> {
        let name = path.join("::");
        if self.builtin_names.contains(name.as_str()) {
            return Some(C::new(T::Function, M::BUILTIN));
        }
        self.db
            .schema_db()
            .facts()
            .function_fact(&name)
            .map(|_| C::new(T::Function, M::HOST.union(M::SCHEMA)))
    }

    pub(super) fn value(&self, path: &[String]) -> Option<C> {
        self.source(path).or_else(|| self.function(path))
    }

    pub(super) fn import(&self, path: &[String], resolution: Option<ImportResolution>) -> C {
        let graph = self.db.hir_db().graph();
        if let Some(ImportResolution::Declaration(id)) = resolution
            && let Some(declaration) = graph.declaration(id)
        {
            return declaration_use_classification(declaration);
        }
        if let Some(target) = self.source(path) {
            return target;
        }
        if let Some(current) = graph.module_key(self.module)
            && let Some(key) = graph.resolve_module_path(current, path)
            && (graph.module_id(&key).is_some() || !graph.module_child_segments(&key).is_empty())
        {
            return C::new(T::Module, M::SOURCE);
        }
        if let Some(target) = self.function(path) {
            return target;
        }
        let name = path.join("::");
        if self.builtin_names.iter().any(|function| {
            function
                .strip_prefix(&name)
                .is_some_and(|tail| tail.starts_with("::"))
        }) {
            return C::new(T::Module, M::BUILTIN);
        }
        let schema = self.db.schema_db().facts();
        let kind = if schema.type_fact(&name).is_some() {
            Some(T::Type)
        } else if schema.trait_fact(&name).is_some() {
            Some(T::Interface)
        } else if schema.module_fact(&name).is_some() {
            Some(T::Module)
        } else {
            None
        };
        kind.map_or(C::new(T::UnresolvedReference, M::UNRESOLVED), |kind| {
            C::new(kind, M::HOST.union(M::SCHEMA))
        })
    }
}
