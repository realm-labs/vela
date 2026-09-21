use super::{
    LocalRenameTarget, TextEdit, WorkspaceEdit, diagnostic_range, local_collisions, shorthand,
    span_text_range, workspace_edit_for_rename,
};
use crate::LanguageServiceDatabases;
use std::collections::BTreeMap;
use vela_hir::binding::BindingResolution;

impl LanguageServiceDatabases {
    pub(super) fn rename_local(
        &self,
        target: LocalRenameTarget<'_>,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        if local_collisions::conflicts(
            self.hir_db().graph(),
            target.bindings,
            target.local,
            new_name,
        ) {
            return None;
        }

        let graph = self.hir_db().graph();
        let binding = target.bindings.local(target.local)?;
        let source = self.source_record_for_rename(binding.span.source)?;
        let document_id = source.document_id();
        let text = source.text();
        let named_sites = if binding.kind == vela_hir::binding::LocalBindingKind::Parameter {
            crate::named_argument_sites::matching(self, &[&binding.name, new_name])
        } else {
            Vec::new()
        };
        if named_sites.iter().any(|site| {
            site.owner == target.bindings.body()
                && site.name == new_name
                && site.parameter != Some(target.local)
        }) {
            return None;
        }
        let shorthand_labels = shorthand::local_labels(graph, source.source_id());
        let mut edits = Vec::new();
        if let Some(binding) = target.bindings.local(target.local)
            && let Some(range) = span_text_range(binding.span)
        {
            edits.push(TextEdit {
                range: diagnostic_range(text, range),
                new_text: shorthand::local_replacement(&shorthand_labels, binding.span, new_name),
            });
        }
        edits.extend(
            target
                .bindings
                .resolutions()
                .filter_map(|(expression, resolution)| match resolution {
                    BindingResolution::Local(local) if *local == target.local => {
                        let span = graph.expression_span(expression)?;
                        Some(TextEdit {
                            range: diagnostic_range(text, span_text_range(span)?),
                            new_text: shorthand::local_replacement(
                                &shorthand_labels,
                                span,
                                new_name,
                            ),
                        })
                    }
                    BindingResolution::Local(_)
                    | BindingResolution::Declaration(_)
                    | BindingResolution::Import(_)
                    | BindingResolution::QualifiedPath(_) => None,
                }),
        );

        edits.sort_by_key(|edit| {
            let start = edit.range.start();
            (start.line, start.character)
        });

        let mut by_document = BTreeMap::from([(document_id.clone(), edits)]);
        for site in named_sites
            .into_iter()
            .filter(|site| site.parameter == Some(target.local))
        {
            let source = self.source_db().records().get(&site.document)?;
            by_document
                .entry(site.document)
                .or_default()
                .push(TextEdit {
                    range: diagnostic_range(source.text(), site.range),
                    new_text: new_name.to_owned(),
                });
        }
        workspace_edit_for_rename(self, by_document, Vec::new())
    }
}
