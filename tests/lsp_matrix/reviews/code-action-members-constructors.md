# S4 member and constructor code actions

B05.20 adds a shared LF/CRLF fixture for source fields, nested writes, inherent
and trait methods, unit/tuple/record enum variants, named tuple arguments,
record labels and shorthand fields. The valid file has no diagnostics or code
actions. The invalid file has one supported automatic repair: a `Box` record
constructor missing its required `extra: Any` field. The service and protocol
pin the one Quick Fix, its insertion point and replacement, apply it to the
whole source, and prove the diagnostic and action disappear. The live service
agrees with a fresh instance, and the protocol agrees with a fresh server.
Closing the dirty protocol document restores the disk diagnostic and action.

The constructor sits after Chinese and a non-BMP character on the same line,
so the protocol's UTF-16 edit column differs from its byte column. The
workspace path also contains encoded Unicode, space and percent characters.
The action is versioned while the document is open and unversioned after
close. Other marked source member misspellings, a wrong constructor label, an
unknown enum variant and an `Any` receiver have empty action sets. These
sites have no safe diagnostic-backed replacement from the current source
facts; the test makes that boundary explicit instead of guessing a target.
An unclosed record constructor also has no insertion action even though it
publishes syntax diagnostics.
Existing schema lifecycle tests cover typed host-field typo replacement and
stale schema removal.
