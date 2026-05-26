use std::env;

mod demo;
mod diagnostics;
mod hot_reload_demo;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [flag, initial, updated] if flag == "--hot-reload" => {
            hot_reload_demo::run(initial, updated)
        }
        [path] => demo::run_script(path),
        _ => {
            Err("usage: vela_cli <script-path> | vela_cli --hot-reload <initial> <updated>".into())
        }
    }
}
