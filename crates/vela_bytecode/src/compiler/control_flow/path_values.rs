use vela_hir::ids::HirExprId;

use super::Compiler;

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn hir_value_path_root_expression(
        &self,
        expression: HirExprId,
    ) -> Option<HirExprId> {
        if let Some(field) = self
            .hir_bodies
            .iter()
            .find_map(|body| body.field(expression))
        {
            return self.hir_value_path_root_expression(field.receiver);
        }
        if self
            .hir_value_path(expression)
            .is_some_and(|path| !path.is_empty())
            || self.local_for_expression(expression).is_some()
        {
            return Some(expression);
        }
        None
    }

    pub(in crate::compiler) fn hir_value_path_for_expression(
        &self,
        expression: HirExprId,
    ) -> Option<Vec<String>> {
        if let Some(path) = self.hir_value_path(expression)
            && !path.is_empty()
        {
            return Some(path.to_vec());
        }
        if let Some(local) = self.local_for_expression(expression)
            && let Some(binding) = self.bindings.local(local)
        {
            return Some(vec![binding.name.clone()]);
        }
        let field = self
            .hir_bodies
            .iter()
            .find_map(|body| body.field(expression))?;
        if field.name.parse::<usize>().is_ok() {
            return None;
        }
        let mut path = self.hir_value_path_for_expression(field.receiver)?;
        path.push(field.name.clone());
        Some(path)
    }
}
