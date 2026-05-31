use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::Program;
use vela_bytecode::compiler::{
    CompilerOptions, compile_module_sources_with_options, compile_program_source_with_options,
};
use vela_common::SourceId;
use vela_hir::ModuleSource;

use crate::{
    FunctionSymbolId, HotReloadAbi, HotReloadError, HotReloadErrorKind, HotReloadPolicy,
    HotReloadResult, HotUpdate, ProgramVersion, ProgramVersionId,
    function_signature::ensure_compatible_function_signature,
};

pub fn compile_initial(source: SourceId, text: &str) -> HotReloadResult<ProgramVersion> {
    compile_initial_with_abi_and_options(
        source,
        text,
        HotReloadAbi::empty(),
        &CompilerOptions::default(),
    )
}

pub fn compile_initial_with_options(
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
) -> HotReloadResult<ProgramVersion> {
    compile_initial_with_abi_and_options(source, text, HotReloadAbi::empty(), options)
}

pub fn compile_initial_with_abi(
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
) -> HotReloadResult<ProgramVersion> {
    compile_initial_with_abi_and_options(source, text, abi, &CompilerOptions::default())
}

pub fn compile_initial_with_abi_and_options(
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
    options: &CompilerOptions,
) -> HotReloadResult<ProgramVersion> {
    let program = compile_program_source_with_options(source, text, options)
        .map_err(|error| HotReloadError::new(HotReloadErrorKind::Compile(error)))?;
    Ok(initial_version_from_program(program, abi))
}

pub fn compile_initial_modules_with_abi_and_options(
    sources: &[ModuleSource],
    abi: HotReloadAbi,
    options: &CompilerOptions,
) -> HotReloadResult<ProgramVersion> {
    let program = compile_module_sources_with_options(sources, options)
        .map_err(|error| HotReloadError::new(HotReloadErrorKind::Compile(error)))?;
    Ok(initial_version_from_program(program, abi))
}

pub fn compile_update(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
) -> HotReloadResult<HotUpdate> {
    compile_update_with_policy(previous, source, text, &HotReloadPolicy::default())
}

pub fn compile_update_with_policy(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
    policy: &HotReloadPolicy,
) -> HotReloadResult<HotUpdate> {
    compile_update_with_abi_and_options_and_policy(
        previous,
        source,
        text,
        previous.abi().clone(),
        &CompilerOptions::default(),
        policy,
    )
}

pub fn compile_update_with_options(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
    options: &CompilerOptions,
) -> HotReloadResult<HotUpdate> {
    compile_update_with_abi_and_options_and_policy(
        previous,
        source,
        text,
        previous.abi().clone(),
        options,
        &HotReloadPolicy::default(),
    )
}

pub fn compile_update_with_abi(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
) -> HotReloadResult<HotUpdate> {
    compile_update_with_abi_and_policy(previous, source, text, abi, &HotReloadPolicy::default())
}

pub fn compile_update_with_abi_and_policy(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
    policy: &HotReloadPolicy,
) -> HotReloadResult<HotUpdate> {
    compile_update_with_abi_and_options_and_policy(
        previous,
        source,
        text,
        abi,
        &CompilerOptions::default(),
        policy,
    )
}

pub fn compile_update_with_abi_and_options(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
    options: &CompilerOptions,
) -> HotReloadResult<HotUpdate> {
    compile_update_with_abi_and_options_and_policy(
        previous,
        source,
        text,
        abi,
        options,
        &HotReloadPolicy::default(),
    )
}

pub fn compile_update_with_abi_and_options_and_policy(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
    options: &CompilerOptions,
    policy: &HotReloadPolicy,
) -> HotReloadResult<HotUpdate> {
    let program = compile_program_source_with_options(source, text, options)
        .map_err(|error| HotReloadError::new(HotReloadErrorKind::Compile(error)))?;
    update_from_program(previous, program, abi, policy)
}

pub fn compile_update_modules_with_abi_and_options_and_policy(
    previous: &ProgramVersion,
    sources: &[ModuleSource],
    abi: HotReloadAbi,
    options: &CompilerOptions,
    policy: &HotReloadPolicy,
) -> HotReloadResult<HotUpdate> {
    let program = compile_module_sources_with_options(sources, options)
        .map_err(|error| HotReloadError::new(HotReloadErrorKind::Compile(error)))?;
    update_from_program(previous, program, abi, policy)
}

fn initial_version_from_program(program: Program, abi: HotReloadAbi) -> ProgramVersion {
    let abi = abi_with_script_metadata(abi, &program);
    ProgramVersion::from_program_with_abi(ProgramVersionId(0), program, abi)
}

fn abi_with_script_metadata(abi: HotReloadAbi, program: &Program) -> HotReloadAbi {
    if let Some(graph) = program.script_metadata() {
        abi.with_script_metadata(graph)
    } else {
        abi
    }
}

fn update_from_program(
    previous: &ProgramVersion,
    program: Program,
    abi: HotReloadAbi,
    policy: &HotReloadPolicy,
) -> HotReloadResult<HotUpdate> {
    let abi = abi_with_script_metadata(abi, &program);
    let script_methods = program.script_methods().clone();
    let script_metadata = program.script_metadata().cloned();
    let mut functions = BTreeMap::new();
    for (name, code) in program.functions {
        if let Some(old_code) = previous.functions.get(&FunctionSymbolId::new(&name)) {
            ensure_compatible_function_signature(&name, old_code, &code, policy)?;
        } else if !policy.allow_new_functions() {
            return Err(HotReloadError::new(HotReloadErrorKind::NewFunctionDenied {
                function: name,
            }));
        }
        functions.insert(FunctionSymbolId::new(name), Arc::new(code));
    }
    for old_name in previous.functions.keys() {
        if !functions.contains_key(old_name) {
            return Err(HotReloadError::new(HotReloadErrorKind::RemovedFunction {
                function: old_name.0.clone(),
            }));
        }
    }
    previous.abi().ensure_compatible_update(&abi)?;
    Ok(HotUpdate::new(
        functions,
        script_methods,
        script_metadata,
        abi,
    ))
}
