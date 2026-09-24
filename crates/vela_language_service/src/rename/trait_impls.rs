use super::{
    BTreeMap, Declaration, DeclarationKind, DocumentId, LanguageServiceDatabases, ModuleGraph,
    RenameToken, SourceId, TextEdit, diagnostic_range, span_text_range,
};
use vela_hir::type_hint::ImplMetadataKind;

impl LanguageServiceDatabases {
    pub(super) fn push_trait_impl_use_edits(
        &self,
        declaration: &Declaration,
        new_name: &str,
        edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
    ) {
        if declaration.kind != DeclarationKind::Trait {
            return;
        }
        let graph = self.hir_db().graph();
        for owner in graph.declarations() {
            let Some(metadata) = graph.impl_metadata(owner.id) else {
                continue;
            };
            let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
                continue;
            };
            if crate::references::trait_declaration_for_path(graph, owner.module, trait_path)
                != Some(declaration.id)
            {
                continue;
            }
            let Some(source) = self.source_record_for_rename(owner.span.source) else {
                continue;
            };
            let Some(span_range) = span_text_range(owner.span) else {
                continue;
            };
            let Some(range) = crate::references::trait_path_name_range_in_text(
                source.text(),
                span_range,
                trait_path,
            ) else {
                continue;
            };
            edits_by_document
                .entry(source.document_id().clone())
                .or_default()
                .push(TextEdit {
                    range: diagnostic_range(source.text(), range),
                    new_text: new_name.to_owned(),
                });
        }
    }
}

pub(super) fn declaration_at_token<'a>(
    graph: &'a ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &RenameToken,
) -> Option<&'a Declaration> {
    graph.declarations().find_map(|owner| {
        if owner.span.source != source_id {
            return None;
        }
        let metadata = graph.impl_metadata(owner.id)?;
        let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
            return None;
        };
        let span_range = span_text_range(owner.span)?;
        let range = crate::references::trait_path_name_range_in_text(text, span_range, trait_path)?;
        if token.range.start < range.start || token.range.end > range.end {
            return None;
        }
        let declaration =
            crate::references::trait_declaration_for_path(graph, owner.module, trait_path)?;
        graph.declaration(declaration)
    })
}
