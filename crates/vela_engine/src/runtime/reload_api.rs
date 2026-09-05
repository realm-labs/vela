use std::fmt;
use std::path::Path;
use std::sync::Arc;

use vela_bytecode::LinkError;
#[cfg(test)]
use vela_common::SourceId;
use vela_hot_reload::error::HotReloadResult;
use vela_hot_reload::report::HotReloadReport;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_hot_reload::version::{HotUpdate, ProgramVersion};

use super::{HotReloadStagingHandle, RuntimeImageStorage, RuntimeImpl};
use crate::reload::{
    EngineHotReloadSourceError, EngineHotReloadSourceErrorKind, EngineHotReloadSourceResult,
};
use crate::source::EngineSourceError;

/// Source input for one ordinary Vela generation update.
#[derive(Clone, Copy, Debug)]
pub enum ReloadSource<'source> {
    Text(&'source str),
    File(&'source Path),
    Directory(&'source Path),
    ChangedFile {
        root: &'source Path,
        changed_file: &'source Path,
    },
}

impl<'source> ReloadSource<'source> {
    #[must_use]
    pub const fn text(source: &'source str) -> Self {
        Self::Text(source)
    }

    #[must_use]
    pub fn file(path: &'source impl AsRef<Path>) -> Self {
        Self::File(path.as_ref())
    }

    #[must_use]
    pub fn directory(root: &'source impl AsRef<Path>) -> Self {
        Self::Directory(root.as_ref())
    }

    #[must_use]
    pub fn changed_file(
        root: &'source impl AsRef<Path>,
        changed_file: &'source impl AsRef<Path>,
    ) -> Self {
        Self::ChangedFile {
            root: root.as_ref(),
            changed_file: changed_file.as_ref(),
        }
    }
}

impl<'source> From<&'source str> for ReloadSource<'source> {
    fn from(source: &'source str) -> Self {
        Self::text(source)
    }
}

#[derive(Debug, PartialEq)]
pub enum RuntimeReloadError {
    NotEnabled,
    Source(EngineSourceError),
    Link(LinkError),
}

impl fmt::Display for RuntimeReloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotEnabled => formatter.write_str("Runtime hot reload is not enabled"),
            Self::Source(error) => error.fmt(formatter),
            Self::Link(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RuntimeReloadError {}

impl<I> RuntimeImpl<I>
where
    I: RuntimeImageStorage,
{
    #[must_use]
    pub fn hot_reload_version(&self) -> Option<Arc<ProgramVersion>> {
        self.hot_reload.as_ref().map(HotReloadRuntime::current)
    }

    /// Returns a staging-only handle that may queue an update while an async
    /// outer call has the Runtime borrowed.
    ///
    /// The handle cannot activate the update. Call [`Self::activate_reload`]
    /// after the outer call completes or is cancelled.
    #[must_use]
    pub fn hot_reload_staging_handle(&self) -> Option<HotReloadStagingHandle> {
        self.hot_reload
            .as_ref()
            .map(HotReloadRuntime::staging_handle)
    }

    /// Compiles and stages one complete ordinary Vela source generation.
    ///
    /// Source and link failures return immediately. ABI and policy rejection
    /// remain pending so [`Self::activate_reload`] can produce the structured
    /// rejected report at the Runtime safe point.
    pub fn stage_reload<'source>(
        &mut self,
        source: impl Into<ReloadSource<'source>>,
    ) -> Result<(), RuntimeReloadError> {
        let previous = self.current_reload_version()?;
        let update = match source.into() {
            ReloadSource::Text(source) => self
                .image
                .engine()
                .compile_hot_reload_update(&previous, source),
            ReloadSource::File(path) => self
                .image
                .engine()
                .compile_hot_reload_update_file(&previous, path),
            ReloadSource::Directory(root) => self
                .image
                .engine()
                .compile_hot_reload_update_dir(&previous, root),
            ReloadSource::ChangedFile { root, changed_file } => self
                .image
                .engine()
                .compile_hot_reload_update_changed_file(&previous, root, changed_file),
        };
        self.stage_source_result(update)
    }

    /// Stages a precompiled update, such as one produced by package tooling.
    pub fn stage_reload_update(&mut self, update: HotUpdate) -> Result<(), RuntimeReloadError> {
        self.stage_update_result(Ok(update))
    }

    pub fn has_pending_reload(&self) -> Result<bool, RuntimeReloadError> {
        let hot_reload = self
            .hot_reload
            .as_ref()
            .ok_or(RuntimeReloadError::NotEnabled)?;
        Ok(hot_reload.has_pending_update())
    }

    /// Activates the pending update at the caller-selected Runtime safe point.
    pub fn activate_reload(&mut self) -> Result<Option<HotReloadReport>, RuntimeReloadError> {
        let hot_reload = self
            .hot_reload
            .as_mut()
            .ok_or(RuntimeReloadError::NotEnabled)?;
        let update = hot_reload.take_pending_update();
        let report = update.map(|update| self.apply_update_report(update));
        self.state.reclaim_dead_generations();
        Ok(report)
    }

    fn current_reload_version(&self) -> Result<Arc<ProgramVersion>, RuntimeReloadError> {
        self.hot_reload_version()
            .ok_or(RuntimeReloadError::NotEnabled)
    }

    fn stage_source_result(
        &mut self,
        update: EngineHotReloadSourceResult<HotUpdate>,
    ) -> Result<(), RuntimeReloadError> {
        match update {
            Ok(update) => self.stage_reload_update(update),
            Err(EngineHotReloadSourceError {
                kind: EngineHotReloadSourceErrorKind::Source(error),
            }) => Err(RuntimeReloadError::Source(error)),
            Err(EngineHotReloadSourceError {
                kind: EngineHotReloadSourceErrorKind::Link(error),
            }) => Err(RuntimeReloadError::Link(error)),
            Err(EngineHotReloadSourceError {
                kind: EngineHotReloadSourceErrorKind::HotReload(error),
            }) => self.stage_update_result(Err(error)),
        }
    }

    fn stage_update_result(
        &mut self,
        update: HotReloadResult<HotUpdate>,
    ) -> Result<(), RuntimeReloadError> {
        let hot_reload = self
            .hot_reload
            .as_mut()
            .ok_or(RuntimeReloadError::NotEnabled)?;
        let _replaced = hot_reload.stage_hot_update_result(update);
        Ok(())
    }

    fn apply_update_report(&mut self, update: HotReloadResult<HotUpdate>) -> HotReloadReport {
        let current = self
            .hot_reload
            .as_ref()
            .map(HotReloadRuntime::current)
            .expect("pending reload requires an enabled Runtime");
        let update = match update {
            Ok(update) => update,
            Err(error) => return HotReloadReport::rejected(current.id, error),
        };
        if let Err(error) = self
            .hot_reload
            .as_ref()
            .expect("pending reload requires an enabled Runtime")
            .validate_hot_update(&update)
        {
            return HotReloadReport::rejected(current.id, error);
        }
        let staging = match self
            .prepare_hot_update_state(&update, super::RuntimeInitializationLimits::default())
        {
            Ok(staging) => staging,
            Err(error) => return HotReloadReport::rejected(current.id, error),
        };
        let next_states = update.linked_artifact().image().states().to_vec();
        let report = self
            .hot_reload
            .as_mut()
            .expect("pending reload requires an enabled Runtime")
            .apply_hot_update_report(update);
        if report.accepted {
            self.commit_hot_update_state(&next_states, staging);
        }
        self.rebind_image_from_reload_report(Some(&report));
        report
    }

    #[cfg(test)]
    pub(crate) fn compile_reload_with_id(
        &self,
        source: SourceId,
        text: &str,
    ) -> Result<EngineHotReloadSourceResult<HotUpdate>, RuntimeReloadError> {
        let previous = self.current_reload_version()?;
        Ok(self
            .image
            .engine()
            .compile_reload_with_id(&previous, source, text))
    }

    #[cfg(test)]
    pub(crate) fn compile_reload_file_for_test(
        &self,
        path: &Path,
    ) -> Result<EngineHotReloadSourceResult<HotUpdate>, RuntimeReloadError> {
        let previous = self.current_reload_version()?;
        Ok(self
            .image
            .engine()
            .compile_hot_reload_update_file(&previous, path))
    }

    #[cfg(test)]
    pub(crate) fn compile_reload_changed_file_for_test(
        &self,
        root: &Path,
        changed_file: &Path,
    ) -> Result<EngineHotReloadSourceResult<HotUpdate>, RuntimeReloadError> {
        let previous = self.current_reload_version()?;
        Ok(self.image.engine().compile_hot_reload_update_changed_file(
            &previous,
            root,
            changed_file,
        ))
    }

    #[cfg(test)]
    pub(crate) fn apply_reload_update_for_test(
        &mut self,
        update: HotUpdate,
    ) -> Result<HotReloadReport, RuntimeReloadError> {
        self.current_reload_version()?;
        Ok(self.apply_update_report(Ok(update)))
    }

    #[cfg(test)]
    pub(crate) fn apply_reload_result_for_test(
        &mut self,
        update: HotReloadResult<HotUpdate>,
    ) -> Result<HotReloadReport, RuntimeReloadError> {
        self.current_reload_version()?;
        Ok(self.apply_update_report(update))
    }

    #[cfg(test)]
    pub(crate) fn stage_reload_result_for_test(
        &mut self,
        update: HotReloadResult<HotUpdate>,
    ) -> Result<(), RuntimeReloadError> {
        self.stage_update_result(update)
    }
}
