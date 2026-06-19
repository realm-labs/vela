use std::collections::BTreeSet;

use vela_language_service::WorkspaceConfig;

use crate::{LaunchConfiguration, config::EditorConfiguration};

#[derive(Debug, Default)]
pub(crate) struct ConfigChange {
    watch_files_enabled: Option<bool>,
    workspace_roots: Option<BTreeSet<String>>,
    editor_config: Option<EditorConfiguration>,
    workspace_config: WorkspaceConfigChange,
}

impl ConfigChange {
    pub(crate) fn from_launch(configuration: LaunchConfiguration) -> Self {
        Self {
            watch_files_enabled: Some(configuration.watch_files_enabled()),
            editor_config: configuration.into_editor_configuration(),
            workspace_config: WorkspaceConfigChange::RecomputeFromEditor,
            ..Self::default()
        }
    }

    pub(crate) fn from_initialize(
        workspace_roots: BTreeSet<String>,
        editor_config: Option<EditorConfiguration>,
    ) -> Self {
        Self {
            workspace_roots: Some(workspace_roots),
            editor_config,
            workspace_config: WorkspaceConfigChange::RecomputeFromEditor,
            ..Self::default()
        }
    }

    pub(crate) fn from_editor_settings(editor_config: EditorConfiguration) -> Self {
        Self {
            editor_config: Some(editor_config),
            workspace_config: WorkspaceConfigChange::RecomputeFromEditor,
            ..Self::default()
        }
    }

    pub(crate) fn from_workspace_roots(workspace_roots: BTreeSet<String>) -> Self {
        Self {
            workspace_roots: Some(workspace_roots),
            workspace_config: WorkspaceConfigChange::RecomputeFromEditor,
            ..Self::default()
        }
    }

    pub(crate) fn from_workspace_file(config: WorkspaceConfig) -> Self {
        Self {
            workspace_config: WorkspaceConfigChange::WorkspaceFile(config),
            ..Self::default()
        }
    }

    pub(crate) fn clear_workspace_file() -> Self {
        Self {
            workspace_config: WorkspaceConfigChange::ClearWorkspaceFile,
            ..Self::default()
        }
    }

    pub(crate) fn watch_files_enabled(&self) -> Option<bool> {
        self.watch_files_enabled
    }

    pub(crate) fn take_workspace_roots(&mut self) -> Option<BTreeSet<String>> {
        self.workspace_roots.take()
    }

    pub(crate) fn take_editor_config(&mut self) -> Option<EditorConfiguration> {
        self.editor_config.take()
    }

    pub(crate) fn workspace_config_change(&mut self) -> WorkspaceConfigChange {
        std::mem::take(&mut self.workspace_config)
    }
}

#[derive(Debug, Default)]
pub(crate) enum WorkspaceConfigChange {
    #[default]
    Unchanged,
    RecomputeFromEditor,
    WorkspaceFile(WorkspaceConfig),
    ClearWorkspaceFile,
}
