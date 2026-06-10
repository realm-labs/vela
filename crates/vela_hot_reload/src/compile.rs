use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use vela_bytecode::Program;
use vela_bytecode::compiler::options::CompilerOptions;
use vela_bytecode::compiler::{
    compile_module_sources_with_options, compile_module_sources_with_options_and_registry,
    compile_program_source_with_options, compile_program_source_with_options_and_registry,
};
use vela_common::SourceId;
use vela_hir::ids::ModuleId;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_registry::RegistryCompileView;

use crate::abi::HotReloadAbi;
use crate::error::{HotReloadError, HotReloadErrorKind, HotReloadResult};
use crate::function_signature::ensure_compatible_function_signature;
use crate::policy::HotReloadPolicy;
use crate::report::AcceptedHotReloadChanges;
use crate::symbol::{FunctionSymbolId, ProgramVersionId};
use crate::version::{HotUpdate, ProgramVersion};

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

pub fn compile_initial_with_abi_options_and_registry(
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
    options: &CompilerOptions,
    registry: RegistryCompileView<'_>,
) -> HotReloadResult<ProgramVersion> {
    let program = compile_program_source_with_options_and_registry(source, text, options, registry)
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

pub fn compile_initial_modules_with_abi_options_and_registry(
    sources: &[ModuleSource],
    abi: HotReloadAbi,
    options: &CompilerOptions,
    registry: RegistryCompileView<'_>,
) -> HotReloadResult<ProgramVersion> {
    let program = compile_module_sources_with_options_and_registry(sources, options, registry)
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

pub fn compile_update_with_abi_options_registry_and_policy(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
    options: &CompilerOptions,
    registry: RegistryCompileView<'_>,
    policy: &HotReloadPolicy,
) -> HotReloadResult<HotUpdate> {
    let program = compile_program_source_with_options_and_registry(source, text, options, registry)
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

pub fn compile_update_modules_with_abi_options_registry_and_policy(
    previous: &ProgramVersion,
    sources: &[ModuleSource],
    abi: HotReloadAbi,
    options: &CompilerOptions,
    registry: RegistryCompileView<'_>,
    policy: &HotReloadPolicy,
) -> HotReloadResult<HotUpdate> {
    let program = compile_module_sources_with_options_and_registry(sources, options, registry)
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
    let global_names = program.global_names().to_vec();
    let script_methods = program.script_methods().clone();
    let script_metadata = program.script_metadata().cloned();
    let mut functions = BTreeMap::new();
    let mut changed_functions = Vec::new();
    for (name, code) in program.functions {
        let symbol = FunctionSymbolId::new(&name);
        if let Some(old_code) = previous.functions.get(&symbol) {
            ensure_compatible_function_signature(&name, old_code, &code, policy)?;
            if old_code.as_ref() != &code {
                changed_functions.push(symbol.clone());
            }
        } else if !policy.allow_new_functions() {
            return Err(HotReloadError::new(HotReloadErrorKind::NewFunctionDenied {
                function: name,
            }));
        } else {
            changed_functions.push(symbol.clone());
        }
        functions.insert(symbol, Arc::new(code));
    }
    let previous_script_method_functions = previous
        .script_methods()
        .function_names()
        .collect::<BTreeSet<_>>();
    for old_name in previous.functions.keys() {
        if previous_script_method_functions.contains(old_name.0.as_str()) {
            continue;
        }
        if !functions.contains_key(old_name) {
            return Err(HotReloadError::new(HotReloadErrorKind::RemovedFunction {
                function: old_name.0.clone(),
            }));
        }
    }
    previous.abi().ensure_compatible_update(&abi)?;
    let (changed_modules, impacted_modules) =
        module_changes(previous.script_metadata(), script_metadata.as_ref());
    let changes =
        AcceptedHotReloadChanges::new(changed_functions, changed_modules, impacted_modules);
    Ok(HotUpdate::new(
        functions,
        global_names,
        script_methods,
        script_metadata,
        abi,
        changes,
    ))
}

fn module_changes(
    previous: Option<&ModuleGraph>,
    next: Option<&ModuleGraph>,
) -> (Vec<String>, Vec<String>) {
    let Some(next) = next else {
        return (Vec::new(), Vec::new());
    };
    let changed = changed_module_ids(previous, next);
    let impacted = next.dependent_modules(changed.iter().copied());
    (module_names(next, &changed), module_names(next, &impacted))
}

fn changed_module_ids(previous: Option<&ModuleGraph>, next: &ModuleGraph) -> BTreeSet<ModuleId> {
    next.module_ids()
        .filter(|module| {
            let Some(next_path) = next.module_path(*module) else {
                return false;
            };
            let Some(previous) = previous else {
                return true;
            };
            let Some(previous_module) = previous.module_id(next_path) else {
                return true;
            };
            previous.module_source_hash(previous_module) != next.module_source_hash(*module)
        })
        .collect()
}

fn module_names(graph: &ModuleGraph, modules: &BTreeSet<ModuleId>) -> Vec<String> {
    modules
        .iter()
        .filter_map(|module| graph.module_path(*module))
        .map(|path| path.join())
        .filter(|name| !name.is_empty())
        .collect()
}
