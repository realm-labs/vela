# Initialization and terminal session policy

B08.1 owns thirteen protocol obligations: initialize S0/S11 positive/negative
and client capabilities, initialized S0 positive/negative and client capabilities,
and shutdown-exit S0/S11 positive/negative and client capabilities. Source-sync,
cancellation, actual framed process transport and UX01/UX18 remain separate.

`session-init` authors the complete initialize result, all capability options,
42 custom token types and 16 modifiers. Five profiles cover omitted capabilities,
standard types/modifiers, explicit empty lists, reversed custom lists with duplicate
and unsupported names, and launch-disabled watching. Expected order and fallback
legends are authored independently; provider results never construct expectations.
The server version is compared to the declared crate version. Dynamic watching
registers exactly once only when supported and enabled; full registration request,
ID, method, source/config roots, glob patterns and kind 7 are exact. Duplicate
workspace folders plus rootUri retain one owned root.

Each profile initializes a fresh physical package in an encoded Chinese/space/%
root, loads both source modules without didOpen, and returns exact cross-module
hover Markdown and UTF-16 range at a marked Unicode position. Repeated initialize
cannot replace roots, capability policy or source facts. An initialized request,
malformed notification and early notification cannot improperly register/unlock
the server. Proper notification registers once, repeated notification/client reply
stays silent, generation and full physical bytes remain unchanged.

Eight malformed initialize payloads cover null/missing capabilities, wrong
capability/process/root/folder types and malformed editor/schema options. Exact
response ID/code/object fields and controlled `invalid initialize params:` prefix
are checked; serde's explanatory suffix is not a stable public contract. State,
empty SourceDb and generation remain untouched, initialize notifications stay
silent, early shutdown is rejected and a valid initialize recovers exact facts.

Two shutdown branches reject malformed request params without closing, ignore a
shutdown notification, return exact null for the real request, and reject all
subsequent ordinary requests before parameter/method dispatch. Malformed exit does
not terminate. Proper exit is silent; the existing Vela exit-request policy returns
InvalidRequest and terminates. Flags and generation are checked throughout.

Two actual production `run_connection` conversations enqueue all seven messages
before starting the main loop. Both exit forms must end it within five seconds;
only the exact initialize/shutdown/error responses exist. An already queued later
initialize and invalid-source didOpen produce neither response nor diagnostics.
The completed thread is joined after bounded completion. This proof does not rely
solely on TestServer's terminal-message filtering and does not claim stdio framing.

The matrix exposed canonical Windows package source roots leaking `\\?\` into
watcher file URIs (`file:///?/...`) and losing percent encoding. URI output now
converts Windows verbatim drive/UNC spelling to ordinary paths before standard
URL encoding; physical project path handling stays at the filesystem boundary.
Independent cases check ordinary/verbatim drive and verbatim UNC, Unicode/spaces/%
encoding, no query/fragment and filesystem conversion. Unix input is preserved,
including literal backslash filenames, for its independent platform run.
Owned roots are checked before recursive cleanup, and all fixture disk bytes are
checked unchanged. Fresh Windows/macOS gates remain independent; B16 is deferred.

With valid watcher roots, native acceptance exposed a late rename file notification
invalidating the following references request. The driver now observes completed
workspace mutations and a 750 ms quiet window, longer than the installed client's
250 ms filesystem-event batching, before issuing the single references shortcut.
The bounded observation restarts on each mutation and never retries accepted input.
Unknown hover/signature absence also requires an actual completed response, not
the main-loop dispatch record with zero output. Independent driver tests reject
pending mutations, mismatched acknowledgements, stale responses and incomplete
trace records. All existing native actions and exact feature assertions remain.
