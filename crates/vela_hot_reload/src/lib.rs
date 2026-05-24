//! Function-level hot reload program versioning.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use vela_bytecode::compiler::{CompileError, compile_program_source};
use vela_bytecode::{CodeObject, Program};
use vela_common::SourceId;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionSymbolId(pub String);

impl FunctionSymbolId {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProgramVersionId(pub u64);

#[derive(Clone, Debug, PartialEq)]
pub struct ProgramVersion {
    pub id: ProgramVersionId,
    functions: BTreeMap<FunctionSymbolId, Arc<CodeObject>>,
}

impl ProgramVersion {
    #[must_use]
    pub fn from_program(id: ProgramVersionId, program: Program) -> Self {
        let functions = program
            .functions
            .into_iter()
            .map(|(name, code)| (FunctionSymbolId::new(name), Arc::new(code)))
            .collect();
        Self { id, functions }
    }

    #[must_use]
    pub fn function(&self, name: &str) -> Option<Arc<CodeObject>> {
        self.functions.get(&FunctionSymbolId::new(name)).cloned()
    }

    #[must_use]
    pub fn to_program(&self) -> Program {
        let mut program = Program::new();
        for function in self.functions.values() {
            program.insert_function((**function).clone());
        }
        program
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HotReloadRuntime {
    current: Arc<ProgramVersion>,
}

impl HotReloadRuntime {
    #[must_use]
    pub fn new(initial: ProgramVersion) -> Self {
        Self {
            current: Arc::new(initial),
        }
    }

    #[must_use]
    pub fn current(&self) -> Arc<ProgramVersion> {
        Arc::clone(&self.current)
    }

    pub fn apply_hot_update(&mut self, update: HotUpdate) -> HotReloadResult<Arc<ProgramVersion>> {
        let mut functions = self.current.functions.clone();
        for (name, function) in update.functions {
            functions.insert(name, function);
        }
        let next = Arc::new(ProgramVersion {
            id: ProgramVersionId(self.current.id.0.saturating_add(1)),
            functions,
        });
        self.current = Arc::clone(&next);
        Ok(next)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HotUpdate {
    functions: BTreeMap<FunctionSymbolId, Arc<CodeObject>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HotReloadError {
    pub kind: HotReloadErrorKind,
}

impl HotReloadError {
    fn new(kind: HotReloadErrorKind) -> Self {
        Self { kind }
    }
}

impl fmt::Display for HotReloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl std::error::Error for HotReloadError {}

#[derive(Clone, Debug, PartialEq)]
pub enum HotReloadErrorKind {
    Compile(CompileError),
    DeletedFunctionParameters {
        function: String,
        old: Vec<String>,
        new: Vec<String>,
    },
}

pub type HotReloadResult<T> = Result<T, HotReloadError>;

pub fn compile_initial(source: SourceId, text: &str) -> HotReloadResult<ProgramVersion> {
    let program = compile_program_source(source, text)
        .map_err(|error| HotReloadError::new(HotReloadErrorKind::Compile(error)))?;
    Ok(ProgramVersion::from_program(ProgramVersionId(0), program))
}

pub fn compile_update(
    previous: &ProgramVersion,
    source: SourceId,
    text: &str,
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
    Ok(HotUpdate { functions })
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

#[cfg(test)]
mod tests {
    use super::*;
    use vela_vm::{Value, Vm};

    #[test]
    fn new_calls_enter_new_code_after_update() {
        let initial =
            compile_initial(SourceId::new(1), "fn main() { return 20; }").expect("compile initial");
        let mut runtime = HotReloadRuntime::new(initial);
        let update = compile_update(
            &runtime.current(),
            SourceId::new(2),
            "fn main() { return 30; }",
        )
        .expect("compile update");

        runtime.apply_hot_update(update).expect("apply update");

        assert_eq!(
            Vm::new().run_program(&runtime.current().to_program(), "main", &[]),
            Ok(Value::Int(30))
        );
    }

    #[test]
    fn old_version_lifetime_preserves_old_code() {
        let initial =
            compile_initial(SourceId::new(1), "fn main() { return 20; }").expect("compile initial");
        let mut runtime = HotReloadRuntime::new(initial);
        let old = runtime.current();
        let update =
            compile_update(&old, SourceId::new(2), "fn main() { return 30; }").expect("update");

        let new = runtime.apply_hot_update(update).expect("apply update");

        assert_eq!(
            Vm::new().run_program(&old.to_program(), "main", &[]),
            Ok(Value::Int(20))
        );
        assert_eq!(
            Vm::new().run_program(&new.to_program(), "main", &[]),
            Ok(Value::Int(30))
        );
    }

    #[test]
    fn deleted_function_parameters_are_rejected() {
        let initial = compile_initial(SourceId::new(1), "fn main(value) { return value; }")
            .expect("compile initial");

        let error = compile_update(&initial, SourceId::new(2), "fn main() { return 0; }")
            .expect_err("deleted param");

        assert_eq!(
            error.kind,
            HotReloadErrorKind::DeletedFunctionParameters {
                function: "main".to_owned(),
                old: vec!["value".to_owned()],
                new: Vec::new(),
            }
        );
    }

    #[test]
    fn new_private_helper_functions_are_accepted() {
        let initial =
            compile_initial(SourceId::new(1), "fn main() { return 1; }").expect("initial");
        let mut runtime = HotReloadRuntime::new(initial);
        let update = compile_update(
            &runtime.current(),
            SourceId::new(2),
            r#"
fn helper() {
    return 7;
}

fn main() {
    return helper();
}
"#,
        )
        .expect("helper update");

        runtime.apply_hot_update(update).expect("apply update");

        assert_eq!(
            Vm::new().run_program(&runtime.current().to_program(), "main", &[]),
            Ok(Value::Int(7))
        );
    }
}
