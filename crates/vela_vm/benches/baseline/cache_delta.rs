use std::collections::BTreeMap;

pub(crate) struct Record {
    pub(crate) name: &'static str,
    pub(crate) mode: &'static str,
    pub(crate) measurement_kind: &'static str,
    pub(crate) cache_enabled: bool,
    pub(crate) min_ns: u128,
    pub(crate) mean_ns: u128,
    pub(crate) median_ns: u128,
    pub(crate) p95_ns: u128,
    pub(crate) checksum: u64,
    pub(crate) cache_hits: usize,
    pub(crate) profile_hits: u64,
}

pub(crate) fn print(records: &[Record]) {
    print_measurement_summary(records);

    let by_name = records
        .iter()
        .enumerate()
        .map(|(index, record)| (record.name, index))
        .collect::<BTreeMap<_, _>>();
    for record in records {
        if !record.cache_enabled {
            continue;
        }
        let Some(base_name) = cache_delta_base_name(record.name, &by_name) else {
            continue;
        };
        let Some(base) = by_name
            .get(base_name.as_str())
            .and_then(|index| records.get(*index))
        else {
            continue;
        };
        println!(
            "cache_delta bench={} mode={} base={} base_mode={} mean_delta_ns={} min_delta_ns={} median_delta_ns={} p95_delta_ns={} mean_ratio_ppm={} checksum_match={} delta_kind={} cache_hits={} profile_hits={} base_profile_hits={} profile_hits_match={}",
            record.name,
            record.mode,
            base.name,
            base.mode,
            signed_delta(record.mean_ns, base.mean_ns),
            signed_delta(record.min_ns, base.min_ns),
            signed_delta(record.median_ns, base.median_ns),
            signed_delta(record.p95_ns, base.p95_ns),
            ratio_ppm(record.mean_ns, base.mean_ns),
            record.checksum == base.checksum,
            record.measurement_kind,
            record.cache_hits,
            record.profile_hits,
            base.profile_hits,
            record.profile_hits == base.profile_hits
        );
    }
}

fn print_measurement_summary(records: &[Record]) {
    let mut summary = MeasurementSummary::default();
    for record in records {
        summary.record(record);
    }
    println!(
        "measurement_summary interpreter_rows={} profile_only_rows={} cache_rows={} cache_no_activity_rows={} cache_mode_profile_only_rows={} cache_mode_no_activity_rows={}",
        summary.interpreter_rows,
        summary.profile_only_rows,
        summary.cache_rows,
        summary.cache_no_activity_rows,
        summary.cache_mode_profile_only_rows,
        summary.cache_mode_no_activity_rows,
    );
}

#[derive(Default)]
struct MeasurementSummary {
    interpreter_rows: usize,
    profile_only_rows: usize,
    cache_rows: usize,
    cache_no_activity_rows: usize,
    cache_mode_profile_only_rows: usize,
    cache_mode_no_activity_rows: usize,
}

impl MeasurementSummary {
    fn record(&mut self, record: &Record) {
        match record.measurement_kind {
            "interpreter" => self.interpreter_rows += 1,
            "profile_only" => {
                self.profile_only_rows += 1;
                if record.cache_enabled {
                    self.cache_mode_profile_only_rows += 1;
                }
            }
            "cache" => self.cache_rows += 1,
            "cache_no_activity" => {
                self.cache_no_activity_rows += 1;
                if record.cache_enabled {
                    self.cache_mode_no_activity_rows += 1;
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn measurement_kind(
    cache_enabled: bool,
    cache_hits: usize,
    profile_hits: u64,
) -> &'static str {
    if cache_hits > 0 {
        "cache"
    } else if profile_hits > 0 {
        "profile_only"
    } else if cache_enabled {
        "cache_no_activity"
    } else {
        "interpreter"
    }
}

fn cache_delta_base_name(name: &str, by_name: &BTreeMap<&'static str, usize>) -> Option<String> {
    let base = name.strip_suffix("_cache_hot_offsets")?;
    let explicit_hot_offsets = format!("{base}_hot_offsets");
    if by_name.contains_key(explicit_hot_offsets.as_str()) {
        return Some(explicit_hot_offsets);
    }
    Some(base.to_owned())
}

fn signed_delta(value: u128, base: u128) -> i128 {
    value as i128 - base as i128
}

fn ratio_ppm(value: u128, base: u128) -> u128 {
    if base == 0 {
        return 0;
    }
    value.saturating_mul(1_000_000) / base
}
