//! Editor-latency harness for the incremental language-service model.
//!
//! The harness replays the sequence a real editor produces on every keystroke
//! against a large synthetic workspace: apply the buffer change, reassemble
//! project sources, update the databases, publish document diagnostics, then
//! answer a completion and a hover request at the caret. It also times the
//! database copy that every background request pays through
//! `GlobalStateSnapshot`.
//!
//! Rows report P50/P95 so a regression in tail latency is visible even when the
//! mean stays flat.
//!
//! ```bash
//! cargo bench -p vela_language_service --bench incremental_latency
//! VELA_LS_BENCH_MODULES=128 cargo bench -p vela_language_service --bench incremental_latency
//! ```
//!
//! Environment overrides: `VELA_LS_BENCH_MODULES`, `VELA_LS_BENCH_FUNCTIONS`,
//! `VELA_LS_BENCH_ITERATIONS`, `VELA_LS_BENCH_WARMUP`.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use vela_language_service::{
    DocumentId, LanguageServiceDatabases, Position, SourceFileSnapshot, SourceVersion, Workspace,
    WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

const ROOT: &str = "/workspace/scripts";
const SUPPORT_DOCUMENT: &str = "/workspace/scripts/support.vela";
const SUPPORT_TEXT: &str =
    "pub const BASE: i64 = 7;\n\npub fn helper(input: i64) -> i64 {\n    return input + BASE;\n}\n";

/// The module the simulated editor keeps open and edits.
const EDITED_MODULE: usize = 17;
/// Line holding `let scaled = base * 3;` in the first generated function body.
const CARET_LINE: usize = 4;
/// Column of the `base` reference on [`CARET_LINE`].
const CARET_COLUMN: usize = 17;

fn main() {
    let modules = env_usize("VELA_LS_BENCH_MODULES", 128);
    let functions = env_usize("VELA_LS_BENCH_FUNCTIONS", 6);
    let iterations = env_usize("VELA_LS_BENCH_ITERATIONS", 8);
    let warmup = env_usize("VELA_LS_BENCH_WARMUP", 2);

    let config = WorkspaceConfig::workspace([WorkspaceRoot::from(ROOT)]);
    let mut files = vec![SourceFileSnapshot::new(SUPPORT_DOCUMENT, SUPPORT_TEXT)];
    files.extend((0..modules).map(|index| {
        SourceFileSnapshot::new(module_document(index), module_text(index, 0, functions))
    }));

    let edited = DocumentId::from(module_document(EDITED_MODULE));
    let open_documents = BTreeSet::from([edited.clone()]);
    let mut workspace = Workspace::new();
    workspace.open_document(
        edited.clone(),
        module_text(EDITED_MODULE, 0, functions),
        SourceVersion::INITIAL,
    );

    let mut databases = LanguageServiceDatabases::new();
    let started = Instant::now();
    let project = assemble_project_sources(&config, &files, &workspace.snapshot());
    databases.update_with_open_documents(&project, &open_documents);
    let cold_index = started.elapsed();

    let mut assemble = Row::new("did_change/assemble_sources");
    let mut update = Row::new("did_change/database_update");
    let mut diagnostics = Row::new("did_change/publish_diagnostics");
    let mut cycle = Row::new("did_change/total");
    let mut completion = Row::new("request/completion");
    let mut hover = Row::new("request/hover");
    let mut snapshot = Row::new("request/database_snapshot");

    let caret = Position::new(CARET_LINE, CARET_COLUMN);
    let total_rounds = warmup.saturating_add(iterations);
    for round in 0..total_rounds {
        let recorded = round >= warmup;
        let salt = i64::try_from(round).unwrap_or(0).saturating_add(1);
        let version = SourceVersion::new(u64::try_from(round).unwrap_or(0).saturating_add(2));

        let cycle_started = Instant::now();
        workspace.change_document(
            edited.clone(),
            module_text(EDITED_MODULE, salt, functions),
            version,
        );

        let step = Instant::now();
        let project = assemble_project_sources(&config, &files, &workspace.snapshot());
        assemble.record(recorded, step.elapsed());

        let step = Instant::now();
        databases.update_with_open_documents(&project, &open_documents);
        update.record(recorded, step.elapsed());

        let step = Instant::now();
        let published = databases.diagnostics_for_document(&edited);
        diagnostics.record(recorded, step.elapsed());
        cycle.record(recorded, cycle_started.elapsed());
        std::hint::black_box(published);

        let step = Instant::now();
        let items = databases.completion_items(&edited, caret);
        completion.record(recorded, step.elapsed());
        std::hint::black_box(items);

        let step = Instant::now();
        let hovered = databases.hover(&edited, caret);
        hover.record(recorded, step.elapsed());
        std::hint::black_box(hovered);

        let step = Instant::now();
        let copy = databases.clone();
        snapshot.record(recorded, step.elapsed());
        std::hint::black_box(copy);
    }

    println!("vela language-service incremental latency");
    println!(
        "workspace: {modules} modules x {functions} functions, {} sources, {} lines",
        files.len(),
        files
            .iter()
            .map(|file| file.text().lines().count())
            .sum::<usize>()
    );
    println!(
        "rounds: {iterations} recorded after {warmup} warmup; cold index {:.1} ms",
        millis(cold_index)
    );
    println!(
        "fixture check: {} diagnostics on the edited document",
        databases
            .diagnostics_for_document(&edited)
            .diagnostics()
            .len()
    );
    println!();
    println!(
        "{:<32} {:>5} {:>12} {:>12} {:>12}",
        "row", "n", "p50 (ms)", "p95 (ms)", "mean (ms)"
    );
    for row in [
        &mut assemble,
        &mut update,
        &mut diagnostics,
        &mut cycle,
        &mut completion,
        &mut hover,
        &mut snapshot,
    ] {
        row.report();
    }
}

