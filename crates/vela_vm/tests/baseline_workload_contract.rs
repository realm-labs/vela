#[allow(dead_code)]
#[path = "../benches/baseline/workload_sources.rs"]
mod workload_sources;

#[allow(dead_code)]
#[path = "../benches/baseline/workloads.rs"]
mod workloads;

use std::collections::{BTreeMap, BTreeSet};

#[test]
fn baseline_workload_names_are_unique() {
    let mut names = BTreeSet::new();

    for workload in workloads::workloads() {
        assert!(
            names.insert(workload.name),
            "duplicate baseline workload name: {}",
            workload.name
        );
    }
}

#[test]
fn cache_hot_offset_workloads_have_delta_baselines() {
    let workloads = workloads::workloads().collect::<Vec<_>>();
    let by_name = workloads
        .iter()
        .map(|workload| (workload.name, *workload))
        .collect::<BTreeMap<_, _>>();

    for workload in workloads {
        let Some(base) = workload.name.strip_suffix("_cache_hot_offsets") else {
            continue;
        };
        assert!(
            workload.mode.is_cache_enabled(),
            "{} must use a cache-enabled mode",
            workload.name
        );

        let hot_offsets = format!("{base}_hot_offsets");
        let base_workload = by_name
            .get(hot_offsets.as_str())
            .or_else(|| by_name.get(base))
            .unwrap_or_else(|| panic!("{} must have a cache_delta baseline", workload.name));
        assert!(
            !base_workload.mode.is_cache_enabled(),
            "{} must pair with a non-cache baseline",
            workload.name
        );
    }
}
