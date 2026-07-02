use vela_common::Span;
use vela_syntax::ast::Expr;

use crate::compiler::Compiler;
use crate::compiler::body_payloads::CompilerExpressionPayload;

use super::RecordShape;

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn record_shape_for_expr_with_payload(
        &self,
        expr: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> Option<RecordShape> {
        self.value_shape_for_expr_with_payload(expr, payload)?
            .as_record()
            .cloned()
    }

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

    pub(in crate::compiler) fn record_shape_for_index_collection(
        &self,
        collection: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> Option<RecordShape> {
        self.value_shape_for_expr_with_payload(collection, payload)?
            .array_element_record()
            .cloned()
    }
}
