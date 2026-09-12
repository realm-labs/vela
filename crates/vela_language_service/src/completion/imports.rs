use std::collections::{BTreeMap, BTreeSet};

use super::{
    CompletionContext, CompletionItem, accumulator::CompletionAccumulator,
    analysis_item::service_item_from_analysis_completion,
};
use crate::{LanguageServiceDatabases, QueryContext, SymbolRef};
use vela_analysis::{
    completion::{
        CompletionItem as AnalysisItem, CompletionKind as AnalysisKind, declaration_completion,
        global_completions,
    },
    stdlib::stdlib_function_completion_facts,
    type_fact::TypeFact,
};
use vela_hir::module_graph::{DeclarationKind, Import, Visibility};

pub(super) struct ImportScope<'a, 'db> {
    databases: &'a LanguageServiceDatabases,
    query: &'a QueryContext<'db>,
    imports: BTreeMap<&'a str, Option<&'a Import>>,
    locals: BTreeSet<&'a str>,
    declarations: BTreeSet<&'a str>,
}

impl<'a, 'db> ImportScope<'a, 'db> {
    pub(super) fn for_type_hint(
        databases: &'a LanguageServiceDatabases,
        query: &'a QueryContext<'db>,
    ) -> Self {
        let mut scope = Self::new(databases, query);
        // Value bindings do not shadow names in a type annotation.
        scope.locals.clear();
        scope
    }
    pub(super) fn external_function_available(&self, path: &str) -> bool {
        let graph = self.databases.hir_db().graph();
        let Some(module) = self.query.module_key().and_then(|key| graph.module_id(key)) else {
            return true;
        };
        let segments = path.split("::").map(str::to_owned).collect::<Vec<_>>();
        if segments.len() == 1 && (self.locals.contains(path) || self.imports.contains_key(path)) {
            return false;
        }
        ![
            DeclarationKind::Function,
            DeclarationKind::Const,
            DeclarationKind::State,
            DeclarationKind::Struct,
            DeclarationKind::Enum,
            DeclarationKind::Trait,
        ]
        .into_iter()
        .any(|kind| {
            graph
                .resolve_visible_declaration_path(module, &segments, kind)
                .is_some()
        }) && !stdlib_function_completion_facts()
            .iter()
            .any(|function| function.name == path)
    }
    pub(super) fn new(
        databases: &'a LanguageServiceDatabases,
        query: &'a QueryContext<'db>,
    ) -> Self {
        let graph = databases.hir_db().graph();
        let module = query.module_key().and_then(|key| graph.module_id(key));
        let mut imports = BTreeMap::new();
        for import in module
            .and_then(|id| graph.imports(id))
            .into_iter()
            .flatten()
        {
            if let Some(name) = import.alias.as_ref().or_else(|| import.path.last()) {
                imports
                    .entry(name.as_str())
                    .and_modify(|value| *value = None)
                    .or_insert(Some(import));
            }
        }
        Self {
            databases,
            query,
            imports,
            locals: query
                .local_bindings_before_cursor()
                .map(|binding| binding.name.as_str())
                .collect(),
            declarations: module
                .into_iter()
                .flat_map(|id| graph.declarations_in_module(id))
                .map(|declaration| declaration.name.as_str())
                .collect(),
        }
    }

    pub(super) fn completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        let mut accumulator = CompletionAccumulator::new(context.replace_range(), context.prefix());
        for (name, import) in &self.imports {
            if !name.starts_with(context.prefix())
                || self.locals.contains(name)
                || self.declarations.contains(name)
            {
                continue;
            }
            let Some(import) = import else {
                continue;
            };
            if let Some(item) = self.item_for_path(&import.path.join("::"), name, context.prefix())
            {
                accumulator.add(item);
            }
        }
        accumulator.into_items()
    }

    pub(super) fn expand(&self, path: &str) -> Option<String> {
        self.query
            .expand_import_path(&path.split("::").map(str::to_owned).collect::<Vec<_>>())
            .map(|path| path.join("::"))
    }

    pub(super) fn item_for_path(
        &self,
        path: &str,
        spelling: &str,
        prefix: &str,
    ) -> Option<CompletionItem> {
        let graph = self.databases.hir_db().graph();
        let schema = self.databases.schema_db().facts();
        let current = self.query.module_key()?;
        let module = graph.module_id(current)?;
        let segments = path.split("::").map(str::to_owned).collect::<Vec<_>>();
        if path.starts_with("service::") {
            let callables = self.query.callable_facts_by_path(self.databases, &segments);
            let callable = callables.first()?;
            return Some(render(
                AnalysisItem {
                    label: path.to_owned(),
                    kind: AnalysisKind::Function,
                    fact: TypeFact::function(
                        callable
                            .params()
                            .iter()
                            .map(|p| p.type_fact().clone())
                            .collect(),
                        callable.returns().clone(),
                    ),
                },
                callable.symbol().clone(),
                spelling,
                prefix,
            ));
        }
        if let Some(declaration) = [
            DeclarationKind::Function,
            DeclarationKind::Const,
            DeclarationKind::State,
            DeclarationKind::Struct,
            DeclarationKind::Enum,
            DeclarationKind::Trait,
        ]
        .into_iter()
        .find_map(|kind| graph.resolve_visible_declaration_path(module, &segments, kind))
        {
            if declaration.module != module && declaration.visibility != Visibility::Public {
                return None;
            }
            let item =
                declaration_completion(graph, self.databases.graph_analysis_facts(), declaration)?;
            let symbol = SymbolRef::Source(item.label.clone());
            return Some(render(item, symbol, spelling, prefix));
        }
        let stdlib = stdlib_function_completion_facts();
        if let Some(function) = stdlib.iter().find(|function| function.name == path) {
            return Some(render(
                AnalysisItem {
                    label: path.to_owned(),
                    kind: AnalysisKind::Function,
                    fact: TypeFact::function(function.params.clone(), function.returns.clone()),
                },
                SymbolRef::Builtin(path.to_owned()),
                spelling,
                prefix,
            ));
        }
        let globals = global_completions(schema);
        if let Some(item) = globals.iter().find(|item| item.label == path) {
            return Some(render(
                item.clone(),
                SymbolRef::Schema(path.to_owned()),
                spelling,
                prefix,
            ));
        }
        let module_key = graph.resolve_module_path(current, &segments)?;
        let namespace = format!("{path}::");
        let origin = if graph.module_id(&module_key).is_some()
            || !graph.module_child_segments(&module_key).is_empty()
        {
            crate::symbol_ref::source_module_symbol(&module_key)
        } else if stdlib
            .iter()
            .any(|function| function.name.starts_with(&namespace))
        {
            SymbolRef::Builtin(path.to_owned())
        } else if globals
            .iter()
            .any(|item| item.label.starts_with(&namespace))
        {
            SymbolRef::Schema(path.to_owned())
        } else {
            return None;
        };
        Some(render(
            AnalysisItem {
                label: path.to_owned(),
                kind: AnalysisKind::Module,
                fact: TypeFact::module(path),
            },
            origin,
            spelling,
            prefix,
        ))
    }
}

fn render(
    mut item: AnalysisItem,
    symbol: SymbolRef,
    spelling: &str,
    prefix: &str,
) -> CompletionItem {
    let canonical = item.label.clone();
    item.label = spelling.to_owned();
    let mut completion = service_item_from_analysis_completion(item, prefix).with_symbol(symbol);
    completion
        .insert_text
        .get_or_insert_with(|| spelling.to_owned());
    completion.metadata.label_details.description = Some(canonical);
    completion
}
