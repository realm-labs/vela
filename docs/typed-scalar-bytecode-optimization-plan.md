# Typed Scalar Bytecode Optimization Plan

> **Track:** M20/post-M19 interpreter performance, scalar hot path  
> **Document status:** architecture + Codex execution plan  
> **Compatibility policy:** breaking internal bytecode/compiler/runtime changes are allowed. Preserve language semantics, hot-reload ABI checks, host-boundary safety, and runtime diagnostics. Do not keep old internal bytecode shapes only for compatibility.  
> **Initial scope:** optimize proven `i64` scalar loops first, using `scalar_branch_loop` as the lead benchmark. Do not implement the full numeric matrix in the first pass.

---

## 0. Executive Summary

The current pure-language benchmark shows that Vela is much slower than Lua,
Node, and Python on `scalar_branch_loop`, while still faster than Rhai:

```text
benchmark: scalar_branch_loop
current Vela: 26981 ns/iteration
Lua 5.4 embedded: 2369 ns/iteration
Node process: 7414 ns/iteration
Python process: 15380 ns/iteration
Rhai embedded: 48019 ns/iteration
```

The benchmark stresses the interpreter's most basic hot path:

```vela
for value in 0..200 {
    if value % 3 == 0 {
        total += value * 2;
        continue;
    }
    if value > 180 {
        break;
    }
    total += (value * 5) % 17;
}
```

The clean long-term fix is not benchmark-specific. Vela should lower
statically proven integer scalar operations into verified typed bytecode so the
VM executes narrow `i64` instructions instead of repeatedly using generic
dynamic `Value` operations.

Target shape:

```text
source/HIR type facts
  -> compiler proves i64 registers and literals
  -> linked bytecode carries typed i64 opcodes
  -> verifier owns safety and operand checks
  -> VM executes thin i64 hot path
```

This keeps Vela dynamic at the language level while allowing static facts to
produce faster bytecode when the program makes those facts obvious.

---

## 1. Goals

### 1.1 Primary goals

- Make `scalar_branch_loop` and adjacent scalar/range workloads significantly
  faster without JIT.
- Introduce a clean typed scalar bytecode tier, starting with `i64`.
- Preserve existing dynamic language semantics for unknown or mixed values.
- Move bytecode structural checks out of the release hot path and into
  verifier/linker contracts.
- Keep the design future-compatible with `f64`, narrow integer types,
  superinstructions, and post-MVP JIT.

### 1.2 Secondary goals

- Make opcode profiles easy to inspect for any benchmark workload.
- Avoid growing `linked_execution.rs` with ad hoc performance conditionals.
- Keep typed fast paths explicit in compiler output and tests.
- Build a clear benchmark checkpoint for future M20/M23 performance decisions.

---

## 2. Non-goals

This pass must not:

- Add JIT, async/coroutines, moving GC, or script-language generics.
- Add implicit numeric conversions.
- Add Rust references or Rust host state into script-owned values.
- Special-case `scalar_branch_loop` by name.
- Implement every primitive numeric type in the first pass.
- Remove dynamic numeric operations; they remain required for unknown values.
- Weaken overflow, division-by-zero, type-contract, or source-span diagnostics.
- Change public language semantics for `i64`, `f64`, arrays, maps, sets, host
  access, reflection, or hot reload.

---

## 3. Design Decisions

### 3.1 Start with `i64` only

The first implementation should support only proven `i64` typed scalar
bytecode.

Reasons:

- Unsuffixed integer literals default to `i64`.
- Range loops such as `0..200` naturally produce `i64`.
- Common business logic counters, levels, XP, indexes, and scores are `i64`.
- The type facts are easy to prove and test.
- It gives the bytecode/verifier/runtime architecture room to settle before
  expanding to `f64`, `u32`, `u8`, or other scalar tags.

Do not create an exhaustive instruction matrix for all numeric types in this
pass. Predeclare reusable type categories if useful, but only implement i64
runtime opcodes initially.

### 3.2 Compiler facts drive specialization

The VM must not speculate that a generic `Value` is an `i64` in the hot path.

Typed scalar bytecode is emitted only when the compiler has enough facts to
prove the operation's operands and result are `i64`.

Allowed examples:

```vela
let total = 0;          // i64
for value in 0..200 {   // value is i64
    total += value * 2; // i64
}
```

Dynamic examples must keep generic bytecode:

```vela
fn add(left, right) {
    return left + right; // generic until facts prove otherwise
}
```

### 3.3 Typed bytecode is a verified linked artifact

Typed opcodes are part of linked bytecode. They should be emitted by the
compiler/linker and accepted only after verifier checks.

The release execution loop should assume linked code has already been
validated. Runtime validation that is purely structural, such as jump target
bounds, should move out of hot execution.

