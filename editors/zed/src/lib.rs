use zed_extension_api::{self as zed, Command, LanguageServerId, Result, Worktree};

const PRIMARY_LSP_SETTINGS_ID: &str = "vela-language-server";
const COMPAT_LSP_SETTINGS_ID: &str = "vela";
const SERVER_BINARY: &str = "vela_lsp_server";
const DEFAULT_SERVER_ARGS: &[&str] = &["--stdio"];

struct VelaExtension;

impl zed::Extension for VelaExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Command> {
        if let Some(command) = configured_server_command(PRIMARY_LSP_SETTINGS_ID, worktree)? {
            return Ok(command);
        }
        if let Some(command) = configured_server_command(COMPAT_LSP_SETTINGS_ID, worktree)? {
            return Ok(command);
        }

        let command = worktree
            .which(SERVER_BINARY)
            .unwrap_or_else(|| SERVER_BINARY.to_owned());
        Ok(Command {
            command,
            args: default_server_args(),
            env: Default::default(),
        })
    }
}

fn configured_server_command(settings_id: &str, worktree: &Worktree) -> Result<Option<Command>> {
    let settings = zed::settings::LspSettings::for_worktree(settings_id, worktree)?;
    Ok(settings.binary.and_then(command_from_binary_settings))
}

fn command_from_binary_settings(binary: zed::settings::CommandSettings) -> Option<Command> {
    let command = binary.path?.trim().to_owned();
    if command.is_empty() {
        return None;
    }

    let args = binary.arguments.unwrap_or_else(default_server_args);
    let mut env = binary
        .env
        .unwrap_or_default()
        .into_iter()
        .collect::<Vec<_>>();
    env.sort_by(|left, right| left.0.cmp(&right.0));

    Some(Command { command, args, env })
}

fn default_server_args() -> Vec<String> {
    DEFAULT_SERVER_ARGS
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect()
}

zed::register_extension!(VelaExtension);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn configured_binary_uses_path_arguments_and_env() {
        let mut env = HashMap::new();
        env.insert(
            "VELA_SCHEMA".to_owned(),
            "target/vela/schema.json".to_owned(),
        );
        env.insert("RUST_LOG".to_owned(), "debug".to_owned());

        let command = command_from_binary_settings(zed::settings::CommandSettings {
            path: Some("/tmp/vela_lsp_server".to_owned()),
            arguments: Some(vec![
                "--stdio".to_owned(),
                "--schema".to_owned(),
                "target/vela/schema.json".to_owned(),
            ]),
            env: Some(env),
        })
        .expect("configured binary should produce a command");

        assert_eq!(command.command, "/tmp/vela_lsp_server");
        assert_eq!(
            command.args,
            vec!["--stdio", "--schema", "target/vela/schema.json"]
        );
        assert_eq!(
            command.env,
            vec![
                ("RUST_LOG".to_owned(), "debug".to_owned()),
                (
                    "VELA_SCHEMA".to_owned(),
                    "target/vela/schema.json".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn configured_binary_defaults_to_stdio_args() {
        let command = command_from_binary_settings(zed::settings::CommandSettings {
            path: Some("/tmp/vela_lsp_server".to_owned()),
            arguments: None,
            env: None,
        })
        .expect("configured binary should produce a command");

        assert_eq!(command.args, vec!["--stdio"]);
    }

    #[test]
    fn configured_binary_ignores_missing_or_blank_paths() {
        assert!(
            command_from_binary_settings(zed::settings::CommandSettings {
                path: None,
                arguments: Some(vec!["--stdio".to_owned()]),
                env: None,
            })
            .is_none()
        );

        assert!(
            command_from_binary_settings(zed::settings::CommandSettings {
                path: Some("  ".to_owned()),
                arguments: Some(vec!["--stdio".to_owned()]),
                env: None,
            })
            .is_none()
        );
    }
}
