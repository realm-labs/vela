pub(crate) const QUICK_REPEATS: usize = 2;
pub(crate) const QUICK_ITERATIONS: usize = 500;
pub(crate) const QUICK_WARMUP: usize = 1;
pub(crate) const DEFAULT_REPEATS: usize = 3;
pub(crate) const DEFAULT_ITERATIONS: usize = 5_000;
pub(crate) const DEFAULT_WARMUP: usize = 1;

pub(crate) const SUPPORTED_RUNTIMES: [&str; 6] =
    ["vela", "vela-cache", "lua54", "rhai", "node", "python3"];

#[derive(Clone, Debug)]
pub(crate) struct BenchConfig {
    pub(crate) params: BenchParams,
    pub(crate) preflight_only: bool,
    filters: Vec<String>,
    runtimes: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BenchParams {
    pub(crate) repeats: usize,
    pub(crate) iterations: usize,
    pub(crate) warmup: usize,
}

impl BenchConfig {
    pub(crate) fn from_iter(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut quick = false;
        let mut preflight_only = false;
        let mut filters = Vec::new();
        let mut repeats = None;
        let mut iterations = None;
        let mut warmup = None;
        let mut runtimes = Vec::new();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--quick" => quick = true,
                "--preflight-only" => preflight_only = true,
                "--repeats" => repeats = args.next().and_then(|value| value.parse().ok()),
                "--iterations" => iterations = args.next().and_then(|value| value.parse().ok()),
                "--warmup" => warmup = args.next().and_then(|value| value.parse().ok()),
                "--runtime" => {
                    let value = args
                        .next()
                        .ok_or("--runtime requires a nonempty runtime selection")?;
                    push_runtime_filters(&mut runtimes, &value)?;
                }
                _ if arg.starts_with("--repeats=") => {
                    repeats = arg["--repeats=".len()..].parse().ok();
                }
                _ if arg.starts_with("--iterations=") => {
                    iterations = arg["--iterations=".len()..].parse().ok();
                }
                _ if arg.starts_with("--warmup=") => {
                    warmup = arg["--warmup=".len()..].parse().ok();
                }
                _ if arg.starts_with("--runtime=") => {
                    push_runtime_filters(&mut runtimes, &arg["--runtime=".len()..])?;
                }
                _ if !arg.starts_with('-') => filters.push(arg),
                _ => {}
            }
        }
        let mut params = if quick {
            BenchParams {
                repeats: QUICK_REPEATS,
                iterations: QUICK_ITERATIONS,
                warmup: QUICK_WARMUP,
            }
        } else {
            BenchParams {
                repeats: DEFAULT_REPEATS,
                iterations: DEFAULT_ITERATIONS,
                warmup: DEFAULT_WARMUP,
            }
        };
        if let Some(repeats) = repeats {
            params.repeats = repeats;
        }
        if let Some(iterations) = iterations {
            params.iterations = iterations;
        }
        if let Some(warmup) = warmup {
            params.warmup = warmup;
        }
        Ok(Self {
            params,
            preflight_only,
            filters,
            runtimes,
        })
    }

    pub(crate) fn filters_label(&self) -> String {
        if self.filters.is_empty() {
            "all".to_owned()
        } else {
            self.filters.join(",")
        }
    }

    pub(crate) fn runtimes_label(&self) -> String {
        if self.runtimes.is_empty() {
            "all".to_owned()
        } else {
            self.runtimes.join(",")
        }
    }

    pub(crate) fn should_run(&self, workload_name: &str) -> bool {
        self.filters.is_empty()
            || self
                .filters
                .iter()
                .any(|filter| workload_name.contains(filter))
    }

    pub(crate) fn runtime_required(&self, runtime: &str) -> bool {
        self.runtimes.iter().any(|filter| runtime == filter)
    }

    pub(crate) fn should_run_runtime(&self, runtime: &str) -> bool {
        self.runtimes.is_empty() || self.runtimes.iter().any(|filter| runtime == filter)
    }
}

fn push_runtime_filters(runtimes: &mut Vec<String>, value: &str) -> Result<(), String> {
    for runtime in value.split(',').map(str::trim) {
        if runtime.is_empty() {
            return Err("--runtime requires a nonempty runtime selection".to_owned());
        }
        if !SUPPORTED_RUNTIMES.contains(&runtime) {
            return Err(format!(
                "unknown runtime selector `{runtime}`; supported runtimes: {}",
                SUPPORTED_RUNTIMES.join(",")
            ));
        }
        runtimes.push(runtime.to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_unknown_and_empty_explicit_runtime_selections() {
        use super::BenchConfig;
        for args in [
            vec!["--runtime=not-a-runtime"],
            vec!["--runtime=vela,not-a-runtime"],
            vec!["--runtime", "not-a-runtime"],
            vec!["--runtime", "vela,not-a-runtime"],
            vec!["--runtime=vela", "--runtime=not-a-runtime"],
            vec!["--runtime="],
            vec!["--runtime", ""],
            vec!["--runtime", "   "],
            vec!["--runtime", ","],
            vec!["--runtime"],
            vec!["--runtime=vela", "--runtime="],
            vec!["--runtime=vela,"],
            vec!["--runtime=,vela"],
            vec!["--runtime=vela,,node"],
        ] {
            assert!(
                BenchConfig::from_iter(args.iter().map(|arg| (*arg).to_owned())).is_err(),
                "{args:?}"
            );
        }
    }

    #[test]
    fn accepts_all_six_ids_and_preserves_default_optional_selection() {
        use super::{BenchConfig, SUPPORTED_RUNTIMES};

        let default = BenchConfig::from_iter(std::iter::empty()).expect("default selection");
        for id in SUPPORTED_RUNTIMES {
            assert!(default.should_run_runtime(id));
            assert!(!default.runtime_required(id));
            let selected =
                BenchConfig::from_iter([format!("--runtime={id}")]).expect("supported ID");
            for candidate in SUPPORTED_RUNTIMES {
                assert_eq!(selected.should_run_runtime(candidate), candidate == id);
                assert_eq!(selected.runtime_required(candidate), candidate == id);
            }
        }
        let selected = BenchConfig::from_iter([
            "--runtime= vela , vela-cache , lua54".to_owned(),
            "--runtime".to_owned(),
            "rhai,node,python3".to_owned(),
        ])
        .expect("all supported IDs across repeated selectors");
        for id in SUPPORTED_RUNTIMES {
            assert!(selected.should_run_runtime(id));
            assert!(selected.runtime_required(id));
        }
    }
}
