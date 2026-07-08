use vela_common::Span;

use crate::compiler::Compiler;

use super::RecordShape;

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn record_shape_for_path_root(
        &self,
        span: Span,
        root: &str,
    ) -> Option<RecordShape> {
        self.value_shapes
            .local_at_span(self.bindings, span)
            .or_else(|| self.value_shapes.name(root))
            .and_then(|shape| shape.as_record().cloned())
            .or_else(|| {
                self.global_type_at_span(span)
                    .or_else(|| self.global_type_named(root))
                    .and_then(|type_name| self.record_shape_for_type(&type_name))
            })
    }
}