### 3.4 Preserve checked arithmetic semantics

Typed i64 operations are faster, but not unchecked language semantics.

Required behavior:

```text
i64 add/sub/mul overflow -> same runtime overflow error as generic op
i64 rem by zero          -> same runtime division/rem error
i64 comparisons          -> same bool result
source span              -> same user-facing operation span where available
```

Optimization removes dynamic tag dispatch, not correctness checks.

### 3.5 Separate execution hooks from the no-hook hot path

Budget charging and bytecode profiling are important, but they should not add
per-instruction branches to the normal no-budget/no-profiler hot path.

Long-term shape:

```text
hot_no_hooks
hot_with_budget
hot_with_profiler
hot_with_budget_and_profiler
```

This can be implemented as separate loops, focused hook strategy types, or a
small execution mode dispatch before entering the instruction loop. Do not mix
hook policy deeply into each opcode implementation.

### 3.6 Superinstructions come after profiles

Do not start by adding benchmark-shaped fused instructions.

First land typed scalar opcodes and measure. Only then add profile-driven
superinstructions such as:

```text
I64RemImmEqZeroJump
I64GtImmJump
I64MulImmAdd
```

Each superinstruction must have a clear opcode-count or benchmark reason.

---

## 4. Current Repository Anchors

Relevant files and subsystems:

- `crates/vela_vm/benches/external_compare/workloads/core.rs`
  - `scalar_branch_loop`, `range_iteration`, and adjacent pure-language
    comparison workloads.
- `crates/vela_vm/benches/baseline/workloads.rs`
  - Internal baseline rows for scalar/range performance.
- `crates/vela_bytecode/src/lib.rs`
  - Unlinked instruction definitions and bytecode structures.
- `crates/vela_bytecode/src/linked.rs`
  - Linked instruction definitions and linked code objects.
- `crates/vela_bytecode/src/compiler/`
  - Lowering, type/value facts, assignments, loops, operators, and peephole
    opportunities.
- `crates/vela_bytecode/src/verification/`
  - Bytecode verifier contracts for operands, jumps, cache sites, and future
    typed scalar invariants.
- `crates/vela_vm/src/linked_execution.rs`
  - Current linked VM dispatch loop and numeric hot path.
- `crates/vela_vm/src/numeric_conversions.rs`
  - Runtime numeric conversion and error helpers.
- `crates/vela_vm/src/iteration.rs`
  - Range and iterator dispatch.
- `docs/performance.md`
  - Current benchmark rules and tracked workload groups.

---

## 5. Target Bytecode Shape

Initial i64 instruction family:

```text
I64Add        dst, lhs, rhs
I64Sub        dst, lhs, rhs
I64Mul        dst, lhs, rhs
I64Rem        dst, lhs, rhs
I64AddImm     dst, lhs, imm_i64
I64SubImm     dst, lhs, imm_i64
I64MulImm     dst, lhs, imm_i64
I64RemImm     dst, lhs, nonzero_imm_i64
I64EqImm      dst, lhs, imm_i64
I64GtImm      dst, lhs, imm_i64
I64RangeNext  cursor, end, done, inclusive, dst, jump_if_done
```

The first implementation may use a smaller subset if opcode profiling shows a
clear order. For `scalar_branch_loop`, the most valuable subset is likely:

```text
I64RemImm
I64MulImm
I64Add
I64AddImm
I64GtImm
I64EqImm
I64RangeNext
```

Design rules:

- Immediate literals are stored as parsed `i64`, not deferred strings.
- `I64RemImm` with zero immediate is rejected before execution.
- Typed opcodes write `Value::Scalar(ScalarValue::I64(...))` or use any future
  verified typed slot representation.
- Source spans are preserved on instructions that can fail.
- Generic numeric opcodes remain for dynamic or unknown operand types.

---

## 6. Implementation Phases

### Phase 0: Measurement and opcode visibility

Goal: know exactly what the scalar loop executes before changing semantics.

Tasks:

- Add a debug/test helper that prints or returns opcode counts for a compiled
  workload.
- Capture opcode counts for:
  - `scalar_branch_loop`
  - `range_iteration`
  - `function_calls`
  - `float_math_loop`
- Record whether `% literal`, `* literal`, `> literal`, and `for 0..N` are
  already lowered to specialized forms.
- Add a lightweight test or snapshot asserting the current scalar workload can
  be compiled and inspected without running the benchmark harness.

Exit condition:

```text
There is a reproducible opcode-count report for scalar_branch_loop, and the
next optimization target is chosen from actual opcode frequency.
```

### Phase 1: Verified linked hot path cleanup

Goal: remove structural validation from hot execution where the verifier
already proves the invariant.

Tasks:

