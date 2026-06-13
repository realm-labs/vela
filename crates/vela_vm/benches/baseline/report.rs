use super::cache_delta;
use super::config::BenchConfig;
use super::workloads::Workload;
use super::{BenchResult, profile};

pub(crate) fn print_header(config: &BenchConfig) {
    let params = config.params;
    println!(
        "vela_vm_baseline profile={} target={}/{} repeats={} iterations={} warmup={} filters={}",
        profile(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        params.repeats,
        params.iterations,
        params.warmup,
        config.filters_label()
    );
}

pub(crate) fn print_row(workload: &Workload, result: &BenchResult) -> cache_delta::Record {
    let cache = result.cache_stats;
    let measurement_kind = cache_delta::measurement_kind(
        workload.mode.is_cache_enabled(),
        cache.total_hits(),
        result.profile_hits,
    );
    println!(
        "bench={} mode={} measurement_kind={} min_ns={} mean_ns={} median_ns={} p95_ns={} checksum={} cache_sets={} cache_hits={} cache_global_sets={} cache_global_hits={} cache_host_sets={} cache_host_hits={} cache_record_sets={} cache_record_hits={} cache_method_sets={} cache_method_hits={} cache_dynamic_method_sets={} cache_dynamic_method_hits={} cache_native_sets={} cache_native_hits={} profile_hits={}",
        workload.name,
        workload.mode.as_str(),
        measurement_kind,
        result.min_ns,
        result.mean_ns,
        result.median_ns,
        result.p95_ns,
        result.checksum,
        cache.total_sets(),
        cache.total_hits(),
        cache.global_read_sets,
        cache.global_read_hits,
        cache.host_access_sets,
        cache.host_access_hits,
        cache.record_field_sets,
        cache.record_field_hits,
        cache.method_dispatch_sets,
        cache.method_dispatch_hits,
        cache.dynamic_method_dispatch_sets,
        cache.dynamic_method_dispatch_hits,
        cache.native_call_sets,
        cache.native_call_hits,
        result.profile_hits
    );
    cache_delta::Record {
        name: workload.name,
        mode: workload.mode.as_str(),
        measurement_kind,
        cache_enabled: workload.mode.is_cache_enabled(),
        min_ns: result.min_ns,
        mean_ns: result.mean_ns,
        median_ns: result.median_ns,
        p95_ns: result.p95_ns,
        checksum: result.checksum,
        cache_hits: result.cache_stats.total_hits(),
        profile_hits: result.profile_hits,
    }
}
