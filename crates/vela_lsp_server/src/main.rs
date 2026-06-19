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
    let mut saw_profile = false;
    let mut saw_profile_slow_ms = false;
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
            "--profile" => {
                if saw_profile {
                    return invalid_input("duplicate `--profile` flag");
                }
                let profile = required_value(&args, index, "--profile")?;
                configuration.set_profile_path(normalize_cli_path(profile)?);
                saw_profile = true;
                index += 2;
            }
            "--profile-slow-ms" => {
                if saw_profile_slow_ms {
                    return invalid_input("duplicate `--profile-slow-ms` flag");
                }
                let slow_ms = required_value(&args, index, "--profile-slow-ms")?;
                let slow_ms = slow_ms.parse::<u64>().map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("invalid `--profile-slow-ms` value `{slow_ms}`: {error}"),
                    )
                })?;
                configuration.set_profile_slow_ms(slow_ms);
                saw_profile_slow_ms = true;
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
    "Usage: vela_lsp_server [--stdio] [--root <path-or-file-uri>]... [--schema <path-or-file-uri>] [--profile <jsonl-path>] [--profile-slow-ms <ms>]\n       vela_lsp_server --version"
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
            "--profile".to_owned(),
            "/tmp/vela-lsp-profile.jsonl".to_owned(),
            "--profile-slow-ms".to_owned(),
            "25".to_owned(),
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
        assert_eq!(
            configuration.profile_path(),
            Some("/tmp/vela-lsp-profile.jsonl")
        );
        assert_eq!(configuration.profile_slow_ms(), 25);
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

    #[test]
    fn cli_profile_slow_ms_must_be_numeric() {
        let error = parse_args(vec![
            "--profile".to_owned(),
            "/tmp/vela-lsp-profile.jsonl".to_owned(),
            "--profile-slow-ms".to_owned(),
            "slow".to_owned(),
        ])
        .expect_err("profile slow threshold should be numeric");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("invalid `--profile-slow-ms`"));
    }
}
