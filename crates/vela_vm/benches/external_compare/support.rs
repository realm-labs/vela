use std::time::Duration;

use vela_vm::owned_value::OwnedValue;

#[derive(Clone, Copy, Debug)]
pub(crate) struct BenchResult {
    pub(crate) min_ns: u128,
    pub(crate) mean_ns: u128,
    pub(crate) median_ns: u128,
    pub(crate) p95_ns: u128,
    pub(crate) checksum: u64,
}

pub(crate) fn summarize(mut samples: Vec<Duration>, checksum: u64) -> BenchResult {
    samples.sort_unstable();
    let min_ns = samples.first().map_or(0, Duration::as_nanos);
    let median_ns = percentile_ns(&samples, 50);
    let p95_ns = percentile_ns(&samples, 95);
    let mean_ns = if samples.is_empty() {
        0
    } else {
        samples.iter().map(Duration::as_nanos).sum::<u128>() / samples.len() as u128
    };
    BenchResult {
        min_ns,
        mean_ns,
        median_ns,
        p95_ns,
        checksum,
    }
}

fn percentile_ns(samples: &[Duration], percentile: usize) -> u128 {
    if samples.is_empty() {
        return 0;
    }
    let index = ((samples.len() - 1) * percentile).div_ceil(100);
    samples[index].as_nanos()
}

pub(crate) fn value_checksum(value: &OwnedValue) -> u64 {
    match value {
        OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) => *value as u64,
        OwnedValue::Scalar(vela_common::ScalarValue::I32(value)) => *value as u64,
        OwnedValue::Bool(value) => u64::from(*value),
        OwnedValue::Scalar(vela_common::ScalarValue::F64(value)) => value.to_bits(),
        OwnedValue::String(value) => bytes_checksum(value.as_bytes()),
        _ => 0,
    }
}

pub(crate) fn bytes_checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |checksum, byte| {
        (checksum ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

pub(crate) fn mix(lhs: u64, rhs: u64) -> u64 {
    lhs.rotate_left(5) ^ rhs.wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

pub(crate) fn sanitize(value: &str) -> String {
    value
        .replace(['\r', '\n', '"'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}
