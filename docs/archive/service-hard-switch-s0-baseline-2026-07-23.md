# Service Hard Switch S0 Baseline

S0 freezes the Rust-default authoring fixture, callable-slot deletion
inventory, and current host-boundary costs before S1 removes callable-level
replacement. Measurements used parent commit `c583ffe56`, rustc 1.97.1,
Cargo 1.97.1, macOS 26.5.2 arm64, the optimized bench profile, 100 warmups,
and 100,000 measured iterations.

Command:

```bash
cargo bench -p vela_engine --bench service_boundary_baseline -- --stable
```

## Stable Rows

| Row | ns/call | calls/s | allocations/call | bytes/call |
|---|---:|---:|---:|---:|
| direct Rust concrete | 4 | 224,887,388 | 0 | 0 |
| direct Rust trait dispatch | 4 | 226,757,370 | 0 | 0 |
| HostRef alias copy | 2 | 399,268,540 | 0 | 0 |
| static field read-write | 13,201 | 75,752 | 284 | 46,729 |
| registered method call | 13,528 | 73,916 | 302 | 49,174 |
| shared argument preflight | 29 | 33,465,343 | 1 | 64 |
| exclusive argument preflight | 29 | 34,172,940 | 1 | 64 |
| nested same-session reborrow | 17,849 | 56,023 | 413 | 77,023 |
| borrowed return and release | 15,750 | 63,490 | 372 | 59,939 |
| host-backed bulk collection | 13,565 | 73,715 | 301 | 48,833 |

Every row emitted a deterministic checksum. Allocation counts and bytes are
balanced by equal deallocation bytes over the measured region.

## Measurement Boundaries

- Direct Rust rows call the same small default operation through concrete and
  trait-object dispatch. They are lower bounds, not service-generation results.
- HostRef alias copy measures the current copyable handle only. S2 replaces its
  metadata ownership with root-local slots while preserving zero allocation,
  zero refcount, and zero lease acquisition for aliases.
- Static field, registered method, nested reborrow, borrowed return/release,
  and bulk collection rows include the complete compiled Vela root call and
  call-scoped host binding. They intentionally expose current end-to-end
  allocation rather than isolating one VM instruction.
- Shared and exclusive preflight rows isolate
  `preflight_host_parameter_leases`. The current returned `Vec` allocates one
  64-byte buffer per two-argument call; S2 must replace this with inline common-
  arity storage without weakening atomic conflict checks.
- Nested reborrow enters Rust through a generated export and re-enters a child
  Vela function through the active `NativeCallContext` with the same mutable
  host origin.
- Borrowed return/release creates a child HostRef from an exclusive owner,
  explicitly releases the borrow group, and then mutates the unfrozen owner.
- The bulk row calls one registered Rust method that traverses a host-owned
  `BTreeMap`. It is the current bulk boundary baseline; S3 must add shared
  MapLike/View protocols and compare prepared protocol operations against it.

## S0 Gate

The domain-neutral `service_hard_switch_fixture` runs entirely through Rust
defaults and covers two services, an async handler, mutable actor state, DTOs,
Array/Map-shaped arguments, Result, and nested calls. The deletion inventory
assigns an owner and replacement to every callable-slot production, macro,
test, example, benchmark, and documentation surface. No feature was added to
the frozen `replaceable` or `override` path.

S0 is accepted. S1 deletes the callable-level replacement model while moving
only neutral lease, re-entry, borrowed-return, ABI, and generation facts to
their proper owners.
