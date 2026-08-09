use std::collections::BTreeMap;
use std::error::Error;
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use vela_bytecode::PortableProgramArtifact;
use vela_common::SourceId;

#[path = "../src/test_support.rs"]
#[allow(dead_code)]
mod test_compile_support;
#[path = "external_compare/workloads.rs"]
#[allow(dead_code)]
mod workloads;

use test_compile_support::compile_test_program_with_registry;

const WARMUPS: usize = 2;
const SAMPLES: usize = 7;
const RSS_POLL_INTERVAL: Duration = Duration::from_millis(2);
const LEAD_WORKLOADS: [&str; 5] = [
    "scalar_branch_loop",
    "range_iteration",
    "function_calls",
    "recursive_countdown",
    "float_math_loop",
];

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::args().any(|argument| argument == "--child") {
        return compile_child();
    }

    println!(
        "suite=verified_mir_compile samples={SAMPLES} warmups={WARMUPS} rss_poll_interval_ms={}",
        RSS_POLL_INTERVAL.as_millis()
    );
    for _ in 0..WARMUPS {
        let sample = run_child()?;
        if sample.rows.len() != LEAD_WORKLOADS.len() {
            return Err("warmup child omitted a lead workload".into());
        }
    }

    let mut rows: BTreeMap<String, Vec<CompileRow>> = BTreeMap::new();
    let mut peak_rss = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let sample = run_child()?;
        peak_rss.push(sample.peak_rss_bytes);
        for row in sample.rows {
            rows.entry(row.workload.clone()).or_default().push(row);
        }
    }

    for workload in LEAD_WORKLOADS {
        let samples = rows
            .remove(workload)
            .ok_or_else(|| format!("missing compile samples for {workload}"))?;
        if samples.len() != SAMPLES {
            return Err(format!("expected {SAMPLES} compile samples for {workload}").into());
        }
        report_compile(workload, &samples)?;
    }
    if !rows.is_empty() {
        return Err(format!("unexpected compile workloads: {:?}", rows.keys()).into());
    }
    report_peak_rss(&peak_rss);
    Ok(())
}

fn compile_child() -> Result<(), Box<dyn Error>> {
    let registry = vela_stdlib::standard_registry()
        .map_err(|error| format!("standard registry failed: {error}"))?;
    for workload_name in LEAD_WORKLOADS {
        let workload = workloads::all_workloads()
            .find(|workload| workload.name == workload_name)
            .ok_or_else(|| format!("unknown lead workload {workload_name}"))?;
        let started = Instant::now();
        let compiled = compile_test_program_with_registry(
            SourceId::new(1),
            workload.vela,
            registry.compile_view(),
        )
        .map_err(|error| format!("{workload_name} failed to compile: {error:?}"))?;
        let compile_ns = started.elapsed().as_nanos();
        let portable = PortableProgramArtifact::from_compiled(compiled)?;
        let checksum = portable.checksum();
        let artifact_bytes = portable.encode()?.len();
        println!(
            "compile_child workload={workload_name} compile_ns={compile_ns} artifact_bytes={artifact_bytes} artifact_checksum={checksum}"
        );
    }
    Ok(())
}

#[derive(Debug)]
struct ChildSample {
    rows: Vec<CompileRow>,
    peak_rss_bytes: u64,
}

#[derive(Debug)]
struct CompileRow {
    workload: String,
    compile_ns: u128,
    artifact_bytes: usize,
    artifact_checksum: String,
}

fn run_child() -> Result<ChildSample, Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    let mut child = Command::new(executable)
        .arg("--child")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut peak_rss_bytes = 0_u64;
    let status = loop {
        peak_rss_bytes = peak_rss_bytes.max(process_rss_bytes(child.id()).unwrap_or_default());
        if let Some(status) = child.try_wait()? {
            break status;
        }
        thread::sleep(RSS_POLL_INTERVAL);
    };

    let mut stdout = String::new();
    child
        .stdout
        .take()
        .expect("captured child stdout")
        .read_to_string(&mut stdout)?;
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("captured child stderr")
        .read_to_string(&mut stderr)?;
    if !status.success() {
        return Err(format!("compile child failed with {status}: {stderr}").into());
    }

    let rows = stdout
        .lines()
        .filter(|line| line.starts_with("compile_child "))
        .map(parse_compile_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ChildSample {
        rows,
        peak_rss_bytes,
    })
}

fn parse_compile_row(line: &str) -> Result<CompileRow, Box<dyn Error>> {
    let fields = line
        .split_whitespace()
        .skip(1)
        .filter_map(|field| field.split_once('='))
        .collect::<BTreeMap<_, _>>();
    Ok(CompileRow {
        workload: required_field(&fields, "workload")?.to_owned(),
        compile_ns: required_field(&fields, "compile_ns")?.parse()?,
        artifact_bytes: required_field(&fields, "artifact_bytes")?.parse()?,
        artifact_checksum: required_field(&fields, "artifact_checksum")?.to_owned(),
    })
}

fn required_field<'a>(
    fields: &'a BTreeMap<&str, &str>,
    name: &str,
) -> Result<&'a str, Box<dyn Error>> {
    fields
        .get(name)
        .copied()
        .ok_or_else(|| format!("missing {name} in compile child row").into())
}

fn process_rss_bytes(pid: u32) -> Option<u64> {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let kibibytes = String::from_utf8(output.stdout)
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    kibibytes.checked_mul(1024)
}

fn report_compile(workload: &str, samples: &[CompileRow]) -> Result<(), Box<dyn Error>> {
    let artifact_bytes = samples[0].artifact_bytes;
    let artifact_checksum = &samples[0].artifact_checksum;
    if samples.iter().any(|sample| {
        sample.artifact_bytes != artifact_bytes || sample.artifact_checksum != *artifact_checksum
    }) {
        return Err(format!("{workload} portable artifact changed between samples").into());
    }
    let times = samples
        .iter()
        .map(|sample| sample.compile_ns)
        .collect::<Vec<_>>();
    let summary = summarize(&times);
    println!(
        "compile_result workload={workload} min_ns={} mean_ns={} median_ns={} p95_ns={} artifact_bytes={artifact_bytes} artifact_checksum={artifact_checksum}",
        summary.min, summary.mean, summary.median, summary.p95
    );
    Ok(())
}

fn report_peak_rss(samples: &[u64]) {
    let values = samples
        .iter()
        .map(|value| u128::from(*value))
        .collect::<Vec<_>>();
    let summary = summarize(&values);
    println!(
        "compile_memory samples={} min_peak_rss_bytes={} mean_peak_rss_bytes={} median_peak_rss_bytes={} p95_peak_rss_bytes={}",
        samples.len(),
        summary.min,
        summary.mean,
        summary.median,
        summary.p95
    );
}

struct Summary {
    min: u128,
    mean: u128,
    median: u128,
    p95: u128,
}

fn summarize(samples: &[u128]) -> Summary {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let total = sorted.iter().copied().sum::<u128>();
    let p95_index = (sorted.len() * 95).div_ceil(100).saturating_sub(1);
    Summary {
        min: sorted[0],
        mean: total / sorted.len() as u128,
        median: sorted[sorted.len() / 2],
        p95: sorted[p95_index],
    }
}
