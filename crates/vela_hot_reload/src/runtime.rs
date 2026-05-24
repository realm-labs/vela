use std::sync::Arc;

use crate::{HotReloadReport, HotReloadResult, HotUpdate, ProgramVersion, ProgramVersionId};

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
        let report = self.apply_hot_update_report(update);
        Ok(report
            .version()
            .expect("accepted hot reload report should carry a version"))
    }

    #[must_use]
    pub fn apply_hot_update_report(&mut self, update: HotUpdate) -> HotReloadReport {
        let from_version = self.current.id;
        let changed_functions = update.functions.keys().cloned().collect::<Vec<_>>();
        let mut functions = self.current.functions.clone();
        for (name, function) in update.functions {
            functions.insert(name, function);
        }
        let next = Arc::new(ProgramVersion {
            id: ProgramVersionId(self.current.id.0.saturating_add(1)),
            functions,
            abi: update.abi,
        });
        self.current = Arc::clone(&next);
        HotReloadReport::accepted(from_version, next, changed_functions)
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
