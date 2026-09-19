use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::SourceId;
use vela_def::FunctionId;
use vela_hir::body::HirBodyOwner;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::{ModulePath, PackageId};

use crate::*;

#[test]
fn required_signature_default_is_not_a_runtime_compilation_root() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(1),
        PackageId::anonymous(),
        ModulePath::from_qualified("game"),
        "trait Reader { fn run(self, callback = |value: bool| value); } fn main() {}",
    ));
    let main = graph
        .declarations()
        .find(|decl| decl.name == "main")
        .expect("main declaration");
    let runtime_body = graph.function_body(main.id).expect("main body").id;
    let signature_body = graph
        .bodies()
        .find(|body| matches!(body.owner, HirBodyOwner::TraitSignatureDefault(_)))
        .expect("signature default root")
        .id;
    let function = FunctionId::new(1);
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(function, runtime_body)],
    )
    .expect("main executable analysis");
    let targets = CompileTargetSnapshot::builder()
        .build()
        .expect("empty target snapshot");
    assert!(
        matches!(MirLoweringInput::new(&graph, CompileFunctionIdentity::Function(function), signature_body, analysis.view(function).expect("main analysis view"), &targets, MirLoweringConfig::default()), Err(MirBuildError::NonRuntimeBody { body, .. }) if body == signature_body)
    );
}
