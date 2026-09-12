use std::collections::BTreeMap;

use vela_analysis::completion::type_completions;
use vela_hir::module_graph::DeclarationKind;
use vela_package::{ModuleKey, ModulePath};

use crate::{LanguageServiceDatabases, QueryContext};

use super::{CompletionItem, CompletionKind, imports::ImportScope};

pub(super) struct TypePaths<'a, 'db> {
    pub scope: ImportScope<'a, 'db>,
    pub paths: BTreeMap<String, String>,
    databases: &'a LanguageServiceDatabases,
    current: &'a ModuleKey,
}

impl<'a, 'db> TypePaths<'a, 'db> {
    pub fn new(
        databases: &'a LanguageServiceDatabases,
        query: &'a QueryContext<'db>,
    ) -> Option<Self> {
        let current = query.module_key()?;
        let graph = databases.hir_db().graph();
        let mut paths = type_completions(databases.schema_db().facts())
            .into_iter()
            .map(|item| (item.label.clone(), item.label))
            .collect::<BTreeMap<_, _>>();
        for declaration in graph.declarations_by_name_prefix("") {
            if !matches!(
                declaration.kind,
                DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait
            ) {
                continue;
            }
            let owner = graph.module_key(declaration.module)?;
            let canonical = graph.qualified_declaration_name(declaration.id)?;
            let address = if owner.package == current.package {
                let path = canonical.split("::").map(str::to_owned).collect::<Vec<_>>();
                if graph
                    .expand_import_path(graph.module_id(current)?, &path)
                    .as_ref()
                    == Some(&path)
                {
                    canonical.clone()
                } else {
                    format!("crate::{canonical}")
                }
            } else {
                let Some(alias) =
                    graph
                        .dependency_aliases(&current.package)
                        .into_iter()
                        .find(|alias| {
                            graph
                                .resolve_module_path(current, &[alias.to_string()])
                                .is_some_and(|key| key.package == owner.package)
                        })
                else {
                    continue;
                };
                format!("{alias}::{canonical}")
            };
            paths.insert(address, canonical);
        }
        for segment in graph
            .module_child_segments(&ModuleKey::new(current.package.clone(), ModulePath::root()))
        {
            paths
                .entry(segment.to_owned())
                .or_insert_with(|| segment.to_owned());
        }
        for alias in graph.dependency_aliases(&current.package) {
            paths.entry(alias.to_string()).or_insert(alias.to_string());
        }
        Some(Self {
            scope: ImportScope::for_type_hint(databases, query),
            paths,
            databases,
            current,
        })
    }

    pub fn expand(&self, path: &str) -> Option<String> {
        let graph = self.databases.hir_db().graph();
        graph
            .expand_import_path(
                graph.module_id(self.current)?,
                &path.split("::").map(str::to_owned).collect::<Vec<_>>(),
            )
            .map(|p| p.join("::"))
    }

    pub fn item(&self, address: &str, prefix: &str, qualified: bool) -> Option<CompletionItem> {
        if !qualified && self.expand(address).as_deref() != Some(address) {
            return None;
        }
        let label = address.rsplit("::").next()?;
        let mut item = self.scope.item_for_path(address, label, prefix)?;
        if !is_type_item(&item) {
            return None;
        }
        let canonical = self.paths.get(address).map_or(address, String::as_str);
        let short_owns = self.expand(label).is_some_and(|expanded| {
            // Compare resolved declaration IDs, not package-less display names.
            let graph = self.databases.hir_db().graph();
            let Some(module) = graph.module_id(self.current) else {
                return false;
            };
            let lookup = |path: &str| {
                [
                    DeclarationKind::Struct,
                    DeclarationKind::Enum,
                    DeclarationKind::Trait,
                ]
                .into_iter()
                .find_map(|kind| {
                    graph.resolve_visible_declaration_path(
                        module,
                        &path.split("::").map(str::to_owned).collect::<Vec<_>>(),
                        kind,
                    )
                })
            };
            match (lookup(address), lookup(&expanded)) {
                (Some(a), Some(b)) => a.id == b.id,
                (None, None) => address == expanded,
                _ => false,
            }
        });
        item.insert_text = Some(if qualified || short_owns {
            label.to_owned()
        } else {
            address.to_owned()
        });
        item.metadata.lookup = Some(address.to_owned());
        item.metadata.filter_text = Some(address.to_owned());
        item.metadata.label_details.description = canonical
            .rsplit_once("::")
            .map(|(owner, _)| owner.to_owned());
        Some(item)
    }
}

pub(super) fn is_type_item(item: &CompletionItem) -> bool {
    matches!(
        item.kind,
        CompletionKind::Type | CompletionKind::Trait | CompletionKind::Module
    )
}