- Audit runtime calls such as jump target validation in linked execution.
- Ensure the linked verifier rejects invalid jump targets before execution.
- Replace release hot-path structural validation with verifier-owned contracts.
- Keep debug assertions where they help catch internal bugs without changing
  release hot-path cost.

Tests:

- Verifier rejects invalid `Jump`, `JumpIfFalse`, `RangeNext`, and any typed
  branch target operands.
- Valid linked programs execute without per-jump validation in the hot path.

Exit condition:

```text
Linked bytecode is structurally verified before execution, and release hot
jump dispatch does not redo verifier work.
```

### Phase 2: Add i64 typed instructions and verifier contracts

Goal: make typed scalar bytecode a first-class bytecode family.

Tasks:

- Add i64 typed instruction variants to unlinked and linked instruction enums.
- Add display/debug support where existing instruction tools expect it.
- Add linker lowering for the typed variants.
- Add verifier checks:
  - register bounds;
  - immediate validity;
  - `I64RemImm` nonzero immediate;
  - jump target validity for typed range/branch instructions;
  - instruction/source-span consistency where existing verifier patterns cover
    it.
- Add VM execution implementations that use focused helper functions rather
  than expanding `linked_execution.rs` with large inline logic.

Tests:

- Each typed instruction executes correctly.
- Overflow and modulo-by-zero errors match generic operation behavior.
- Verifier rejects malformed typed instructions.
- Source spans are preserved for failing typed arithmetic.

Exit condition:

```text
i64 typed bytecode can be hand-built in tests, verified, linked, and executed
with the same semantics as generic numeric bytecode.
```

### Phase 3: Compiler lowering from type facts

Goal: emit typed opcodes only when facts prove the operation is `i64`.

Tasks:

- Extend or reuse compiler value facts so the compiler knows:
  - unsuffixed integer literals in i64 contexts;
  - range loop cursor facts;
  - locals initialized from i64 literals;
  - locals updated only by proven i64 operations;
  - bool results from i64 comparisons.
- Lower proven `i64 op i64` to typed instructions.
- Lower proven `i64 op literal` to immediate typed instructions.
- Keep generic opcodes for dynamic function parameters and unknown locals.
- Avoid compatibility shims for old internal bytecode shapes; update tests to
  the new canonical lowering.

Tests:

- `scalar_branch_loop` emits typed i64 opcodes for its hot operations.
- Dynamic `fn add(left, right) { left + right }` still emits generic add.
- Mixed known numeric tags remain compile-time errors or generic guarded paths
  according to the primitive type contract.
- Existing conformance tests still pass.

Exit condition:

```text
Compiler output for scalar_branch_loop uses typed i64 operations for the hot
numeric path without changing language behavior.
```

### Phase 4: i64 range loop lowering

Goal: make simple integer range loops thin.

Tasks:

- Add or specialize integer range lowering so `for value in 0..N` can use
  `I64RangeNext` when start/end/inclusive facts are known.
- Preserve inclusive and exclusive range semantics.
- Preserve budget behavior and source spans.
- Keep generic `RangeNext` for dynamic ranges or non-i64 future range types.

Tests:

- Exclusive and inclusive i64 ranges execute correctly.
- Empty ranges, one-element ranges, and boundary cases behave the same as
  generic range execution.
- `range_iteration` and `scalar_branch_loop` compile to `I64RangeNext` where
  proven.

Exit condition:

```text
Simple i64 range loops avoid generic range dispatch in linked bytecode.
```

### Phase 5: Execution hook separation

Goal: remove inactive budget/profiler branches from the normal hot loop.

Tasks:

- Design a small execution-mode boundary before instruction dispatch.
- Keep current budget and profiler behavior exactly when enabled.
- Route no-budget/no-profiler benchmark and normal hot embedding paths through
  a no-hook loop or zero-cost hook strategy.
- Avoid duplicating semantic opcode implementations; share focused operation
  helpers where possible.

Tests:

- Budget traps still fire in budgeted mode.
- Bytecode profiler still records expected offsets in profiled mode.
- No-hook execution returns the same results as hooked execution.

Exit condition:

```text
Inactive budget/profiler features do not add per-instruction branches to the
normal release hot path.
```

### Phase 6: Profile-driven superinstructions

Goal: add only the fused instructions that the new opcode profile proves are
worth it.

Candidate superinstructions:

```text
I64RemImmEqZeroJump
I64GtImmJump
I64MulImmAdd
I64AddAssign
```

Rules:

- Do not add a superinstruction without an opcode-count or benchmark reason.
- Keep verifier coverage for all fused operands.
- Keep source-span behavior clear when fused operations can fail.
- Avoid fusing across semantic boundaries that would complicate diagnostics.

Exit condition:

```text
Any superinstruction has a measured reason, test coverage, and a documented
before/after effect.
```

---

## 7. Benchmark Plan

