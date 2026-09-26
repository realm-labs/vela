//! External callable metadata follows one unambiguous import and source ownership.
use vela_hir::{
    binding::BindingResolution,
    body::{HirBody, HirPathKind},
    ids::HirExprId,
    module_graph::{DeclarationKind, ModuleGraph},
};

pub(super) fn path(graph: &ModuleGraph, body: &HirBody, callee: HirExprId) -> Option<String> {
    let original = super::expression_path(body, callee, HirPathKind::Callee)?;
    if original.len() > 1 && original[0] == "task" {
        return Some(original.join("::"));
    }
    let bindings = graph.bindings_for_body(body.id)?;
    if matches!(
        bindings.resolution(callee),
        Some(BindingResolution::Local(_) | BindingResolution::Declaration(_))
    ) {
        return None;
    }
    let module = graph.declaration(bindings.declaration)?.module;
    let expanded = graph.expand_import_path(module, original)?;
    // Scoped-task paths are compiler-owned literal capabilities; imports cannot
    // manufacture them. Ordinary stdlib and registry aliases remain valid.
    if expanded.first().is_some_and(|root| root == "task") && original.first() != expanded.first() {
        return None;
    }
    for length in 1..=expanded.len() {
        if [
            DeclarationKind::Function,
            DeclarationKind::Const,
            DeclarationKind::State,
            DeclarationKind::Struct,
            DeclarationKind::Enum,
            DeclarationKind::Trait,
        ]
        .into_iter()
        .any(|kind| {
            graph
                .resolve_visible_declaration_path(module, &expanded[..length], kind)
                .is_some()
        }) {
            return None;
        }
    }
    Some(expanded.join("::"))
}
