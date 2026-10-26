use vela_common::SourceId;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::ModulePath;

#[rstest::rstest]
#[case::extern_state("extern state root: Root; fn main() { root.value = 1; }")]
#[case::array_view("fn main(value: ArrayView<i64>) { return value[0]; }")]
#[case::release("fn main(value) { host::release(value); }")]
#[case::aliased_try_release(
    "use host::try_release as drop; fn main(value) { let f = drop; f(value); }"
)]
fn borrowed_host_execution_is_rejected_before_mir(#[case] text: &str) {
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
