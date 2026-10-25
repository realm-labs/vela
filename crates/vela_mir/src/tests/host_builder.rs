use vela_common::SourceId;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::ModulePath;

#[test]
fn borrowed_host_execution_is_rejected_before_mir() {
    for text in [
        "extern state root: Root; fn main() { root.value = 1; }",
        "fn main(value: ArrayView<i64>) { return value[0]; }",
        "fn main(value) { host::release(value); }",
        "use host::try_release as drop; fn main(value) { let f = drop; f(value); }",
    ] {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            vela_package::PackageId::anonymous(),
            ModulePath::from_qualified("removed"),
            text,
        ));
        assert!(
            !graph.diagnostics().is_empty(),
            "removed boundary accepted: {text}"
        );
    }
}
