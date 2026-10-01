# Typed cancellation and real stdio acceptance

B08.5 owns eight obligations: cancel S11 positive/negative, stale-version and
cancellation states and client profiles; stdio S0 positive/negative and real
process transport. Ten editor lifecycle obligations still keep B08 open. B16
remains deferred; each registered platform needs its own fresh evidence.

`request-cancellation` authors a complete marked declaration/call pair, unknown
call and shifted source independently. LF/CRLF and three diagnostic/client
profiles share exact source facts. Integer extremes/zero and Unicode, numeric-
looking and empty string request IDs traverse the actual dispatcher and scheduler.
Tests cancel real queued work both before receiving it and after completion but
before publication. Duplicate cancellation still yields exactly one typed
RequestCancelled response, without any old locations. Text/version, SourceDb,
parse count and generation stay unchanged. Reusing the same ID and repeated
later requests return exactly the two marked locations or the unknown-call empty
set; unknown/late/malformed cancellations do not poison subsequent work.

Integer 20 and string "20" are distinct request owners. Cancelling the wrong
typed ID permits the held task to complete with exact locations. Holding both
tasks across a new source generation and cancelling only integer 20, before or
after the change, yields exactly RequestCancelled for integer 20 and
ContentModified for string "20". Cancellation takes priority for its own target;
subsequent requests return the new authored ranges. Temporary disk bytes remain
unchanged. The shared scheduler test helper keeps existing integer call sites
and adds explicit RequestId scheduling without changing production code.

The stdio audit now discovers and runs the actual `stdio_transport` integration
target in addition to both Rust libraries. It preserves library results when
merging targets and rejects duplicate identities or undiscovered results.
Its integration executable lives in an isolated target directory: building a
test-profile binary must not overwrite the dev binary fingerprinted by VSIX
and native evidence. The initial normal-target run exposed this overwrite as a
real stale-server rejection. Preserve the strict hash gate and collect fresh
editor evidence after correcting build ownership.
The enclosing editor audit derives its deadline from the Rust suite count;
each suite keeps the existing ten-minute bound and each child the ten-second
exit bound, with discovery/report overhead allowed separately.
Negative Node tests prove that library-only, discovery-only, failed and ignored
process results cannot close stdio requirements. Cargo command failures still
fail the entire audit. Integration logs are separate and no cross-platform
merging is introduced.

An owned real binary starts with stdio and disabled watchers. The test parent
drains both output pipes while a finite writer emits frames; a ten-second exit
deadline and one-MiB output limits prevent a stalled child from hanging tests.
The guard kills and reaps the owned child on failure. The original process
test retains its assertions and now uses this bounded helper. Output is validated
by lsp-server's typed frame reader, then the original wire JSON is inspected;
the helper does not synthesize a missing jsonrpc field after typed decoding.

Seven-byte writes cut headers and UTF-8 bodies; concatenated messages carry
Chinese/non-BMP source and signed versions in LF/CRLF under three client profiles.
The reviewed declaration corpus independently supplies all seven full diagnostic
facts, labels, ordered candidates and repair counts. Rootless scratch analysis
has an explicitly authored `main::BadFields.size` diagnostic owner rather than
the package corpus's `invalid::BadFields.size`; all other facts/ranges remain exact.
Repair clears facts, an older version cannot replace them, close clears scratch,
shutdown echoes its Unicode ID and exit ignores the following queued request.
Uninitialized/unknown-method/request-as-notification errors stay exactly framed;
malformed notifications stay silent and default stdout/stderr have no log noise.
EOF exits successfully; nonnumeric/missing lengths, truncated bodies and invalid
JSON exit unsuccessfully with stderr and no stdout junk, within the same deadline.
The integration target also runs six existing shared fixture self-tests under
its own namespace, separately from the four actual process tests.
