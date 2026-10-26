//! Independent raw-result oracles, not hashes of another runtime's output.

use std::error::Error;

pub(crate) const ITERATIONS: [usize; 3] = [0, 1, 2];

// Each value is one run_once result. Derivations are recorded in the SF11
// workload evidence; repeated runs start with fresh workload-local state.
pub(crate) const EXPECTED: &[(&str, i64)] = &[
    ("scalar_branch_loop", 11_940),
    ("range_iteration", 67_040),
    ("function_calls", 116_882),
    ("array_scan", 10_578),
    ("map_lookup_update", 15_722),
    ("set_lookup_mutation", 475),
    ("string_methods", 1_391),
    ("closure_callbacks", 45_008),
    ("map_string_index_lookup_update", 15_722),
    ("map_string_key_lookup_update", 15_722),
    ("map_i64_key_lookup_update", 15_722),
    ("map_record_identity_lookup_update", 10_465),
    ("set_i64_lookup_mutation", 475),
    ("set_i64_large_lookup_mutation", 12_858),
    ("set_string_lookup_mutation", 475),
    ("set_record_identity_lookup_mutation", 670),
    ("recursive_countdown", 1_620),
    ("nested_collection_allocation", 11_104),
    ("object_field_methods", 9_788),
    ("string_build_split_join", 2_649),
    ("float_math_loop", 19_605),
    ("array_transform_sort", 10_388),
];

pub(crate) fn validate(
    runtime: &str,
    workload: &str,
    iterations: usize,
    actual: i64,
) -> Result<(), Box<dyn Error>> {
    let once = EXPECTED
        .iter()
        .find_map(|(name, value)| (*name == workload).then_some(*value))
        .ok_or_else(|| format!("missing semantic expectation for {workload}"))?;
    if !ITERATIONS.contains(&iterations) {
        return Err(format!("unsupported preflight iteration count {iterations}").into());
    }
    let expected = once * iterations as i64;
    if actual != expected {
        return Err(format!(
            "{runtime} {workload} semantic preflight: iterations={iterations} expected={expected} actual={actual}"
        )
        .into());
    }
    Ok(())
}
