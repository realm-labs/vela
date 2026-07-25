use vela_hot_reload::report::HotReloadReport;
use vela_hot_reload::symbol::ProgramVersionId;
use vela_hot_reload::version::ProgramVersion;
use vela_vm::error::VmResult;

use super::image::{RuntimeImage, RuntimeImageStorage};
use super::value_support::value_type_id;
use super::{RuntimeImpl, VelaValue};
use crate::error::{EngineError, EngineErrorKind, EngineResult};

impl<I> RuntimeImpl<I>
where
    I: RuntimeImageStorage,
{
    pub(super) fn check_vela_value_runtime(&self, value: &VelaValue) -> VmResult<()> {
        if value.runtime_id == self.state.id {
            return Ok(());
        }
        Err(super::call_args::call_args_type_error(
            "VelaValue belongs to another Runtime",
        ))
    }

    pub(super) fn current_program_version_id(&self) -> Option<ProgramVersionId> {
        self.image.current_program_version_id()
    }

    pub(crate) fn active_binding_schema(&self) -> &vela_bytecode::RustBindingSchema {
        self.image.linked_artifact().binding_schema()
    }

    pub(super) fn value_type_id(&self, value: &VelaValue) -> Option<vela_def::TypeId> {
        value_type_id(
            &value.value,
            &self.state.vm_states.heap,
            self.image.engine().registry().as_ref(),
            |handle| self.state.host_slots.resolve(handle),
        )
    }

    pub(super) fn current_hot_reload_version(
        &self,
    ) -> EngineResult<std::sync::Arc<ProgramVersion>> {
        self.hot_reload_version()
            .ok_or_else(|| EngineError::new(EngineErrorKind::RuntimeNotHotReloadEnabled))
    }

    pub(super) fn rebind_image_from_reload_report(&mut self, report: Option<&HotReloadReport>) {
        let Some(version) = report.and_then(HotReloadReport::version) else {
            return;
        };
        self.image = I::from_runtime_image(RuntimeImage::from_program_version(
            self.image.engine().clone(),
            &version,
        ));
        self.state.rebind_to_image(&self.image);
    }
}
