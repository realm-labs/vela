use std::error::Error;
use std::hint::black_box;

use vela_engine::engine::Engine;
use vela_engine::runtime::Runtime;
use vela_hot_reload::version::ProgramVersion;

const FUNCTION_COUNT: usize = 200;
const RUNTIME_COUNT: usize = 32;
const RETAINED_GENERATIONS: usize = 16;

fn main() -> Result<(), Box<dyn Error>> {
    let shape = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "top-level".to_owned());
    let engine = Engine::builder().build()?;

    match shape.as_str() {
        "top-level" => report(
            shape.as_str(),
            compile(&engine, &top_level_source(0))?,
            1,
            1,
        ),
        "lambda" => report(shape.as_str(), compile(&engine, &lambda_source(0))?, 1, 1),
        "shared-runtime" => {
            let version = compile(&engine, &lambda_source(0))?;
            let runtimes = (0..RUNTIME_COUNT)
                .map(|_| Runtime::from_hot_reload_version(engine.clone(), version.clone()))
                .collect::<Vec<_>>();
            black_box(&runtimes);
            report(shape.as_str(), version, runtimes.len(), 1);
        }
        "retained-generations" => {
            let versions = (0..RETAINED_GENERATIONS)
                .map(|generation| compile(&engine, &lambda_source(generation)))
                .collect::<Result<Vec<_>, _>>()?;
            let version = versions.last().expect("at least one generation").clone();
            black_box(&versions);
            report(shape.as_str(), version, 1, versions.len());
        }
        _ => return Err(format!("unknown generation-memory shape `{shape}`").into()),
    }

    Ok(())
}

fn compile(engine: &Engine, source: &str) -> Result<ProgramVersion, Box<dyn Error>> {
    Ok(engine.compile_hot_reload_initial(source)?)
}

fn top_level_source(generation: usize) -> String {
    let mut source = String::new();
    for index in 0..FUNCTION_COUNT {
        source.push_str(&format!(
            "fn function_{index}() {{ return {}; }}\n",
            index + generation
        ));
    }
    source.push_str("fn main() { return function_0(); }\n");
    source
}

fn lambda_source(generation: usize) -> String {
    let mut source = format!("fn main() {{ let seed = {generation};\n");
    for index in 0..FUNCTION_COUNT {
        source.push_str(&format!("let closure_{index} = || seed + {index};\n"));
    }
    source.push_str("return closure_0(); }\n");
    source
}

fn report(shape: &str, version: ProgramVersion, runtimes: usize, retained: usize) {
    let roots = version.verified_mir().roots().count();
    let executables = version.linked_program().function_count();
    let cache_sites = version.linked_artifact().cache_layout().len();
    let profile_slots = version
        .linked_artifact()
        .profile_layout()
        .functions()
        .iter()
        .map(|function| function.instruction_count)
        .sum::<usize>();
    println!(
        "shape={shape} roots={roots} executables={executables} cache_sites={cache_sites} profile_slots={profile_slots} runtimes={runtimes} retained_generations={retained}"
    );
    black_box(version);
}
