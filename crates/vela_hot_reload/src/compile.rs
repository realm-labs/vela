use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use vela_bytecode::LinkedArtifact;
use vela_hir::ids::ModuleId;
use vela_hir::module_graph::ModuleGraph;

use crate::abi::HotReloadAbi;
use crate::error::{HotReloadError, HotReloadErrorKind, HotReloadResult};
use crate::function_signature::ensure_compatible_function_signature;
use crate::package_abi::ensure_compatible_package_update;
use crate::policy::HotReloadPolicy;
use crate::report::AcceptedHotReloadChanges;
use crate::symbol::{FunctionSymbolId, ProgramVersionId};
use crate::version::{HotUpdate, ProgramVersion};

pub fn initial_version_from_linked_artifact(
    abi: HotReloadAbi,
    artifact: Arc<LinkedArtifact>,
) -> HotReloadResult<ProgramVersion> {
    let abi = abi_with_script_metadata(abi, artifact.image().script_metadata());
    Ok(ProgramVersion::from_linked_artifact(
        ProgramVersionId(0),
        abi,
        artifact,
    ))
}

fn abi_with_script_metadata(abi: HotReloadAbi, graph: Option<&ModuleGraph>) -> HotReloadAbi {
    if let Some(graph) = graph {
        abi.with_script_metadata(graph)
    } else {
        abi
    }
}

pub fn update_from_linked_artifact(
    previous: &ProgramVersion,
    abi: HotReloadAbi,
    policy: &HotReloadPolicy,
    artifact: Arc<LinkedArtifact>,
) -> HotReloadResult<HotUpdate> {
    ensure_compatible_package_update(previous.linked_artifact(), &artifact)?;
    let abi = abi_with_script_metadata(abi, artifact.image().script_metadata());
    let script_metadata = artifact.image().script_metadata().cloned();
    let mut functions = BTreeMap::new();
    let mut changed_functions = Vec::new();
    for (_, code) in artifact.image().functions() {
        let name = code.name.clone();
        let symbol = FunctionSymbolId::new(&name);
        if let Some(old_code) = previous.function(&name) {
            ensure_compatible_function_signature(&name, &old_code, code, policy)?;
            if old_code.as_ref() != code {
                changed_functions.push(symbol.clone());
            }
        } else if !policy.allow_new_functions() {
            return Err(HotReloadError::new(HotReloadErrorKind::NewFunctionDenied {
                function: name,
            }));
        } else {
            changed_functions.push(symbol.clone());
        }
        functions.insert(symbol, Arc::new(code.clone()));
    }
    let previous_script_method_functions = previous
        .script_methods()
        .function_names()
        .collect::<BTreeSet<_>>();
    for old_name in previous.function_names() {
        if previous_script_method_functions.contains(old_name) {
            continue;
        }
        if !functions.contains_key(&FunctionSymbolId::new(old_name)) {
            return Err(HotReloadError::new(HotReloadErrorKind::RemovedFunction {
                function: old_name.to_owned(),
            }));
        }
    }
    previous.abi().ensure_compatible_update(&abi)?;
    let module_changes = module_changes(
        previous.script_metadata(),
        script_metadata.as_ref(),
        artifact.package_metadata().is_some(),
    );
    let changes = AcceptedHotReloadChanges::new(
        changed_functions,
        module_changes.changed_modules,
        module_changes.impacted_modules,
        module_changes.changed_packages,
        module_changes.impacted_packages,
    );
    let update = HotUpdate::new(abi, changes, artifact);
    Ok(update)
}

fn module_changes(
    previous: Option<&ModuleGraph>,
    next: Option<&ModuleGraph>,
    include_packages: bool,
) -> ModuleChanges {
    let Some(next) = next else {
        return ModuleChanges::default();
    };
    let changed = changed_module_ids(previous, next);
    let impacted = next.dependent_modules(changed.iter().copied());
    ModuleChanges {
        changed_modules: module_names(next, &changed),
        impacted_modules: module_names(next, &impacted),
        changed_packages: if include_packages {
            package_names(next, &changed)
        } else {
            Vec::new()
        },
        impacted_packages: if include_packages {
            package_names(next, &impacted)
        } else {
            Vec::new()
        },
    }
}

#[derive(Default)]
struct ModuleChanges {
    changed_modules: Vec<String>,
    impacted_modules: Vec<String>,
    changed_packages: Vec<String>,
    impacted_packages: Vec<String>,
}

fn changed_module_ids(previous: Option<&ModuleGraph>, next: &ModuleGraph) -> BTreeSet<ModuleId> {
    next.module_ids()
        .filter(|module| {
            let Some(next_key) = next.module_key(*module) else {
                return false;
            };
            let Some(previous) = previous else {
                return true;
            };
            let Some(previous_module) = previous.module_id(next_key) else {
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

fn package_names(graph: &ModuleGraph, modules: &BTreeSet<ModuleId>) -> Vec<String> {
    modules
        .iter()
        .filter_map(|module| graph.module_package(*module))
        .map(ToString::to_string)
        .collect()
}
