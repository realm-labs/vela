# B06 semantic-token schema lifecycle review

This child owns the six full/delta/range protocol missing_schema and stale_schema
obligations. Its independent five-file fixture specifies every token's text,
kind, complete modifiers and UTF-16 coordinates across 25 states. LF and CRLF
runs cover 50 workspace states and 100 complete document streams. Caller bytes
stay fixed: metadata changes alone must change the relevant token facts.

| Partition | Independent expectations |
| --- | --- |
| Missing and invalid schema | Disk-only and open callers work without schema. Deletion, truncated JSON and an unsupported format version remove all schema ownership, callable, member and variant facts. Unknown type hints remain lexical type tokens; unknown qualified calls and variants are unresolved. Every open-file update publishes exactly one schema::unavailable diagnostic with the corresponding controlled reason, and repair clears it. |
| Valid empty schema | Empty facts produce the same complete source-only stream and ID as missing schema, with no schema::unavailable diagnostic. Diagnostic state cannot contaminate semantic-token identity. |
| Individual removals | Rename both fields, both methods, the trait method and the enum variant in separate replacements. Removed members lose property/method provenance; the variant is unresolved while its known enum owner remains schema-owned. Unchanged facts retain their exact roles. Each repair restores the original stream and ID. |
| Dynamic factory | The imported schema factory changes HostCell return to Any. The factory remains a host/schema function; downstream returned members become lexical, while explicitly typed receivers retain their current metadata. Repair restores ownership. |
| Host versus record | Replace both the named HostCell type and factory return with a record fact. Its fields retain schema provenance and lose host provenance; schema methods retain their callable policy. Restore the host facts and original IDs. |
| Source and builtin control | The caller's LocalCell declaration, field and typed receiver remain source-owned throughout. A real math::abs call and primitive hint retain builtin provenance. A closed anchor file supplies real nonzero SourceIds and byte spans for differently named schema items; sourceSpan does not confer source ownership on metadata. |
| Closed caller | Close the importer, then replace fields, delete and recreate schema while it remains closed. Diagnostics are cleared on close and no closed-file diagnostics are published by watch events. Current complete token streams still agree with newly initialized servers. |
| Full/delta/range | Each state checks an independent complete stream, repeat fulls and IDs, applied deltas from the preceding, original missing and original valid IDs, current-ID empty deltas, each token/line range and empty token-end/document-start ranges. Token ranges repeat and agree with fresh servers reading actual disk and explicit overlays. Queries cannot parse, rebuild project/HIR or advance the generation. Exact disk/overlay and effective SourceDb text remain fixed. |

The reproducer found that external callable analysis used the unexpanded alias
spelling for schema/stdlib return, target/effect and callback facts. A shared
external-call path resolver now follows one unambiguous import after binding and
source ownership checks. A 21-case analysis regression covers module/function
aliases, stdlib aliases, host origins/effects, private/noncallable source owners,
local Any/unknown shadowing, missing targets, duplicate imports and literal task
capabilities. It also rejects invalid descendants of a source function rather
than treating the function prefix as the complete callable. A callback regression
checks host, Any, missing and repaired parameter facts through the same alias.

Expected token kinds and modifiers never come from a provider, and no scripts or
live host code execute. The remaining nine B06 editor/interaction obligations
stay open. B00-B05 snapshots are preserved, profiles require independent fresh
evidence, and B16 stays deferred.
