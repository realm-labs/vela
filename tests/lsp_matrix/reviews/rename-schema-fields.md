# Schema record-field ownership

B04.08 advances S4/S6 references, highlights, prepareRename and rename partitions
at service/protocol layers. Broad cells remain unreviewed; callable, schema-variant
and remaining semantic/lifecycle ownership need their own evidence.

The shared independent three-file fixture has 14 query points, 14 field sites,
four ownership groups and three local-binding probes. It distinguishes two
source-backed schema fields, a same-named source struct field and a metadata-only
schema field. Qualified and imported-alias constructors, explicit/shorthand match
patterns, dot reads/compound writes, a closed consumer, unknown field/owner labels
and literals are covered. Existing-field and same-owner unknown-label capture
reject; another schema owner can independently adopt that unknown spelling.

Both drivers repeat exact reference sets with/without declarations, kinds,
highlights, definition/prepare targets and complete edits. Three writable groups
are actually renamed, entire files compared and parsed, schema metadata regenerated,
then all queries repeated before and after restoration. Metadata-only fields have
their own reference sets but null definition/prepare/rename results. Local probes
prove that shorthand expansion preserves parameters and pattern bindings.

Protocol uses Unicode percent-encoded paths, LF/CRLF, actual didChange, schema
file-watch notifications, both UTF-16 edit forms, open/closed client versions and
schema ABI confirmation annotations. Applying source edits does not regenerate
the artifact implicitly: the driver does this explicitly at the metadata boundary.
Service additionally checks canonical symbol identity and schema ABI risks.

The full parallel Windows run exposed colliding timestamp-only fixture roots.
The protocol driver now uses the existing atomically sequenced temporary-root
allocator (with its frozen-clock concurrency regression) and keeps its encoded
Unicode child path and guarded cleanup. No behavioral assertions were relaxed.

Earlier B04.08 commits repaired omitted constructor/pattern rename edits and
alias shorthand references. References and rename now share scoped schema label
ownership rather than separate short-name matching. The focused service regression
also checks applied/rebuilt ownership, exact classifications and negative cases.

Original B00-B03 acceptance snapshots stay fixed. Fresh Windows evidence belongs
to the checkpoint's current profile; macOS needs independent fresh evidence when
used. Resume B04.09 callable/schema-variant ownership and subsequent semantic,
lifecycle and UX05/UX06 gates. B16 remains deferred.
