use super::{RuntimeGlobalStore, RuntimeScriptGlobalStore, next_runtime_id};

pub(super) struct RuntimeState {
    pub(super) id: u64,
    pub(super) globals: RuntimeGlobalStore,
    pub(super) script_globals: RuntimeScriptGlobalStore,
}

impl RuntimeState {
    pub(super) fn with_global_layout(names: &[String]) -> Self {
        Self {
            id: next_runtime_id(),
            globals: RuntimeGlobalStore::with_global_layout(names),
            script_globals: RuntimeScriptGlobalStore::with_global_layout(names),
        }
    }

    pub(super) fn set_global_layout(&mut self, names: &[String]) {
        self.globals.set_global_layout(names);
        self.script_globals.set_global_layout(names);
    }
}
