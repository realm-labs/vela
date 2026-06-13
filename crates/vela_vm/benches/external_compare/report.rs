use super::config::{BenchConfig, BenchParams};
use super::support::{BenchResult, profile, sanitize};

pub(crate) fn print_header(config: &BenchConfig) {
    let params = config.params;
    println!(
        "vela_vm_external_compare profile={} target={}/{} repeats={} iterations={} warmup={} filters={}",
        profile(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        params.repeats,
        params.iterations,
        params.warmup,
        config.filters_label()
    );
}

pub(crate) fn print_result(
    runtime: &str,
    version: &str,
    mode: &str,
    bench: &str,
    result: BenchResult,
    params: BenchParams,
) {
    println!(
        "runtime={} version=\"{}\" bench={} mode={} min_ns={} mean_ns={} median_ns={} p95_ns={} per_iter_mean_ns={} checksum={}",
        runtime,
        sanitize(version),
        bench,
        mode,
        result.min_ns,
        result.mean_ns,
        result.median_ns,
        result.p95_ns,
        result.mean_ns / params.iterations as u128,
        result.checksum
    );
}
