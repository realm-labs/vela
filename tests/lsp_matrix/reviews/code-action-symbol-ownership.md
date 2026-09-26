# S10 symbol ownership for code actions

B05.22 covers the four `code-action/syntax/S10/{positive,negative}/{service,protocol}`
cells. The shared import fixture now places a parameter named `award`, a local
named `spare`, and a local named `FLAG` beside unresolved uses of the same
public source names. Declaration and use sites of these local owners return
empty action arrays. The unshadowed uses still offer one exact import action;
the unused source alias still offers removal. Valid source, schema and stdlib
aliases, module paths, private names and ambiguous public names also remain
action-free. Service and protocol assert these results under LF and CRLF.

Existing independent fixture assertions complete the ownership map: source
member and variant misspellings and dynamic `Any` access do not receive guessed
edits; a missing source constructor field does receive one structural edit.
Schema host-field and builtin `Array` method typos receive typed replacements.
Protocol checks full versioned UTF-16 edits and encoded URIs. Applying the
supported import, constructor, schema and builtin fixes clears the selected
diagnostic while retaining unrelated source; fresh service/server instances
agree with current state. The schema test also checks missing and replaced
schema facts, so actions never retain stale host ownership.
