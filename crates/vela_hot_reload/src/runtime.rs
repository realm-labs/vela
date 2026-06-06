use std::sync::Arc;

use crate::error::HotReloadResult;
use crate::profile::ProgramProfile;
use crate::report::HotReloadReport;
use crate::symbol::ProgramVersionId;
use crate::version::{HotUpdate, ProgramVersion};

#[derive(Clone, Debug, PartialEq)]
pub struct HotReloadRuntime {
    current: Arc<ProgramVersion>,
    pending: Option<HotReloadResult<HotUpdate>>,
}

impl HotReloadRuntime {
    #[must_use]
    pub fn new(initial: ProgramVersion) -> Self {
        Self {
            current: Arc::new(initial),
            pending: None,
        }
    }

    #[must_use]
    pub fn current(&self) -> Arc<ProgramVersion> {
        Arc::clone(&self.current)
    }

    #[must_use]
    pub fn has_pending_update(&self) -> bool {
        self.pending.is_some()
    }

    pub fn stage_hot_update(&mut self, update: HotUpdate) -> Option<HotReloadResult<HotUpdate>> {
        self.stage_hot_update_result(Ok(update))
    }

    pub fn stage_hot_update_result(
        &mut self,
        update: HotReloadResult<HotUpdate>,
    ) -> Option<HotReloadResult<HotUpdate>> {
        self.pending.replace(update)
    }

    #[must_use]
    pub fn check_reload(&mut self) -> Option<HotReloadReport> {
        self.pending
            .take()
            .map(|update| self.apply_hot_update_result_report(update))
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
        let changes = update.changes;
        let mut functions = self.current.functions.clone();
        for (name, function) in update.functions {
            functions.insert(name, function);
        }
        let profile = ProgramProfile::from_functions(&functions);
        let next = Arc::new(ProgramVersion {
            id: ProgramVersionId(self.current.id.0.saturating_add(1)),
            functions,
            script_methods: update.script_methods,
            script_metadata: update.script_metadata,
            abi: update.abi,
            profile,
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
