use vela_hir::module_graph::{Declaration, ModuleGraph};
use vela_package::ModuleKey;

// Display names omit package identity. An insertion must address the actual
// declaration from the current package/import scope instead.
pub(super) fn declaration_address(
    graph: &ModuleGraph,
    current: &ModuleKey,
    declaration: &Declaration,
) -> Option<String> {
    let module = graph.module_id(current)?;
    let owner = graph.module_key(declaration.module)?;
    let canonical = graph.qualified_declaration_name(declaration.id)?;
    let resolves = |address: &str| {
        let path = address.split("::").map(str::to_owned).collect::<Vec<_>>();
        graph
            .expand_import_path(module, &path)
            .and_then(|path| {
                graph.resolve_visible_declaration_path(module, &path, declaration.kind)
            })
            .is_some_and(|target| target.id == declaration.id)
    };
    if owner.package == current.package {
        let address = if resolves(&canonical) {
            canonical
        } else {
            format!("crate::{canonical}")
        };
        return resolves(&address).then_some(address);
    }
    graph
        .dependency_aliases(&current.package)
        .into_iter()
        .map(|alias| format!("{alias}::{canonical}"))
        .find(|address| resolves(address))
}