struct Row {
    name: &'static str,
    samples: Vec<Duration>,
}

impl Row {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            samples: Vec::new(),
        }
    }

    fn record(&mut self, recorded: bool, elapsed: Duration) {
        if recorded {
            self.samples.push(elapsed);
        }
    }

    fn report(&mut self) {
        self.samples.sort_unstable();
        let count = self.samples.len();
        let mean = if count == 0 {
            Duration::ZERO
        } else {
            self.samples.iter().sum::<Duration>() / u32::try_from(count).unwrap_or(1)
        };
        println!(
            "{:<32} {count:>5} {:>12.3} {:>12.3} {:>12.3}",
            self.name,
            millis(self.percentile(0.50)),
            millis(self.percentile(0.95)),
            millis(mean)
        );
    }

    fn percentile(&self, fraction: f64) -> Duration {
        if self.samples.is_empty() {
            return Duration::ZERO;
        }
        let last = self.samples.len().saturating_sub(1);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss,
            clippy::cast_sign_loss,
            reason = "sample counts are small enough for exact f64 rank arithmetic"
        )]
        let rank = (fraction * last as f64).round() as usize;
        self.samples[rank.min(last)]
    }
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn module_document(index: usize) -> String {
    format!("{ROOT}/mod_{index}.vela")
}

/// Generates a module whose bodies reference an imported helper so the edit
/// exercises cross-module resolution, not just local parsing.
fn module_text(index: usize, salt: i64, functions: usize) -> String {
    let mut text = String::from("use support::helper\n\n");
    for function in 0..functions {
        text.push_str(&format!(
            "pub fn value_{index}_{function}(input: i64) -> i64 {{\n\
             \x20   let base = helper(input) + {salt};\n\
             \x20   let scaled = base * 3;\n\
             \x20   let label = \"mod {index} fn {function}\";\n\
             \x20   let bucket = [base, scaled];\n\
             \x20   if scaled > 100 {{\n\
             \x20       return bucket[1] + base;\n\
             \x20   }}\n\
             \x20   return base;\n\
             }}\n\n"
        ));
    }
    text
}
