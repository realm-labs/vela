use std::collections::BTreeMap;
use std::sync::Arc;

use vela_bytecode::CodeObject;
use vela_bytecode::compiler::compile_program_source;
use vela_common::SourceId;

use crate::{
    FunctionSymbolId, HotReloadAbi, HotReloadError, HotReloadErrorKind, HotReloadResult, HotUpdate,
    ProgramVersion, ProgramVersionId,
};

pub fn compile_initial(source: SourceId, text: &str) -> HotReloadResult<ProgramVersion> {
    compile_initial_with_abi(source, text, HotReloadAbi::empty())
}

pub fn compile_initial_with_abi(
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
) -> HotReloadResult<ProgramVersion> {
    let program = compile_program_source(source, text)
        .map_err(|error| HotReloadError::new(HotReloadErrorKind::Compile(error)))?;
    Ok(ProgramVersion::from_program_with_abi(
        ProgramVersionId(0),
        program,
        abi,
    ))
}

pub fn compile_update(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
) -> HotReloadResult<HotUpdate> {
    compile_update_with_abi(previous, source, text, previous.abi().clone())
}

pub fn compile_update_with_abi(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
) -> HotReloadResult<HotUpdate> {
    let program = compile_program_source(source, text)
        .map_err(|error| HotReloadError::new(HotReloadErrorKind::Compile(error)))?;
    let mut functions = BTreeMap::new();
    for (name, code) in program.functions {
        if let Some(old_code) = previous.functions.get(&FunctionSymbolId::new(&name)) {
            ensure_compatible_params(&name, old_code, &code)?;
        }
        functions.insert(FunctionSymbolId::new(name), Arc::new(code));
    }
    previous.abi().ensure_compatible_update(&abi)?;
    Ok(HotUpdate::new(functions, abi))
}

fn ensure_compatible_params(
    name: &str,
    old_code: &CodeObject,
    new_code: &CodeObject,
) -> HotReloadResult<()> {
    if new_code.params.len() < old_code.params.len() {
        return Err(HotReloadError::new(
            HotReloadErrorKind::DeletedFunctionParameters {
                function: name.to_owned(),
                old: old_code.params.clone(),
                new: new_code.params.clone(),
            },
        ));
    }
    Ok(())
}
