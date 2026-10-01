# Installed server failure and native reload recovery

B08.7 covers the six UX18 Command/Input/Render obligations for server-stop/reload
and invalid-server-path. The independently authored `input-lifecycle.json`
contains a marked call and definition, literal UTF-16 coordinates, an unsaved
Chinese/emoji prefix, immutable disk bytes and exact extension error messages.
Fixture tests check marker positions against those literals before any capture.

Both routes begin with real typing and F12. The complete real-client definition
request and response must agree with the authored file, position, target range
and text, and the actual editor must land there. Returning to the call preserves
the complete dirty source, language mode, selection and unchanged disk bytes.

The stop route captures exactly one server under the test window, validates its
executable, creation identity and parent ancestry, then suspends it. A real
`vscode.executeDefinitionProvider` request must appear in the verbose client
trace with no response and remain unresolved before that captured process is
stopped. Its result must be exactly empty within five seconds. Native Vela: Show
Output must display the exact connection-closed message in the Vela channel.
Three process/startup observations spanning at least one second require zero
servers and exactly one startup attempt, preserving the no-auto-restart policy.

The invalid-path route sets an absent executable under the private workspace,
uses native Developer: Reload Window and Quick Open, and requires zero owned
servers after the new host settles. The visible Vela output must contain the
authored startup-error prefix and the exact configured executable. Monaco's
actual text nodes retain the path across visual wrapping; screenshots and ARIA
retain the rendered presentation. A provider request returns exactly empty
within five seconds, and the same stopped/startup policy applies. The route
removes the workspace override before recovery.

Native reload must create a new installed extension host and one server, retain
the exact dirty source and original call caret, and return the complete expected
target from a separate command query. F12 must again produce exactly one real
completed request and move the actual editor to the authored definition. Every
action runs once; readiness polling only observes state. Each route has a
60-second deadline under the external run's 180-second bound.

Investigations exposed why development-window reload cannot prove this behavior:
it restored disk source and discarded the dirty prefix. Native acceptance now
uses an ordinary window with Vela and its test-only observer installed into
private extension, user-data and shared-data directories. Observer activation
survives reload. A PID-marked output directory identifies each host's logs even
when VS Code shares the parent exthost directory. Raw per-session snapshots,
typed receipts, failures and recovery screenshots remain in the hashed bundle.
New-host readiness also observes termination of the preceding host's server;
an old process still exiting is not treated as a settled invalid-path result.
Full regression exposed a second ordinary-window difference: Settings opened
modally, so the previously accepted Ctrl+Tab return did not leave that overlay.
Both registered profiles now explicitly pin `workbench.editor.useModal: "off"`
to retain the established tab route. The failed full attempt remains outside
acceptance, all captures are recollected, and prior snapshots/action hashes
remain unchanged. No source/style expectation or input action is relaxed.

Windows thread suspension and macOS SIGSTOP share exact expectations. Mutations
and cleanup target only captured test-owned processes. Previous failed attempts
remain diagnostic artifacts and cannot close the gate. Platform evidence is
fresh and separate; deferred B16 environments remain outside this scope.
