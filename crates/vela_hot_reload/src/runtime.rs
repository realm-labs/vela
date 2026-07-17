use std::sync::{Arc, Mutex, MutexGuard};

use crate::error::HotReloadResult;
use crate::report::HotReloadReport;
use crate::symbol::ProgramVersionId;
use crate::version::{HotUpdate, ProgramVersion};

/// A cloneable producer for the pending hot-update slot.
///
/// Staging never changes the active generation. Only `HotReloadRuntime` can
/// consume the slot at an explicit reload safe point.
#[derive(Clone, Debug)]
pub struct HotReloadStagingHandle {
    pending: Arc<Mutex<Option<HotReloadResult<HotUpdate>>>>,
}

impl HotReloadStagingHandle {
    fn pending(&self) -> MutexGuard<'_, Option<HotReloadResult<HotUpdate>>> {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[must_use]
    pub fn has_pending_update(&self) -> bool {
        self.pending().is_some()
    }

    pub fn stage_hot_update(&self, update: HotUpdate) -> Option<HotReloadResult<HotUpdate>> {
        self.stage_hot_update_result(Ok(update))
    }

    pub fn stage_hot_update_result(
        &self,
        update: HotReloadResult<HotUpdate>,
    ) -> Option<HotReloadResult<HotUpdate>> {
        self.pending().replace(update)
    }
}

#[derive(Clone, Debug)]
pub struct HotReloadRuntime {
    current: Arc<ProgramVersion>,
    staging: HotReloadStagingHandle,
}

impl HotReloadRuntime {
    #[must_use]
    pub fn new(initial: ProgramVersion) -> Self {
        Self {
            current: Arc::new(initial),
            staging: HotReloadStagingHandle {
                pending: Arc::new(Mutex::new(None)),
            },
        }
    }

    #[must_use]
    pub fn current(&self) -> Arc<ProgramVersion> {
        Arc::clone(&self.current)
    }

    #[must_use]
    pub fn has_pending_update(&self) -> bool {
        self.staging.has_pending_update()
    }

    #[must_use]
    pub fn staging_handle(&self) -> HotReloadStagingHandle {
        self.staging.clone()
    }

    pub fn stage_hot_update(&mut self, update: HotUpdate) -> Option<HotReloadResult<HotUpdate>> {
        self.stage_hot_update_result(Ok(update))
    }

    pub fn stage_hot_update_result(
        &mut self,
        update: HotReloadResult<HotUpdate>,
    ) -> Option<HotReloadResult<HotUpdate>> {
        self.staging.stage_hot_update_result(update)
    }

    #[must_use]
    pub fn check_reload(&mut self) -> Option<HotReloadReport> {
        let update = self.take_pending_update();
        update.map(|update| self.apply_hot_update_result_report(update))
    }

    /// Removes the pending update without activating it so an embedding can
    /// perform per-Runtime staging before calling an apply method.
    pub fn take_pending_update(&mut self) -> Option<HotReloadResult<HotUpdate>> {
        self.staging.pending().take()
    }

    pub fn apply_hot_update(&mut self, update: HotUpdate) -> HotReloadResult<Arc<ProgramVersion>> {
        let report = self.apply_hot_update_report(update);
        Ok(report
            .version()
            .expect("accepted hot reload report should carry a version"))
    }

    #[must_use]
    pub fn apply_hot_update_report(&mut self, update: HotUpdate) -> HotReloadReport {
        let from_version = self.current.id;
        let HotUpdate {
            abi,
            changes,
            artifact,
        } = update;
        let next = Arc::new(ProgramVersion {
            id: ProgramVersionId(self.current.id.0.saturating_add(1)),
            abi: Arc::new(abi),
            artifact,
        });
        self.current = Arc::clone(&next);
        HotReloadReport::accepted(from_version, next, changes)
    }

    #[must_use]
    pub fn apply_hot_update_result_report(
        &mut self,
        update: HotReloadResult<HotUpdate>,
    ) -> HotReloadReport {
        match update {
            Ok(update) => self.apply_hot_update_report(update),
            Err(error) => HotReloadReport::rejected(self.current.id, error),
        }
    }
}
