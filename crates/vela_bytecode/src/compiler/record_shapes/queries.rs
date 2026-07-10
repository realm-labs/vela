use vela_hir::ids::HirExprId;

use crate::compiler::Compiler;

use super::RecordShape;

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn record_shape_for_path_root(
        &self,
        expression: HirExprId,
        root: &str,
    ) -> Option<RecordShape> {
        self.local_for_expression(expression)
            .and_then(|local| self.value_shapes.local(local))
            .or_else(|| self.value_shapes.name(root))
            .and_then(|shape| shape.as_record().cloned())
            .or_else(|| {
                self.global_type_for_expression(expression)
                    .or_else(|| self.global_type_named(root))
                    .and_then(|type_name| self.record_shape_for_type(&type_name))
            })
    }
}
