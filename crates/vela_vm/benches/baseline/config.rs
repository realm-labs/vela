const QUICK_REPEATS: usize = 2;
const QUICK_ITERATIONS: usize = 8;
const QUICK_WARMUP: usize = 2;
const DEFAULT_REPEATS: usize = 7;
const DEFAULT_ITERATIONS: usize = 100;
const DEFAULT_WARMUP: usize = 10;

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
        let mut quick = false;
        let mut filters = Vec::new();
        for arg in std::env::args().skip(1) {
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
