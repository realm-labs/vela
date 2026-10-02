# Workspace Trust installed workbench review

B09.6 owns UX21 untrusted-open and grant-trust Input/Render obligations. Vela
explicitly declares `untrustedWorkspaces.supported:false`, preserving the pinned
editor's previous default for its native runtime launcher. The plain policy
description explains why native activation requires trust. The private installed
observer supports Restricted Mode only to read state and query providers; its
API has no trust, document or configuration mutation.

The new lane opens a second sequential ordinary installed window with separate
private user/shared data and Workspace Trust explicitly enabled. It never passes
`--disable-workspace-trust`. The first trusted window exits before this window
starts. An owned copy of the installed bundled server is configured through
workspace settings; its bytes/hash are retained as evidence. The trust window
has independently pinned display, settings, locale, platform and folder identity.
Its own PID-marked host log directories cannot be confused with the first window.

Native refusal of the startup trust prompt establishes Restricted Mode. Quick
Open and actual Home/prefix typing create an unsaved Unicode caller. Full caller
text, unchanged disk bytes and exact caret remain unchanged after physical F12;
the definition provider is empty and Vela/server remain inactive, with no owned
server trace. Disabled extensions may be absent from the runtime extension API.
Policy observations therefore read the already verified installed manifest and
re-query runtime presence/activation after each trust observation.

Native Extensions search and pointer selection open the installed Vela detail
page, which renders the exact declared disabled policy. The status icon supplies
a leading layout NBSP; the raw string/aria are retained and the complete semantic
description uses the existing hover paragraph whitespace normalization. No
word, error body or policy detail is discarded. Restricted Mode's full banner
and trust-editor heading/button are independently pinned from primary code in
the supported desktop. The grant action is `Trust`; `Trust Folder` labels the
trusted-directory picker and is not this editor's grant action.

Actual trust grant restores permitted activation without an API mutation. Native
Quick Open retains the same dirty caller, exact provider and separate F12 wire
request/Location, current target caret, complete caller/target diagnostic clears,
and trusted banner/editor state. The current configured executable must match
the installed bytes, launcher command and single owned server session. The
server only answers LSP queries; this lane never executes scripts or host logic.
Marked LF/CRLF fixtures and literal UTF-16 goldens independently pin the caller
and declaration; Unicode precedes the ASCII native input line.

The dedicated observer publishes its endpoint atomically, uses fresh owned
session identity and permits bounded transport retries for read-only observation
during host transitions. Native input and finish are never replayed. HTTP/observer
logic failures are not retried. Both 90-second proofs fit a separately reviewed
210-second window budget with setup; the existing trusted 450-second budget stays
unchanged. A targeted Windows run passes 20+11 actual actions and 8+10 complete
checks. Its screenshots/aria, setup, private binary, logs and session identities
are required evidence, not expected-value generators.

All original 48 native contract hashes remain unchanged. Whole B09 acceptance
also requires fresh full 50-proof native input, installed VSIX regressions and
strict previous/current batch gates on the same frozen source/profile/server.
macOS requires its own fresh capture when development switches machines; it may
not borrow Windows proof. B16 environment expansion remains deferred.
