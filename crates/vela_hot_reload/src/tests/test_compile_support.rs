use std::sync::Arc;

use vela_bytecode::compiler::options::CompilerOptions;
use vela_bytecode::compiler::{ProgramCompilationKind, ProgramCompilationRequest, compile_program};
use vela_bytecode::{LinkedArtifact, Linker};
use vela_common::SourceId;
use vela_hir::module_graph::{ModulePath, ModuleSource};
use vela_hir::source_ingestion::build_source_set;

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
    let sources = [ModuleSource::new(source, ModulePath::root(), text)];
    initial_version_from_linked_artifact(
        abi,
        compile_artifact(&sources, true, &CompilerOptions::default()),
    )
}

pub(super) fn compile_initial_modules_with_abi_and_options(
    sources: &[ModuleSource],
    abi: HotReloadAbi,
    options: &CompilerOptions,
) -> HotReloadResult<ProgramVersion> {
    initial_version_from_linked_artifact(abi, compile_artifact(sources, false, options))
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
    let sources = [ModuleSource::new(source, ModulePath::root(), text)];
    update_from_linked_artifact(
        previous,
        abi,
        policy,
        compile_artifact(&sources, true, &CompilerOptions::default()),
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
        compile_artifact(sources, false, options),
    )
}

fn compile_artifact(
    sources: &[ModuleSource],
    single_source: bool,
    options: &CompilerOptions,
) -> Arc<LinkedArtifact> {
    let sources = build_source_set(sources).expect("hot-reload test source must build");
    let kind = if single_source {
        ProgramCompilationKind::SingleSource
    } else {
        ProgramCompilationKind::ModuleGraph
    };
    let program = compile_program(ProgramCompilationRequest {
        sources: &sources,
        kind,
        options,
        registry: None,
    })
    .expect("hot-reload test source must compile");
    Linker::new()
        .link_compiled_program(program)
        .expect("hot-reload test artifact must link")
}
