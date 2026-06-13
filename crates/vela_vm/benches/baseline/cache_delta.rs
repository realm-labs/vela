use std::collections::BTreeMap;

const DELTA_BAND_TOLERANCE_PPM: u128 = 10_000;
const PARITY_PPM: u128 = 1_000_000;

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
    let mut summary = DeltaSummary::default();
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
        let mean_delta = signed_delta(record.mean_ns, base.mean_ns);
        let mean_ratio = ratio_ppm(record.mean_ns, base.mean_ns);
        let delta_band = delta_band(mean_ratio);
        summary.record(record, base, mean_delta, delta_band);
        println!(
            "cache_delta bench={} mode={} base={} base_mode={} mean_delta_ns={} min_delta_ns={} median_delta_ns={} p95_delta_ns={} mean_ratio_ppm={} delta_band={} checksum_match={} delta_kind={} cache_hits={} profile_hits={} base_profile_hits={} profile_hits_match={}",
            record.name,
            record.mode,
            base.name,
            base.mode,
            mean_delta,
            signed_delta(record.min_ns, base.min_ns),
            signed_delta(record.median_ns, base.median_ns),
            signed_delta(record.p95_ns, base.p95_ns),
            mean_ratio,
            delta_band,
            record.checksum == base.checksum,
            record.measurement_kind,
            record.cache_hits,
            record.profile_hits,
            base.profile_hits,
            record.profile_hits == base.profile_hits
        );
    }
    summary.print();
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

#[derive(Default)]
struct DeltaSummary {
    paired_rows: usize,
    cache_rows: usize,
    profile_only_rows: usize,
    cache_no_activity_rows: usize,
    improved_rows: usize,
    regressed_rows: usize,
    neutral_rows: usize,
    faster_rows: usize,
    slower_rows: usize,
    flat_rows: usize,
    cache_bands: BandCounts,
    profile_only_bands: BandCounts,
    cache_no_activity_bands: BandCounts,
    checksum_mismatches: usize,
    profile_mismatches: usize,
}

impl DeltaSummary {
    fn record(&mut self, record: &Record, base: &Record, mean_delta: i128, delta_band: &str) {
        self.paired_rows += 1;
        match record.measurement_kind {
            "cache" => self.cache_rows += 1,
            "profile_only" => self.profile_only_rows += 1,
            "cache_no_activity" => self.cache_no_activity_rows += 1,
            _ => {}
        }
        match mean_delta.cmp(&0) {
            std::cmp::Ordering::Less => self.improved_rows += 1,
            std::cmp::Ordering::Greater => self.regressed_rows += 1,
            std::cmp::Ordering::Equal => self.neutral_rows += 1,
        }
        match delta_band {
            "faster" => self.faster_rows += 1,
            "slower" => self.slower_rows += 1,
            "flat" => self.flat_rows += 1,
            _ => {}
        }
        match record.measurement_kind {
            "cache" => self.cache_bands.record(delta_band),
            "profile_only" => self.profile_only_bands.record(delta_band),
            "cache_no_activity" => self.cache_no_activity_bands.record(delta_band),
            _ => {}
        }
        if record.checksum != base.checksum {
            self.checksum_mismatches += 1;
        }
        if record.profile_hits != base.profile_hits {
            self.profile_mismatches += 1;
        }
    }

    fn print(&self) {
        println!(
            "cache_delta_summary paired_rows={} cache_rows={} profile_only_rows={} cache_no_activity_rows={} improved_rows={} regressed_rows={} neutral_rows={} faster_rows={} slower_rows={} flat_rows={} cache_faster_rows={} cache_slower_rows={} cache_flat_rows={} profile_only_faster_rows={} profile_only_slower_rows={} profile_only_flat_rows={} cache_no_activity_faster_rows={} cache_no_activity_slower_rows={} cache_no_activity_flat_rows={} checksum_mismatches={} profile_mismatches={}",
            self.paired_rows,
            self.cache_rows,
            self.profile_only_rows,
            self.cache_no_activity_rows,
            self.improved_rows,
            self.regressed_rows,
            self.neutral_rows,
            self.faster_rows,
            self.slower_rows,
            self.flat_rows,
            self.cache_bands.faster,
            self.cache_bands.slower,
            self.cache_bands.flat,
            self.profile_only_bands.faster,
            self.profile_only_bands.slower,
            self.profile_only_bands.flat,
            self.cache_no_activity_bands.faster,
            self.cache_no_activity_bands.slower,
            self.cache_no_activity_bands.flat,
            self.checksum_mismatches,
            self.profile_mismatches,
        );
    }
}

#[derive(Default)]
struct BandCounts {
    faster: usize,
    slower: usize,
    flat: usize,
}

impl BandCounts {
    fn record(&mut self, delta_band: &str) {
        match delta_band {
            "faster" => self.faster += 1,
            "slower" => self.slower += 1,
            "flat" => self.flat += 1,
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

fn delta_band(mean_ratio_ppm: u128) -> &'static str {
    if mean_ratio_ppm + DELTA_BAND_TOLERANCE_PPM < PARITY_PPM {
        "faster"
    } else if mean_ratio_ppm > PARITY_PPM + DELTA_BAND_TOLERANCE_PPM {
        "slower"
    } else {
        "flat"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn measurement_kind_separates_interpreter_profile_cache_and_idle_cache_rows() {
        assert_eq!(super::measurement_kind(false, 0, 0), "interpreter");
        assert_eq!(super::measurement_kind(false, 0, 12), "profile_only");
        assert_eq!(super::measurement_kind(true, 0, 12), "profile_only");
        assert_eq!(super::measurement_kind(true, 0, 0), "cache_no_activity");
        assert_eq!(super::measurement_kind(true, 3, 0), "cache");
        assert_eq!(super::measurement_kind(true, 3, 12), "cache");
    }

    #[test]
    fn cache_delta_prefers_explicit_hot_offset_base_when_present() {
        let by_name = std::collections::BTreeMap::from([
            ("record_fields", 0),
            ("record_fields_hot_offsets", 1),
            ("record_fields_cache_hot_offsets", 2),
        ]);

        assert_eq!(
            super::cache_delta_base_name("record_fields_cache_hot_offsets", &by_name),
            Some("record_fields_hot_offsets".to_owned())
        );
    }

    #[test]
    fn cache_delta_falls_back_to_interpreter_base_without_hot_offset_row() {
        let by_name = std::collections::BTreeMap::from([("native_call_wide_args", 0)]);

        assert_eq!(
            super::cache_delta_base_name("native_call_wide_args_cache_hot_offsets", &by_name),
            Some("native_call_wide_args".to_owned())
        );
    }

    #[test]
    fn delta_band_applies_one_percent_tolerance_around_parity() {
        assert_eq!(super::delta_band(989_999), "faster");
        assert_eq!(super::delta_band(990_000), "flat");
        assert_eq!(super::delta_band(1_010_000), "flat");
        assert_eq!(super::delta_band(1_010_001), "slower");
    }
}
