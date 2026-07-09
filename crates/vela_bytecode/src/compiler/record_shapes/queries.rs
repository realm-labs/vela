use vela_common::Span;

use crate::compiler::Compiler;

use super::RecordShape;

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn record_shape_for_path_root(
        &self,
        span: Span,
        root: &str,
    ) -> Option<RecordShape> {
        let expression = self.expression_at_span(span);
        expression
            .and_then(|expression| self.local_for_expression(expression))
            .and_then(|local| self.value_shapes.local(local))
            .or_else(|| self.value_shapes.name(root))
            .and_then(|shape| shape.as_record().cloned())
            .or_else(|| {
                expression
                    .and_then(|expression| self.global_type_for_expression(expression))
                    .or_else(|| self.global_type_named(root))
                    .and_then(|type_name| self.record_shape_for_type(&type_name))
            })
    }
}
