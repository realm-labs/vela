use vela_hir::module_graph::{DeclarationKind, Visibility};

use crate::{LanguageServiceDatabases, QueryContext, hir_path_sites, symbol_target::SymbolTarget};

use super::{
    Definition, source_enum_variant_symbol, source_members::definition_from_named_span_with_symbol,
};

#[derive(Default)]
pub(super) struct VariantNavigation {
    pub(super) definition: Option<Definition>,
    pub(super) type_definition: Option<Definition>,
}

impl LanguageServiceDatabases {
    pub(super) fn source_variant_navigation(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
    ) -> Option<VariantNavigation> {
        let graph = self.hir_db().graph();
        let module = graph.module_id(query.module_key()?)?;
        for site in graph
            .paths_in_source(query.source_id()?)
            .filter_map(hir_path_sites::site)
        {
            if site.segment_range != target.range() {
                continue;
            }
            let path = query.expand_import_path(site.path)?;
            let (name, owner) = path.split_last()?;
            if owner.is_empty() {
                continue;
            }
            let Some(declaration) =
                graph.resolve_visible_declaration_path(module, owner, DeclarationKind::Enum)
            else {
                continue;
            };
            if declaration.module != module && declaration.visibility != Visibility::Public {
                return Some(VariantNavigation::default());
            }
            let Some(variant) = graph
                .enum_shape(declaration.id)?
                .variants
                .iter()
                .find(|variant| variant.name == *name)
            else {
                return Some(VariantNavigation::default());
            };
            return Some(VariantNavigation {
                definition: definition_from_named_span_with_symbol(
                    self,
                    variant.span,
                    &variant.name,
                    source_enum_variant_symbol(graph, declaration.id, &variant.name),
                ),
                type_definition: self.definition_from_declaration(declaration),
            });
        }
        None
    }
}
