# Close ownership and unsolicited save notifications

B08.4 owns twelve obligations: did-close S0/S11 positive/negative at service
and protocol layers, its client profiles, and did-save S0 positive/negative
and client profiles. Standalone cancellation and process/editor lifecycle stay
open; B16 is deferred.

The independently authored `document-change` corpus supplies seventeen complete
marked source documents and explicit initial/shifted/repaired/restored diagnostic
oracles. This batch reuses those reviewed inputs; it adds close/save assertions,
not duplicated fixture data. Declaration/state/recovery/body failures give real
non-empty facts to restore or clear. Chinese/non-BMP source and LF/CRLF are exact.

Closing removes the overlay and its version ownership. Existing disk sources
restore complete bytes, initial source version and exact diagnostic code,
severity, message, UTF-16 primary/label ranges, ordered candidates and repair
count. Scratch sources absent on disk disappear from SourceDb and clear their
diagnostics; unrelated text never enters analysis. Old workspace/database
snapshots retain the overlay and its facts. Repeated results agree with authored
expectations and fresh databases or servers. Unknown/repeated closes are controlled
and cannot remove another overlay. Malformed notifications cannot mutate state.
Reopening at i32::MIN after closing i32::MAX verifies per-session version reset.

Service tests supply disk sources explicitly, preserving the filesystem-neutral
architecture. Updated source, deletion and recreation prove that a closed source
comes from the supplied current disk revision. Protocol tests change only owned
temporary files, validate their canonical containing directory, and restore
physical fixture bytes before exit. Changed and deleted files are re-read at
close without a watcher; a saved scratch source becomes an ordinary disk source.
Removing it later cannot leave a phantom SourceDb entry. Other open text/version
and the full owned fixture remain intact.

Closing a defining module rebinds its open importer to the disk parameter/docs
target and republishes current diagnostic facts for both documents. Immutable
snapshots retain the former target. A completed real references task held across
close publishes only the exact ContentModified error; old locations cannot leak.
Repeated subsequent hover returns the authored complete Markdown and UTF-16 range.

The advertised textDocumentSync explicitly has save:false. A valid unsolicited
didSave is therefore a no-response no-op, including omitted/null/poisoned optional
text, open/closed/unknown/non-source documents, and three client profiles.
Malformed payloads stay silent. Current text/version, SourceDb, parse count and
generation do not change; no phantom document is created. A real physical disk
change while the overlay is open does not make save consume the supplied text or
clear diagnostics. Close is the point that adopts current disk bytes and repairs
the independently marked declaration failure. This preserves the existing
advertised protocol policy rather than introducing unsupported save synchronization.

Fresh VSIX and native artifacts must share the frozen source/server on the
registered local platform. Accepted B00-B07 and prior children are preserved;
macOS requires independent fresh evidence and cannot borrow Windows results.
