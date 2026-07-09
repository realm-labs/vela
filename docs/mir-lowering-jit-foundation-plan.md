# MIR Lowering And JIT Foundation Plan

> **Track:** middle IR, bytecode lowering architecture, optimizer/JIT
> foundation after Heavy HIR
> **Document status:** Codex goal-mode execution plan
> **Compatibility policy:** breaking pre-release MIR, bytecode-compiler, and
> internal test APIs are allowed. Preserve Vela language semantics, VM behavior,
> diagnostics, execution budgets, GC roots, HostAccess safety, reflection
> permissioning, hot-reload ABI/schema checks, and current bytecode verifier
> guarantees.

Hard-switch policy: this plan is a full execution plan, but implementation must
not begin until the Heavy HIR hard switch is complete enough for MIR to consume
body-level semantic facts without reading syntax. MIR exists to model execution
shape, not to repair semantic gaps.

Use deletion-first subsystem slices. The old direct bytecode lowering path is
allowed to be broken during a checkpoint and may be used only as a compile-error
migration queue inside that checkpoint. Do not keep direct lowering as a
compatibility backend across completed checkpoints; the checkpoint that switches
an equivalent MIR backend path green must also delete or rewrite the old direct
path and its dual-path tests.

MIR v1 must lower to the existing bytecode and VM. It must not introduce
Cranelift, machine code, deoptimization runtime machinery, or user-visible
language behavior changes. Cranelift remains M22 work.

---

## 0. Codex Goal

Use this prompt to execute the full refactor after Heavy HIR acceptance:

```text
/goal Execute the MIR lowering and JIT foundation plan from
docs/mir-lowering-jit-foundation-plan.md. Treat docs/goal.md as the product
roadmap, docs/architecture.md and docs/architecture/*.md as the architecture
contract, docs/progress.md as current milestone status, and
docs/heavy-hir-hard-switch-plan.md as the required semantic input contract.
Add an internal vela_mir crate that consumes Heavy HIR plus analysis facts,
builds verified MIR with explicit CFG, operands, places, temporaries, typed ops,
guards, liveness/debug metadata, and lowers MIR to the existing bytecode
backend. Do not read syntax from MIR, do not add JIT, and do not change VM
semantics. Use deletion-first subsystem slices: delete or rewrite the matching
direct lowering path in the same checkpoint that makes the MIR backend path
green, and do not preserve dual bytecode backends as a compatibility layer.
Validate every slice with MIR verifier tests, bytecode equivalence tests, VM
tests, and workspace checks.
```

---

## 1. Purpose

The current bytecode compiler lowers source semantics directly into bytecode
while also selecting typed operations, frame slots, guards, control-flow shape,
and cache-ready instruction forms. That makes local optimizations possible but
keeps CFG/data-flow, temporary lifetime, and future JIT decisions coupled to
bytecode emission.

MIR introduces one explicit execution-shape layer:

```text
Heavy HIR + Analysis Facts
  -> MIR: CFG, typed ops, places, guards, liveness, debug maps
  -> existing bytecode backend
  -> future Cranelift backend
```

MIR v1 is successful when it can reproduce current bytecode semantics through a
verified MIR -> bytecode path while preserving source diagnostics and runtime
contracts.

---

## 2. Dependency On Heavy HIR

MIR consumes only:

- Heavy HIR bodies, IDs, source origins, scopes, bindings, captures, and
  semantic targets;
- analysis facts keyed by Heavy HIR IDs;
- bytecode/runtime metadata needed to construct existing instructions and
  verified guard plans.

MIR must not:

- parse source text;
- inspect body-level syntax wrappers;
- infer name binding, call targets, member targets, or type facts from syntax;
- duplicate language-service-only query logic;
- emit diagnostics that should belong to Heavy HIR or analysis.

If a MIR phase needs semantic information that is not available from Heavy HIR,
stop and extend Heavy HIR first.

---

## 3. Target MIR Model

MIR v1 should live in a new internal `vela_mir` crate and expose focused APIs
to bytecode/compiler crates without becoming a stable public embedding API.

Core model:

- `MirProgram` and `MirFunction` for lowered functions, methods, lambdas,
  defaults, and initializer bodies.
- `MirBlockId`, `MirLocalId`, `MirTempId`, `MirPlace`, `MirOperand`,
  `MirRvalue`, `MirStatement`, and `MirTerminator`.
- Explicit basic blocks with terminators for return, jump, branch, switch,
  call, fail, break/continue lowering targets, try propagation, and unreachable.
- Places for locals, temporaries, fields, indexes, tuple fields, host paths, and
  dereference-like runtime handles where applicable.
- Rvalues for literals, copies/moves, unary/binary ops, typed i64 ops,
  aggregates, records, arrays, maps, tuples, enum variants, calls, method calls,
  host operations, stdlib/native calls, iterator steps, and guard checks.
- Debug/source maps from MIR statements and terminators back to Heavy HIR IDs
  and source spans.
- Liveness and root metadata sufficient for bytecode frame/debug metadata and
  future compiled-frame GC/debugger support.

---

## 4. Phase Status

Use this checklist as the durable execution tracker. Mark a task only after its
focused tests and validation command pass.

```text
[ ] not started
[~] in progress
[x] complete
```

---

## 5. Phase 0: Preconditions

Purpose: prevent MIR from becoming a second semantic pipeline.

- [ ] Heavy HIR acceptance is complete or explicitly marked complete enough for
  MIR input.
- [ ] Bytecode compiler can obtain body, type, effect, call, member, and
  control-flow facts from Heavy HIR/analysis without body-level syntax
  reconstruction.
- [ ] Remaining semantic gaps are documented before MIR implementation begins.
- [ ] `docs/progress.md` records MIR as planned or in progress, not complete.

