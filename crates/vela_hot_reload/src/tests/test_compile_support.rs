use std::sync::Arc;

use vela_bytecode::compiler::options::CompilerOptions;
use vela_bytecode::compiler::{ProgramCompilationRequest, compile_program};
use vela_bytecode::{LinkedArtifact, Linker};
use vela_common::SourceId;
use vela_hir::module_graph::ModuleSource;
use vela_hir::source_ingestion::{HirSourceSet, build_module_source_set, build_single_source};
use vela_registry::{DefinitionRegistry, TypeDef};

use crate::abi::HotReloadAbi;
use crate::compile::{initial_version_from_linked_artifact, update_from_linked_artifact};
use crate::error::HotReloadResult;
use crate::policy::HotReloadPolicy;
use crate::version::{HotUpdate, ProgramVersion};

pub(super) fn compile_initial(source: SourceId, text: &str) -> HotReloadResult<ProgramVersion> {
    compile_initial_with_abi(source, text, HotReloadAbi::empty())
}

pub(super) fn compile_initial_with_abi(
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
) -> HotReloadResult<ProgramVersion> {
    initial_version_from_linked_artifact(
        abi,
        compile_single_artifact(source, text, &CompilerOptions::default()),
    )
}

pub(super) fn compile_initial_modules_with_abi_and_options(
    sources: &[ModuleSource],
    abi: HotReloadAbi,
    options: &CompilerOptions,
) -> HotReloadResult<ProgramVersion> {
    initial_version_from_linked_artifact(abi, compile_module_artifact(sources, options))
}

pub(super) fn compile_update(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
) -> HotReloadResult<HotUpdate> {
    compile_update_with_policy(previous, source, text, &HotReloadPolicy::default())
}

pub(super) fn compile_update_with_policy(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
    policy: &HotReloadPolicy,
) -> HotReloadResult<HotUpdate> {
    compile_update_with_abi_and_policy(previous, source, text, previous.abi().clone(), policy)
}

pub(super) fn compile_update_with_abi(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
) -> HotReloadResult<HotUpdate> {
    compile_update_with_abi_and_policy(previous, source, text, abi, &HotReloadPolicy::default())
}

fn compile_update_with_abi_and_policy(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
    policy: &HotReloadPolicy,
) -> HotReloadResult<HotUpdate> {
    update_from_linked_artifact(
        previous,
        abi,
        policy,
        compile_single_artifact(source, text, &CompilerOptions::default()),
    )
}

pub(super) fn compile_update_modules_with_abi_and_options_and_policy(
    previous: &ProgramVersion,
    sources: &[ModuleSource],
    abi: HotReloadAbi,
    options: &CompilerOptions,
    policy: &HotReloadPolicy,
) -> HotReloadResult<HotUpdate> {
    update_from_linked_artifact(
        previous,
        abi,
        policy,
        compile_module_artifact(sources, options),
    )
}

fn compile_single_artifact(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
) -> Arc<LinkedArtifact> {
    let sources = build_single_source(source, text).expect("hot-reload test source must build");
    compile_artifact(&sources, options)
}

fn compile_module_artifact(
    sources: &[ModuleSource],
    options: &CompilerOptions,
) -> Arc<LinkedArtifact> {
    let sources = build_module_source_set(sources).expect("hot-reload test source must build");
    compile_artifact(&sources, options)
}

fn compile_artifact(sources: &HirSourceSet, options: &CompilerOptions) -> Arc<LinkedArtifact> {
    let mut registry = DefinitionRegistry::new();
    registry
        .register_type(
            TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "Player",
            ))
            .host_runtime_id(1),
        )
        .expect("hot-reload test host type must register");
    let program = compile_program(ProgramCompilationRequest {
        sources,
        options,
        registry: Some(registry.compile_view()),
    })
    .expect("hot-reload test source must compile");
    Linker::with_registry(&registry)
        .link_compiled_program(program)
        .expect("hot-reload test artifact must link")
}
