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
        for arg in args {
            if arg == "--quick" {
                quick = true;
            } else if !arg.starts_with('-') {
                filters.push(arg);
            }
        }
        let params = if quick {
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
        Self { params, filters }
    }

    pub(crate) fn filters_label(&self) -> String {
        if self.filters.is_empty() {
            "all".to_owned()
        } else {
            self.filters.join(",")
        }
    }

    pub(crate) fn should_run(&self, workload_name: &str) -> bool {
        self.filters.is_empty()
            || self
                .filters
                .iter()
                .any(|filter| workload_name.contains(filter))
    }
}
