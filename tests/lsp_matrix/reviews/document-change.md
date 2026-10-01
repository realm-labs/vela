# Document change and rejected-edit diagnostics

B08.3 owns twenty-three did-change obligations: S0/S1/S2/S9/S11 positive and
negative at both layers, plus protocol stale-version/cancellation/client profiles.
Close/save, standalone cancellation and process/editor lifecycle remain separate.

`document-change` contains seventeen independently marked documents. It retains
the reviewed declaration/recovery corpus and adds nested function/local binding,
shadowing, closure writes, guarded match/loop control and unresolved body input.
Each source is opened, shifted, repaired and restored. LF/CRLF runs match complete
source/version ownership and full diagnostic code, severity, message, primary
range, label ranges, ordered candidates and repair count. Fixing duplicates,
invalid state/legacy forms, incomplete contexts and unresolved body facts clears
their diagnostics; restoration reproduces exact original facts. Old snapshots
stay immutable, repeated queries agree, and fresh analysis matches the oracle.
The service remains editor-neutral; protocol owns message and UTF-16 validation.

Three client profiles retain identical semantics. Signed source versions advance
from -7 through -6/0/i32::MAX with opaque bit preservation. Equal/older versions
and i32::MIN do not change text, parse count, generation or publication. A missing
overlay rejects ranged edits but retains the existing full-replacement creation
policy. Seven malformed payloads stay silent without losing current ownership.

Authored intermediate source markers define a multiple-edit packet: lengthen a
number, edit its new range, then insert at EOF. Mixed full/ranged edits use the
post-replacement source. Chinese/non-BMP and LF/CRLF bytes remain exact. Deprecated
rangeLength is not trusted; the actual range controls replacement. Out-of-document,
out-of-line, surrogate-splitting, reversed and CRLF-overrun ranges reject. A valid
first edit followed by an invalid edit, including a full/ranged mixture, rolls
back the entire packet. No version or generation is consumed; a valid version-2
repair still succeeds.

The failure exposed invalid edit notifications clearing current diagnostics with
an empty list even though the source was unchanged. Rejected edits now publish
the exact current diagnostic facts alongside the existing controlled error field.
This keeps clients' current problems intact, without writing source or changing
normal synchronization/projection behavior. The reproducer includes an unresolved
method so an empty publication cannot falsely pass.

Body-only defining-module edits reparse one document, preserve declaration/import
indexes and rebuild HIR facts. Parameter/docs changes update importer hover and
republish current importer diagnostics. A completed real references task held
across didChange can publish only ContentModified; explicitly cancelled work can
publish only RequestCancelled. Neither leaks old locations, and repeated current
hover remains valid. Physical fixture bytes and absent scratch/text inputs stay
unchanged; cleanup validates its canonical owned temporary parent.

Shared test support is extracted by responsibility and all original B08.2 test
bodies remain. Fresh source/server evidence belongs to one registered platform;
macOS needs its own capture. B16 stays deferred and the whole B08 remains open.
