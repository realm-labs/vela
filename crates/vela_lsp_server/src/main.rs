use std::env;
use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;

use vela_lsp_server::LaunchConfiguration;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    match parse_args(env::args().skip(1))? {
        Command::Stdio(configuration) => {
            let stdin = io::stdin();
            let stdout = io::stdout();
            vela_lsp_server::stdio::run_stdio_with_configuration(
                BufReader::new(stdin.lock()),
                BufWriter::new(stdout.lock()),
                configuration,
            )
        }
        Command::Version => {
            println!("vela_lsp_server {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Help => {
            println!("{}", help_text());
            Ok(())
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Stdio(LaunchConfiguration),
    Version,
    Help,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> io::Result<Command> {
    let args = args.into_iter().collect::<Vec<_>>();
    if args.is_empty() {
        return Ok(Command::Stdio(LaunchConfiguration::new()));
    }
    if args.len() == 1 {
        match args[0].as_str() {
            "--version" | "-V" => return Ok(Command::Version),
            "--help" | "-h" => return Ok(Command::Help),
            _ => {}
        }
    }

    let mut configuration = LaunchConfiguration::new();
    let mut saw_stdio = false;
    let mut saw_schema = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--stdio" => {
                if saw_stdio {
                    return invalid_input("duplicate `--stdio` flag");
                }
                saw_stdio = true;
                index += 1;
            }
            "--root" => {
                let root = required_value(&args, index, "--root")?;
                configuration.add_workspace_root(normalize_cli_path(root)?);
                index += 2;
            }
            "--schema" => {
                if saw_schema {
                    return invalid_input("duplicate `--schema` flag");
                }
                let schema = required_value(&args, index, "--schema")?;
                configuration.set_host_schema(normalize_cli_path(schema)?);
                saw_schema = true;
                index += 2;
            }
            "--version" | "-V" => {
                return invalid_input("`--version` cannot be combined with stdio configuration");
            }
            "--help" | "-h" => {
                return invalid_input("`--help` cannot be combined with stdio configuration");
            }
            argument => return invalid_input(format!("unknown argument `{argument}`")),
        }
    }

    Ok(Command::Stdio(configuration))
}

fn required_value<'a>(args: &'a [String], index: usize, flag: &str) -> io::Result<&'a str> {
    let Some(value) = args.get(index + 1) else {
        return invalid_input(format!("missing value for `{flag}`"));
    };
    if value.starts_with('-') {
        return invalid_input(format!("missing value for `{flag}`"));
    }
    Ok(value)
}

fn normalize_cli_path(value: &str) -> io::Result<String> {
    if value.starts_with("file://") {
        return Ok(value.to_owned());
    }
    let path = PathBuf::from(value);
    let path = if path.is_absolute() {
        path
    } else {
        env::current_dir()?.join(path)
    };
    Ok(path.display().to_string())
}

fn invalid_input<T>(message: impl Into<String>) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::InvalidInput, message.into()))
}

fn help_text() -> &'static str {
    "Usage: vela_lsp_server [--stdio] [--root <path-or-file-uri>]... [--schema <path-or-file-uri>]\n       vela_lsp_server --version"
}

#[cfg(test)]
mod tests {
    use super::{Command, parse_args};

    #[test]
    fn cli_config_flags_parse_roots_and_schema() {
        let command = parse_args(vec![
            "--stdio".to_owned(),
            "--root".to_owned(),
            "file:///workspace/scripts".to_owned(),
            "--root".to_owned(),
            "file:///workspace/vendor".to_owned(),
            "--schema".to_owned(),
            "file:///workspace/target/vela/schema.json".to_owned(),
        ])
        .expect("config flags should parse");

        let Command::Stdio(configuration) = command else {
            panic!("config flags should run stdio");
        };
        assert_eq!(
            configuration.workspace_roots(),
            &[
                "file:///workspace/scripts".to_owned(),
                "file:///workspace/vendor".to_owned()
            ]
        );
        assert_eq!(
            configuration.host_schema(),
            Some("file:///workspace/target/vela/schema.json")
        );
    }

    #[test]
    fn cli_version_rejects_config_flags() {
        let error = parse_args(vec![
            "--version".to_owned(),
            "--root".to_owned(),
            ".".to_owned(),
        ])
        .expect_err("version should be standalone");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("cannot be combined"));
    }

    #[test]
    fn cli_config_flags_require_values() {
        let error =
            parse_args(vec!["--schema".to_owned()]).expect_err("schema should require a value");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("missing value"));
    }
}
