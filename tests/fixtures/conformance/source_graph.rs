use vela_common::SourceId;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::{ModulePath, PackageId};

// Shared only by HIR and analysis: reward is a dependency, not an executable case.
pub fn conformance_graph() -> ModuleGraph {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(1),
        PackageId::anonymous(),
        ModulePath::from_qualified("conformance::core"),
        include_str!("core_language.vela"),
    ));
    graph.add_source(ModuleSource::new(
        SourceId::new(2),
        PackageId::anonymous(),
        ModulePath::from_qualified("conformance::reward"),
        include_str!("reward_module.vela"),
    ));
    graph.resolve_imports();
    graph
}
