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
        [flag, path] if flag == "--allow-random" => demo::run_script_with_random(path),
        [flag, path] if flag == "--stale-player" => demo::run_script_with_stale_player(path),
        [path] => demo::run_script(path),
        _ => {
            Err("usage: vela_cli <script-path> | vela_cli --allow-random <script-path> | vela_cli --stale-player <script-path> | vela_cli --hot-reload <initial> <updated>".into())
        }
    }
}
