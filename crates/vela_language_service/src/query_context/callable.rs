use vela_hir::binding::BindingResolution;
use vela_hir::module_graph::{DeclarationKind, Visibility};

use super::QueryContext;
use crate::LanguageServiceDatabases;
use crate::callable_context::{
    CallableFacts, external_callable_facts, source_callable_facts_for_declaration,
    source_variant_callable_fact,
};

impl QueryContext<'_> {
    pub(super) fn scoped_callable_facts(
        &self,
        databases: &LanguageServiceDatabases,
        path: &[String],
    ) -> Vec<CallableFacts> {
        if let Some(callables) = self.service_callable_facts(databases, path) {
            return callables;
        }
        let Some(first) = path.first() else {
            return Vec::new();
        };
        let graph = databases.hir_db().graph();
        if self
            .call_argument_facts()
            .and_then(|call| call.callee_path())
            == Some(path)
            && self.hir_call_for_cursor().is_some_and(|(body, call)| {
                body.call(call)
                    .and_then(|call| graph.bindings_for_body(body.id)?.resolution(call.callee))
                    .is_some_and(|resolution| matches!(resolution, BindingResolution::Local(_)))
            })
        {
            return Vec::new();
        }
        let module = self.module_key().and_then(|key| graph.module_id(key));
        // A declaration in the current module owns its spelling even when it
        // is not callable. Imports cannot replace that declaration's contract.
        if path.len() == 1
            && let Some(declaration) = module.and_then(|module| {
                graph
                    .declarations_in_module(module)
                    .into_iter()
                    .find(|d| d.name == *first)
            })
        {
            return if declaration.kind == DeclarationKind::Function {
                self.source_callable_facts_by_path(databases, path)
            } else {
                Vec::new()
            };
        }
        let mut imports = module
            .and_then(|module| graph.imports(module))
            .into_iter()
            .flatten()
            .filter(|import| import.alias.as_ref().or_else(|| import.path.last()) == Some(first));
        let imported = imports.next().map(|import| {
            import
                .path
                .iter()
                .chain(&path[1..])
                .cloned()
                .collect::<Vec<_>>()
        });
        if imports.next().is_some() {
            return Vec::new();
        }
        let external = imported.as_deref().unwrap_or(path);
        if imported.is_some()
            && let Some(callables) = self.service_callable_facts(databases, external)
        {
            return callables;
        }
        if let Some(declaration) = module.and_then(|module| {
            [
                DeclarationKind::Function,
                DeclarationKind::Const,
                DeclarationKind::State,
                DeclarationKind::Struct,
                DeclarationKind::Enum,
                DeclarationKind::Trait,
            ]
            .into_iter()
            .find_map(|kind| {
                graph
                    .resolve_visible_declaration_path(module, external, kind)
                    .or_else(|| graph.resolve_visible_declaration_path(module, path, kind))
            })
        }) {
            // Keep private or otherwise unavailable source owners closed to
            // same-name registry/stdlib fallback.
            return if declaration.kind == DeclarationKind::Function
                && (Some(declaration.module) == module
                    || declaration.visibility == Visibility::Public)
            {
                source_callable_facts_for_declaration(
                    graph,
                    databases.schema_db().facts(),
                    databases.schema_analysis_facts(),
                    declaration,
                )
                .into_iter()
                .collect()
            } else {
                Vec::new()
            };
        }
        if path.len() == 1
            && imported.is_none()
            && let Some(module) = module
        {
            let mut variants = graph
                .declarations_in_module(module)
                .into_iter()
                .filter(|d| {
                    d.kind == DeclarationKind::Enum
                        && graph
                            .enum_shape(d.id)
                            .is_some_and(|s| s.variants.iter().any(|v| v.name == *first))
                });
            if let Some(owner) = variants.next() {
                if variants.next().is_some() {
                    return Vec::new();
                }
                return source_variant_callable_fact(databases, owner.id, first)
                    .into_iter()
                    .collect();
            }
        }
        if let Some((variant, owner)) = external.split_last()
            && let Some(declaration) = module.and_then(|module| {
                graph.resolve_visible_declaration_path(module, owner, DeclarationKind::Enum)
            })
        {
            if Some(declaration.module) != module && declaration.visibility != Visibility::Public {
                return Vec::new();
            }
            return source_variant_callable_fact(databases, declaration.id, variant)
                .into_iter()
                .collect();
        }
        external_callable_facts(databases.schema_db().facts(), &external.join("::"))
    }
}