Use `scalar_branch_loop` as the lead benchmark, but avoid overfitting by
tracking adjacent rows.

Required commands:

```bash
cargo bench -p vela_vm --bench external_compare -- --quick scalar
cargo bench -p vela_vm --bench external_compare -- --quick range
cargo bench -p vela_vm --bench external_compare -- --quick function
cargo bench -p vela_vm --bench external_compare -- --quick float
cargo bench -p vela_vm --bench baseline -- --quick scalar
cargo bench -p vela_vm --bench baseline -- --quick range
```

Full checkpoint command:

```bash
cargo bench -p vela_vm --bench external_compare
```

Measurement rules:

- Compare Vela before/after first.
- Compare Lua/Rhai/Node/Python only as directional context.
- Do not mix `embedded_hot_loop` and `process_hot_loop` into one absolute
  fairness claim.
- Keep raw benchmark logs out of current docs unless a milestone exit decision
  depends on the numbers.

Expected result for a successful first pass:

```text
scalar_branch_loop improves materially, ideally at least 2x before/after.
range_iteration should improve or stay flat.
function_calls should not regress materially.
float_math_loop may stay unchanged until f64 typed bytecode exists.
```

The exact target may be tightened after Phase 0 opcode counts and Phase 2/3
initial measurements.

---

## 8. Validation Plan

Run focused validation after each phase:

```bash
cargo test -p vela_bytecode
cargo test -p vela_vm
cargo clippy -p vela_bytecode --all-targets -- -D warnings
cargo clippy -p vela_vm --all-targets -- -D warnings
cargo fmt --all -- --check
```

Run full workspace validation before marking the track complete:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Required test families:

- Compiler lowering tests for typed i64 operations.
- Verifier rejection tests for invalid typed op operands.
- VM execution tests for typed arithmetic success and failure.
- Range-loop tests for exclusive/inclusive/boundary behavior.
- Conformance tests proving dynamic numeric behavior still works.
- Benchmark contract tests ensuring the external workloads still compile and
  checksums still match.

---

## 9. Completion Criteria

This track is complete enough when:

- `scalar_branch_loop` linked bytecode uses typed i64 instructions for the hot
  numeric path.
- Simple `0..N` i64 loops use a thin i64 range path or have a documented reason
  for deferral.
- Release linked execution no longer repeats structural verifier checks on
  every scalar-loop jump.
- Dynamic numeric scripts still use generic bytecode and preserve runtime
  diagnostics.
- Full `vela_vm` validation passes.
- A before/after benchmark summary exists in commit or PR notes, with durable
  docs updated only if the performance contract or milestone status changes.

---

## 10. Codex Goal Prompt

Use this as the focused goal when starting the implementation:

```text
/goal Implement the typed scalar bytecode optimization plan in
docs/typed-scalar-bytecode-optimization-plan.md. Treat docs/goal.md as the
product roadmap, docs/architecture.md as the technical contract, and
docs/progress.md as current milestone state. Optimize the non-JIT interpreter
for scalar_branch_loop by introducing verified i64 typed bytecode, compiler
lowering from proven type facts, and a thin i64 range hot path. Preserve Vela
language semantics, checked arithmetic errors, source-spanned diagnostics,
hot-reload ABI checks, HostAccess safety, and dynamic generic numeric
fallbacks. Do not implement JIT, script-language generics, implicit numeric
conversions, or the full numeric instruction matrix in the first pass. Work in
small verified phases: opcode/profile visibility, verifier-owned hot path
cleanup, i64 typed instruction support, compiler lowering, i64 range lowering,
hook separation, then only profile-driven superinstructions. After each phase,
run the relevant vela_bytecode/vela_vm tests and benchmark smoke checks; before
completion run cargo fmt --all -- --check, cargo clippy --workspace
--all-targets -- -D warnings, and cargo test --workspace. Commit each coherent
verified checkpoint with Conventional Commit messages.
```

---

## 11. First Task Template

```text
Task: Add opcode-count visibility for scalar_branch_loop.
Context: This begins the typed scalar bytecode optimization track. We need to
measure the current linked bytecode shape before changing instruction lowering.
Expected behavior:
  - A test/helper can compile scalar_branch_loop and report opcode counts.
  - The report identifies generic Rem/Mul/Add/Greater/Jump/RangeNext usage.
  - No runtime semantics change.
Tests:
  - cargo test -p vela_vm --test external_compare_contract
  - cargo test -p vela_bytecode
Do not change:
  - Do not add typed scalar instructions yet.
  - Do not change VM execution behavior.
  - Do not update benchmark baseline numbers.
Validation:
  cargo fmt --all -- --check
  cargo clippy -p vela_bytecode --all-targets -- -D warnings
  cargo clippy -p vela_vm --all-targets -- -D warnings
```

