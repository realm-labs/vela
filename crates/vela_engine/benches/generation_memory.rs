use std::error::Error;
use std::hint::black_box;
use std::time::Instant;

use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
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
        "call-heavy" => call_heavy(&engine)?,
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

fn call_heavy_source(function_count: usize) -> String {
    let mut source = String::new();
    for index in 0..function_count {
        source.push_str(&format!("fn function_{index}() {{ return {index}; }}\n"));
    }
    source.push_str(
        "fn main() { let total = 0; for index in 0..2000 { total += function_0() + index - index; } return total; }\n",
    );
    source
}

fn call_heavy(engine: &Engine) -> Result<(), Box<dyn Error>> {
    let small = compile(engine, &call_heavy_source(1))?;
    let large = compile(engine, &call_heavy_source(FUNCTION_COUNT))?;
    let (small_elapsed, small_result) = measure_calls(engine, &small)?;
    let (large_elapsed, large_result) = measure_calls(engine, &large)?;
    assert_eq!(small_result, large_result);
    println!(
        "shape=call-heavy calls=2000 small_executables={} large_executables={} small_ns={} large_ns={} ratio={:.3} result={small_result:?}",
        small.linked_program().function_count(),
        large.linked_program().function_count(),
        small_elapsed.as_nanos(),
        large_elapsed.as_nanos(),
        large_elapsed.as_secs_f64() / small_elapsed.as_secs_f64(),
    );
    black_box((small, large));
    Ok(())
}

fn measure_calls(
    engine: &Engine,
    version: &ProgramVersion,
) -> Result<(std::time::Duration, vela_vm::owned_value::OwnedValue), Box<dyn Error>> {
    let mut runtime = Runtime::from_hot_reload_version(engine.clone(), version.clone());
    let warmup = runtime.call("main", CallArgs::new(), CallOptions::unbounded())?;
    black_box(runtime.value_to_owned(&warmup)?);
    let started = Instant::now();
    let result = runtime.call("main", CallArgs::new(), CallOptions::unbounded())?;
    let result = runtime.value_to_owned(&result)?;
    Ok((started.elapsed(), result))
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
