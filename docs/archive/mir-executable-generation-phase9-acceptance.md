# Executable-Generation Phase 9 Acceptance

This is the durable evidence index for the final architecture gate. Raw
benchmark logs remain local; durable measurements and the accepted regression
decision are in `docs/performance.md` and `docs/decisions.md`.

## Behavior Evidence

| Requirement | Authoritative coverage |
|---|---|
| CFG facts and typed operations | MIR verifier CFG/liveness/contracts suites, bytecode CFG-join source regressions, malformed-MIR negative tests, and the Phase 9 dynamic-call compound regression. |
| Callable direction and arity | Analysis callable-contract tests, MIR call-guard tests, bytecode forwarding tests, and VM container-family forwarding tests. |
| Cache/profile layouts and isolation | Linker/artifact cache-ID tests; Engine hit, miss, wrong-guard, fallback, reload, schema-epoch, profile-reset, and multi-runtime suites; complete quick baseline with 76 paired rows and no checksum/profile mismatch. |
| Reload generations | Hot-reload old-version/new-entry tests, retained closure handle-layout reload tests, rejected-update tests, weak sidecar pruning, and retained-generation memory harness. |
| Budgets | Exact VM loop/call/guard/try/allocation/callback/HostAccess/reflection edges, committed-write ordering, retained-closure budget test, backend conformance helper, and paired bounded/unbounded benchmark. |
| GC and debug | Unique-safepoint verifier negatives, value/root/debug liveness suites, capture/default roots, old-generation ownership tests, and full VM heap/root suite. |
| Diagnostics | Structured MIR/backend category tests, register overflow, unsupported record patterns, CLI fixtures, Engine source reports, VM call-stack fixtures, and runnable negative examples. |
| Host/reflection | HostAccess write-through/permissions/stale-generation suites, reflection permission/value tests, macro reference rejection, and zero Rust-host-state ownership changes. |

## Final Commands

All completed on macOS/aarch64 with Rust 1.96.0:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --manifest-path examples/Cargo.toml --test runnable_examples
cargo bench --workspace --no-run
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench external_compare -- --quick
cargo bench -p vela_engine --bench hot_reload -- --quick
```

The complete quick baseline produced 76 cache/profile pairs, 63 improved, 13
regressed, with zero checksum and profile mismatches. The exact Phase 0 scalar
command preserved checksum `3828494456532927350`. Generation footprint and
clean compiler RSS measurements are recorded in `docs/performance.md`.

## Architecture Audits

The plan's six zero-hit searches passed. Additional audits found no physical
register/slot/cache types in `vela_mir`, no HIR/syntax/analysis dependency in
the physical backend, no mutable cache/profile state in shared artifacts, and
no ownerless old-generation dense handle resolution. Cache-bearing opcodes are
covered by the exhaustive linked/unlinked verifier matches and Phase 0
inventory. RuntimeImage delegates linking to the Engine/Linker authority.

Every active file above 1200 lines is reviewed in
`docs/architecture/file-size-exceptions.md`; any new unlisted hit is a future
audit failure.
