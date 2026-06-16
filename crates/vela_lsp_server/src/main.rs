use std::env;
use std::io::{self, BufReader, BufWriter};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        None | Some("--stdio") => {
            if let Some(argument) = args.next() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unexpected argument `{argument}`"),
                ));
            }
            let stdin = io::stdin();
            let stdout = io::stdout();
            vela_lsp_server::stdio::run_stdio(
                BufReader::new(stdin.lock()),
                BufWriter::new(stdout.lock()),
            )
        }
        Some("--version") | Some("-V") => {
            println!("vela_lsp_server {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help") | Some("-h") => {
            println!("Usage: vela_lsp_server [--stdio]\n       vela_lsp_server --version");
            Ok(())
        }
        Some(argument) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown argument `{argument}`"),
        )),
    }
}
