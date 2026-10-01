# Open-document synchronization

B08.2 owns the seventeen did-open obligations: S0/S1/S9/S11 positive and
negative at service/protocol layers, plus protocol client capabilities. Change,
close/save, cancellation and process/editor lifecycle gates remain separate.

`document-open` reuses independently marked top-level and recovery corpus inputs
and authors its own opening phases. The fifteen documents include every S1
declaration family, duplicate owners, invalid state forms, removed global syntax,
empty/comment-only sources, a scratch file absent from disk, an unrelated text
file, incomplete member/call/type/declaration contexts and a cross-module API.
No expected fact is populated by a provider result. Diagnostic code, severity,
message, complete range, labels, ordered candidates and repair-hint count match
the authored corpus; primary ranges and labels include Chinese/non-BMP prefixes.

Both layers run LF/CRLF, original and prefixed complete source text, and versions
0, -1, i32::MAX and i32::MIN. Signed protocol bits are opaque service identities.
Each accepted opening yields one current document, exact text/version and source
record, a new generation, and precise current diagnostics. The unrelated text
document retains its workspace buffer but contributes no Vela analysis input.
Old workspace/database snapshots remain immutable, current diagnostics repeat,
and a fresh analysis of the same disk/overlay state matches the independent facts.
All physical disk inputs remain unchanged; scratch/text inputs remain absent.

Opening a defining module with unchanged source, a body-only change, and a changed
parameter/docs preserves or replaces exact importer Markdown and ranges. The
signature change republishes the open importer's current empty diagnostics.
The service rejects old-generation diagnostic publication. Protocol requests run
through the real scheduler: hold a completed references task, accept didOpen, then
attempt publication. Only the exact ContentModified error may appear, followed by
repeated current hover results; old locations/documentation cannot publish.

Three client capability profiles retain identical source/diagnostic semantics.
Seven malformed notification payloads cannot replace current sources or change
generation. Exact advertised textDocumentSync remains openClose=true/change=2/
save=false. The fixture uses the supported doc attribute, not comment conventions
from another language. Cleanup verifies the canonical owned temporary parent.

The reproducer exposed document notifications writing source and publishing
diagnostics before initialize and after shutdown. Notification dispatch now
enforces the existing session boundary for all ordinary notifications. Preserve
the existing initialize/initialized/exit/cancel pre-initialize policy and allow
exit/cancel after shutdown. Valid source/config/root/watch notifications before
initialize or after shutdown produce no source mutation or publication. This
corrects lifecycle enforcement without changing runtime or script semantics.

Fresh editor/native evidence belongs to the current registered platform and
source/server identity. Another machine must capture its own evidence; B16 stays
deferred. This child does not close the whole B08 batch.
