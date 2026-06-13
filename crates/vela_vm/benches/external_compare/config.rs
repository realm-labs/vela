pub(crate) const QUICK_REPEATS: usize = 2;
pub(crate) const QUICK_ITERATIONS: usize = 500;
pub(crate) const QUICK_WARMUP: usize = 1;
pub(crate) const DEFAULT_REPEATS: usize = 3;
pub(crate) const DEFAULT_ITERATIONS: usize = 5_000;
pub(crate) const DEFAULT_WARMUP: usize = 1;

#[derive(Clone, Debug)]
pub(crate) struct BenchConfig {
    pub(crate) params: BenchParams,
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
    pub(crate) fn from_args() -> Self {
        Self::from_iter(std::env::args().skip(1))
    }

    pub(crate) fn from_iter(args: impl IntoIterator<Item = String>) -> Self {
        let mut quick = false;
        let mut filters = Vec::new();
        let mut repeats = None;
        let mut iterations = None;
        let mut warmup = None;
        let mut runtimes = Vec::new();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--quick" => quick = true,
                "--repeats" => repeats = args.next().and_then(|value| value.parse().ok()),
                "--iterations" => iterations = args.next().and_then(|value| value.parse().ok()),
                "--warmup" => warmup = args.next().and_then(|value| value.parse().ok()),
                "--runtime" => {
                    if let Some(value) = args.next() {
                        push_runtime_filters(&mut runtimes, &value);
                    }
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
                    push_runtime_filters(&mut runtimes, &arg["--runtime=".len()..]);
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
        Self {
            params,
            filters,
            runtimes,
        }
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

    pub(crate) fn should_run_runtime(&self, runtime: &str) -> bool {
        self.runtimes.is_empty() || self.runtimes.iter().any(|filter| runtime.contains(filter))
    }
}

fn push_runtime_filters(runtimes: &mut Vec<String>, value: &str) {
    runtimes.extend(
        value
            .split(',')
            .map(str::trim)
            .filter(|runtime| !runtime.is_empty())
            .map(str::to_owned),
    );
}
