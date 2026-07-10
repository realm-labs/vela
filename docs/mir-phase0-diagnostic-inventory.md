# MIR Phase 0 Compiler Diagnostic Inventory

This inventory freezes every user-facing diagnostic built by the direct
bytecode compiler before MIR construction. Syntax and HIR semantic diagnostics
are pass-through values and retain their upstream owners. The table covers all
24 distinct coded diagnostics constructed under `compiler.rs` and
`compiler/**` outside tests.

| Code | Message contract | Primary span | Current seam | Final owner |
|---|---|---|---|---|
| `compiler::invalid_int_literal` | `invalid integer literal \`{literal}\`: {error}` | HIR literal expression | `error.rs`, `const_eval.rs` | analysis/pre-MIR literal validation |
| `compiler::invalid_float_literal` | `invalid float literal \`{literal}\`: {error}` | HIR literal expression | `error.rs`, `const_eval.rs` | analysis/pre-MIR literal validation |
| `compiler::type_contract_mismatch` | `type contract mismatch for {context}` with expected/actual label | actual value/default expression | `value_types.rs` | analysis type-contract validation |
| `compiler::unresolved_native_function` | `unresolved native function \`{name}\`` | whole call | `calls.rs` | compile-target validation |
| `compiler::unresolved_method` | `unresolved method \`{method}\`` | whole method call | `calls/metadata.rs` | compile-target validation |
| `compiler::unknown_named_argument` | `unknown named argument \`{name}\`` | offending argument | `call_args.rs` | analysis call placement |
| `compiler::positional_after_named_argument` | `positional argument after named argument` | offending argument | `call_args.rs` | analysis call placement |
| `compiler::too_many_arguments` | `too many arguments` | first extra argument | `call_args.rs` | analysis call placement |
| `compiler::duplicate_argument` | `duplicate argument for parameter \`{name}\`` | duplicate argument; prior argument is labeled | `call_args.rs` | analysis call placement |
| `compiler::missing_required_argument` | `missing required argument \`{name}\`` | whole call; parameter declaration is labeled when available | `call_args.rs` | analysis call placement |
| `compiler::invalid_identity_comparison` | `` `{op}` requires reference identity operands, but the {side} operand has type `{type}` `` | binary expression; offending operand is labeled | `hir_lowering/operators.rs`, `expression_checks.rs` | analysis operator validation |
| `compiler::missing_comparison_trait` | `` `{type}` does not implement `{trait}` for `{operator}` `` | binary expression | `expression_checks.rs` | analysis operator validation |
| `compiler::missing_ord_for_array_ordering` | `` `Array.{method}` requires an `Ord` {key-or-element}, but `{type}` does not implement `Ord` `` | whole call | `calls.rs` | analysis call/operator validation |
| `compiler::unknown_constructor_variant` | `unknown enum variant \`{enum}::{variant}\`` | whole constructor | `schema_defaults.rs` | analysis constructor validation |
| `compiler::duplicate_constructor_field` | `duplicate constructor field \`{name}\`` | duplicate field; prior field is labeled | `schema_defaults.rs` | analysis constructor validation |
| `compiler::unknown_constructor_field` | `unknown constructor field \`{field}\` for \`{type}\`` | field/argument name | `schema_defaults.rs` | analysis constructor validation |
| `compiler::missing_constructor_field` | `missing constructor field \`{field}\` for \`{type}\`` | whole constructor | `schema_defaults.rs` | analysis constructor validation |
| `analysis::field_not_writable` | `field is read-only for script writes` | assignment or mutating call | `hir_lowering/assignments.rs` | analysis plus compile-target access validation |
| `analysis::host_index_not_supported` | `type \`{type}\` does not support host index access` | index operation; receiver is labeled | `hir_lowering/assignments.rs` | compile-target validation |
| `analysis::host_index_not_readable` | `type \`{type}\` does not allow host index reads` | index read | `hir_lowering/assignments.rs`, `host_paths.rs` | compile-target validation |
| `analysis::host_index_not_writable` | `type \`{type}\` does not allow host index writes` | index assignment | `hir_lowering/assignments.rs`, `host_paths.rs` | compile-target validation |
| `analysis::host_index_not_mutable` | `type \`{type}\` does not allow host index mutations` | compound index assignment | `hir_lowering/assignments.rs`, `host_paths.rs` | compile-target validation |
| `analysis::host_index_not_removable` | `type \`{type}\` does not allow host index removals` | indexed remove call | `hir_lowering/assignments.rs`, `host_paths.rs` | compile-target validation |
| `analysis::host_index_key_mismatch` | `host index key for \`{type}\` must be \`{expected}\`` | index operation; actual key is labeled | `hir_lowering/assignments.rs` | analysis plus compile-target validation |

All direct `Diagnostic::error` builders in the audited scope set both a code
and a primary span. One projection gap remains: negated const/schema-default
integer or float overflow can leave `const_eval::evaluate_const_expression`
before the literal origin is attached. Phase 0 must fix and test that path.

The legacy `CompileErrorKind` variants have these final assignments:

| Variant | Current projection gap | Final owner |
|---|---|---|
| `FunctionNotFound` | no code or span | source-front-door API error |
| `UnknownLocal` | no code; some branches also lack a span | source-spanned `MirBuildError` for inconsistent binding/input |
| `RegisterOverflow` | no code or span | bytecode backend physical-limit error |
| `BytecodeVerification` | verifier payload only | bytecode backend/verifier error |
| `UnsupportedSyntax` | no diagnostic projection and inconsistent spans | real language-invalid cases move to HIR/analysis/target validation; missing semantic input becomes `MirBuildError`; jump/operand/layout cases become backend errors |
| `SyntaxDiagnostics` | wrapper has no code/span | contained syntax diagnostics retain syntax ownership |
| `SemanticDiagnostics` | wrapper has no code/span | contained HIR/analysis/target diagnostics retain their owner |

`UnsupportedSyntax` must not survive the hard switch as a semantic catch-all.
Break/continue placement, named-argument restrictions, and non-constant schema
defaults need ordinary user diagnostics. Missing HIR bodies, blocks,
statements, expressions, patterns, paths, captures, or parameter-default
bodies need source-spanned `MirBuildError` values. Jump patching, dynamic host
operand counts, and physical layout failures belong only to the bytecode
backend.