Validation:

```bash
cargo test -p vela_hir
cargo test -p vela_analysis
cargo test -p vela_bytecode
```

---

## 6. Phase 1: Add `vela_mir` Crate And Core Types

Purpose: establish the MIR ownership boundary without changing bytecode output.

- [ ] Add internal crate `crates/vela_mir`.
- [ ] Add MIR ID newtypes, function/program containers, block list, local/temp
  tables, source map, and diagnostics/error types.
- [ ] Add operands, places, rvalues, statements, terminators, guards, and typed
  operation enums.
- [ ] Add display/debug helpers for stable test snapshots.
- [ ] Keep crate APIs internal to the workspace; do not expose MIR through
  engine/runtime public APIs.

Validation:

```bash
cargo test -p vela_mir
cargo check -p vela_bytecode
```

---

## 7. Phase 2: MIR Builder From Heavy HIR

Purpose: lower representative Heavy HIR bodies into MIR.

- [ ] Build MIR for literals, locals, declarations, assignments, blocks,
  returns, if/else, loops, break/continue, match, try propagation, and lambdas.
- [ ] Build MIR for calls, methods, stdlib/native calls, host paths, field
  access, index access, tuple field access, record/array/map/tuple/enum
  construction, and pattern destructuring.
- [ ] Preserve source-origin mapping and diagnostic spans.
- [ ] Represent dynamic boundaries explicitly instead of lowering them as
  unknown syntax fallbacks.

Validation:

```bash
cargo test -p vela_mir builder
cargo test -p vela_bytecode compiler
```

---

## 8. Phase 3: MIR Verifier

Purpose: make MIR safety and structural invariants explicit before bytecode
emission.

- [ ] Verify block and terminator reachability.
- [ ] Verify local/temp definitions, uses, moves/copies, and liveness.
- [ ] Verify type/guard consistency where Heavy HIR facts prove a value.
- [ ] Verify call targets, arity, argument locations, return locations, and try
  propagation families.
- [ ] Verify source/debug metadata coverage for emitted operations.

Validation:

```bash
cargo test -p vela_mir verifier
```

---

## 9. Phase 4: MIR To Existing Bytecode Backend

Purpose: route execution through MIR while preserving current bytecode and VM
behavior.

- [ ] Add a MIR -> unlinked bytecode backend that emits the existing instruction
  set.
- [ ] Preserve existing verifier/linker behavior and source-spanned errors.
- [ ] Preserve frame slots, debug names, local metadata, guard plans, cache-site
  descriptions, and hot-reload metadata.
- [ ] Add equivalence tests comparing current compiler output semantics and MIR
  backend execution for representative constructs.
- [ ] Delete or rewrite each old direct lowering path in the same completed
  checkpoint that routes the equivalent construct through MIR. Do not keep the
  old path as a cross-checkpoint compatibility backend.

Validation:

```bash
cargo test -p vela_bytecode mir
cargo test -p vela_vm
cargo test -p vela_engine
```

---

## 10. Phase 5: Typed Ops, Guards, And Liveness

Purpose: move optimization-shaped decisions into MIR without changing language
semantics.

- [ ] Represent proven i64 arithmetic, comparison, branch, and range-loop ops in
  MIR.
- [ ] Represent generic dynamic operation boundaries explicitly.
- [ ] Represent guard/deopt-style metadata needed by inline caches and future
  JIT side exits, without adding JIT runtime behavior.
- [ ] Compute liveness and root/debug metadata for locals, temporaries, heap
  values, closures, iterators, host refs, and captured values.
- [ ] Preserve existing budget, GC, HostAccess, reflection, and hot-reload
  invariants.

Validation:

```bash
cargo test -p vela_mir typed guard liveness
cargo test -p vela_bytecode typed scalar range
cargo test -p vela_vm typed scalar range
```

---

## 11. Phase 6: Compiler Hard Switch To MIR

Purpose: make MIR the only bytecode lowering input.

- [ ] Route all compile paths through Heavy HIR -> MIR -> bytecode.
- [ ] Delete the direct Heavy HIR/syntax -> bytecode lowering path.
- [ ] Delete migration-only backend helpers, direct-lowering flags, and tests
  that prove both lowering paths stay alive.
- [ ] Update docs/progress.md only after focused and workspace validation pass.

Audit searches:

```bash
rg -n "direct.*lower|legacy.*lower|syntax.*lower.*bytecode|Compiler.*Payload|body_payload|mir.*fallback" crates/vela_bytecode crates/vela_mir
rg -n "parse_source_with_id\\(|Syntax.*Expr|Syntax.*Stmt" crates/vela_mir crates/vela_bytecode/src/compiler
```

Validation:

```bash
cargo test -p vela_mir
cargo test -p vela_bytecode
cargo test -p vela_vm
cargo test -p vela_engine
```

---

## 12. Phase 7: Acceptance And JIT Readiness

Purpose: finish MIR as a stable internal foundation for M22 without implementing
Cranelift.

- [ ] MIR verifier covers all emitted MIR forms.
- [ ] MIR -> bytecode backend covers all current executable language behavior.
- [ ] Bytecode equivalence/runtime behavior fixtures pass for parser, HIR,
  compiler, VM, host, reflection, hot reload, stdlib, iterator, tuple, and
  Option/Result surfaces.
- [ ] Source maps and debug metadata are sufficient for M21 debugger and M22
  compiled-frame work.
- [ ] JIT-specific implementation remains deferred to M22.

Final validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --manifest-path examples/Cargo.toml --test runnable_examples
```

The plan is complete only when MIR is the single internal bytecode-lowering
input, the old direct lowering path is gone, current behavior is preserved, and
the MIR metadata boundary is ready for debugger and Cranelift planning.
