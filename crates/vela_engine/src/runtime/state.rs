use super::{
    RuntimeGlobalStore, RuntimeScriptGlobalStore, bytecode_profile::RuntimeBytecodeProfile,
    image::RuntimeImage, inline_cache::InlineCaches, next_runtime_id,
};

pub(super) struct RuntimeState {
    pub(super) id: u64,
    pub(super) globals: RuntimeGlobalStore,
    pub(super) script_globals: RuntimeScriptGlobalStore,
    pub(super) inline_caches: InlineCaches,
    pub(super) bytecode_profile: RuntimeBytecodeProfile,
}

impl RuntimeState {
    pub(super) fn for_image(image: &RuntimeImage) -> Self {
        Self {
            id: next_runtime_id(),
            globals: RuntimeGlobalStore::with_global_layout(image.global_names()),
            script_globals: RuntimeScriptGlobalStore::with_global_layout(image.global_names()),
            inline_caches: InlineCaches::for_image(image),
            bytecode_profile: RuntimeBytecodeProfile::for_image(image),
        }
    }

    pub(super) fn set_global_layout(&mut self, names: &[String]) {
        self.globals.set_global_layout(names);
        self.script_globals.set_global_layout(names);
    }

    pub(super) fn rebind_to_image(&mut self, image: &RuntimeImage) {
        self.set_global_layout(image.global_names());
        self.inline_caches.clear_for_image(image);
        self.bytecode_profile.clear_for_image(image);
    }
}
