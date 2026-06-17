use zed_extension_api::{self as zed, Command, LanguageServerId, Result, Worktree};

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
        let command = worktree
            .which("vela_lsp_server")
            .unwrap_or_else(|| "vela_lsp_server".to_owned());
        Ok(Command {
            command,
            args: vec!["--stdio".to_owned()],
            env: Default::default(),
        })
    }
}

zed::register_extension!(VelaExtension);
