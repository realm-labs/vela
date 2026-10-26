use std::error::Error;

#[path = "external_compare/bounded_process.rs"]
mod bounded_process;
/// Shared with the `baseline` bench: single-threaded inline-cache storage for
/// cache-enabled measurement rows. `#[path]` is the repository's documented
/// mechanism for justified cross-bench-target sharing.
#[path = "baseline/cache_support.rs"]
#[allow(dead_code)]
mod cache_support;
#[path = "external_compare/config.rs"]
mod config;
#[path = "external_compare/embedded.rs"]
mod embedded;
#[path = "external_compare/preflight.rs"]
mod preflight;
#[path = "external_compare/process.rs"]
mod process;
#[path = "external_compare/report.rs"]
mod report;
#[path = "external_compare/support.rs"]
mod support;
#[path = "../src/test_support.rs"]
#[allow(dead_code)]
mod test_compile_support;
#[path = "external_compare/vela.rs"]
mod vela;
#[path = "external_compare/version.rs"]
mod version;
#[path = "external_compare/workloads.rs"]
mod workloads;

#[path = "external_compare/runner.rs"]
mod runner;

fn main() -> Result<(), Box<dyn Error>> {
    runner::run(std::env::args().skip(1))
}
