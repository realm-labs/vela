use vela_bytecode::Program;
use vela_hot_reload::profile::ProgramProfile;
use vela_hot_reload::symbol::ProgramVersionId;
use vela_hot_reload::version::ProgramVersion;

use crate::engine::Engine;

pub(super) struct RuntimeImage {
    engine: Engine,
    program: Program,
    version_id: Option<ProgramVersionId>,
    layout: RuntimeImageLayout,
    #[allow(dead_code)]
    profile: Option<ProgramProfile>,
}

pub(super) struct RuntimeImageLayout {
    global_names: Vec<String>,
}

impl RuntimeImage {
    pub(super) fn new(engine: Engine, program: Program) -> Self {
        let layout = RuntimeImageLayout::from_global_names(program.global_names());
        Self {
            engine,
            program,
            version_id: None,
            layout,
            profile: None,
        }
    }

    pub(super) fn from_program_version(engine: Engine, version: &ProgramVersion) -> Self {
        let version_id = Some(version.id);
        let layout = RuntimeImageLayout::from_global_names(version.global_names());
        let profile = Some(version.profile().clone());
        let program = version.to_program();
        Self {
            engine,
            program,
            version_id,
            layout,
            profile,
        }
    }

    pub(super) const fn engine(&self) -> &Engine {
        &self.engine
    }

    pub(super) const fn program(&self) -> &Program {
        &self.program
    }

    pub(super) fn global_names(&self) -> &[String] {
        self.layout.global_names()
    }

    pub(super) fn current_program_version_id(&self) -> Option<ProgramVersionId> {
        self.version_id
    }
}

impl RuntimeImageLayout {
    fn from_global_names(names: &[String]) -> Self {
        Self {
            global_names: names.to_vec(),
        }
    }

    fn global_names(&self) -> &[String] {
        &self.global_names
    }
}
