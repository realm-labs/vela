use vela_common::SourceId;
use vela_hot_reload::{
    HotReloadAbi, HotReloadResult, HotUpdate, ProgramVersion, compile_initial_with_abi_and_options,
    compile_update_with_abi_and_options_and_policy,
};

use crate::Engine;

impl Engine {
    #[must_use]
    pub fn hot_reload_abi(&self) -> HotReloadAbi {
        HotReloadAbi::from_registry(&self.registry())
    }

    pub fn compile_hot_reload_initial(
        &self,
        source: SourceId,
        text: &str,
    ) -> HotReloadResult<ProgramVersion> {
        compile_initial_with_abi_and_options(
            source,
            text,
            self.hot_reload_abi(),
            &self.compiler_options(),
        )
    }

    pub fn compile_hot_reload_update(
        &self,
        previous: &ProgramVersion,
        source: SourceId,
        text: &str,
    ) -> HotReloadResult<HotUpdate> {
        compile_update_with_abi_and_options_and_policy(
            previous,
            source,
            text,
            self.hot_reload_abi(),
            &self.compiler_options(),
            self.hot_reload_policy(),
        )
    }
}
