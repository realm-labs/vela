# Active File-Size Exceptions

The ordinary 1200-line rule remains the default. This list records the active
files reviewed above that threshold where mechanical splitting would obscure
one exhaustive contract or turn dense fixture groups into navigation-only
wrappers. Any unlisted active file above 1200 lines fails the architecture
audit. Exceptions should be removed when a responsibility boundary becomes
clear; they are not permission for unrelated growth.

| File | Reviewed reason |
|---|---|
| `vela_vm/src/linked_execution.rs` | Exhaustive linked opcode dispatch and its root/frame-driver glue. Execution-session/frame/continuation definitions and start policy live in `execution_session.rs`, async boundary/resume policy in `async_resume.rs`, and reentry push/abort policy in `execution_reentry.rs`; this exception does not cover adding those responsibilities back. |
| `vela_lsp_server/src/global_state.rs` | One typed LSP state machine and its message-transition fixture matrix; splitting transitions from queue/state ownership would duplicate protocol setup. |
| `vela_lsp_server/src/lsp/to_proto.rs` | Exhaustive protocol projection table whose variants are reviewed together. |
| `vela_vm/src/runtime_type_guards.rs` | Mutually recursive exhaustive guard interpreter; container, sum, callable, and identity cases share cycle/stamp state. |
| `vela_vm/src/script_method_calls.rs` | Exhaustive standard/dynamic method router with one fallback-order contract. |
| `vela_bytecode/src/linked.rs` | Declarative linked instruction and immutable layout definitions. |
| `vela_bytecode/src/lib.rs` | Declarative unlinked bytecode instruction, operand, metadata, and compiled-program definitions reviewed as one public format contract. |
| `vela_bytecode/src/linker.rs` | Single generation-sealing pass whose instruction, identity, provider, debug, and verification mappings must remain auditable together. |
| `vela_bytecode/src/verification.rs` | Exhaustive unlinked instruction verifier and shared invariant helpers. |
| `vela_bytecode/src/verification/linked.rs` | Exhaustive linked instruction verifier; every linked opcode must remain in the same match audit. |
| `vela_mir/src/verifier/operations.rs` | Exhaustive MIR statement, terminator, place, and state-operation verifier kept together so every executable operation participates in one invariant audit. |
| `vela_analysis/src/registry.rs` | Declarative registry-to-analysis projection for the complete metadata surface. |
| `vela_syntax/src/ast/items.rs` | Declarative CST-backed AST wrappers for the complete module-item surface; declaration accessors share one casting and contextual-keyword contract. |
| `vela_lsp_server/src/tests.rs` | Dense typed-protocol fixtures. |
| `vela_lsp_server/src/tests/signature.rs` | Dense signature-help fixture matrix. |
| `vela_syntax/src/ast/expr_tests.rs` | Dense AST fixture matrix. |
| `vela_syntax/src/parse/tests.rs` | Dense parser fixture matrix. |
| `vela_vm/src/tests/type_guards.rs` | Dense end-to-end guard contract matrix corresponding to the exhaustive guard interpreter. |
| `vela_bytecode/src/verification/tests.rs` | Dense negative verifier fixture matrix. |
| `vela_hot_reload/src/tests/runtime_reports.rs` | Dense runtime staging, acceptance, rejection, and diagnostic-report fixture matrix. |
| `vela_language_service/src/semantic_tokens/tests.rs` | Dense semantic-token classification fixture matrix covering the complete editor-neutral taxonomy and contextual declaration cases. |
| `vela_language_service/src/rename/tests.rs` | Dense rename prepare/apply/rejection fixture matrix across declaration and reference categories. |
| `vela_engine/src/tests/source_reload/runtime_safe_points.rs` | Dense runtime safe-point, staged update, async ownership, and rollback fixture matrix. |
| `vela_mir/src/tests/model.rs` | Dense MIR model invariant fixtures. |
