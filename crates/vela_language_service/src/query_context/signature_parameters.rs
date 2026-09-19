use std::collections::BTreeSet;

use vela_hir::type_hint::ParamHint;

use super::QueryContext;

impl QueryContext<'_> {
    /// Required trait methods have signature metadata but no executable body.
    /// Their parameter defaults still have a declaration-owned authoring scope.
    pub(crate) fn signature_parameters(&self) -> Vec<&ParamHint> {
        if self.body().is_some() {
            return Vec::new();
        }
        let (Some(graph), Some(source)) = (self.graph, self.source_id()) else {
            return Vec::new();
        };
        let Ok(offset) = u32::try_from(self.cursor().replace_range().end) else {
            return Vec::new();
        };
        graph
            .declarations()
            .filter(|declaration| {
                declaration.span.source == source && declaration.span.contains(offset)
            })
            .filter_map(|declaration| graph.trait_shape(declaration.id))
            .flat_map(|shape| &shape.methods)
            .filter(|method| !method.has_default)
            .find(|method| {
                method.signature.params.iter().any(|parameter| {
                    parameter
                        .default_value_span
                        .is_some_and(|span| span.start <= offset && offset <= span.end)
                })
            })
            .map(|method| {
                method
                    .signature
                    .params
                    .iter()
                    .filter(|parameter| parameter.span.end <= offset)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn visible_scope_names(&self) -> Vec<String> {
        self.local_bindings_before_cursor()
            .map(|binding| binding.name.clone())
            .chain(
                self.signature_parameters()
                    .into_iter()
                    .map(|parameter| parameter.name.clone()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}
