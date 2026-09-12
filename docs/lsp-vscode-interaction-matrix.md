# VS Code User Interaction Acceptance Matrix

Status: feature interaction acceptance remains pending. B01 now supplies an
installed-workbench input/render driver and strict local evidence validation. The current suite
has nine installed-VSIX scenarios and invokes providers or editor commands. Its
legacy F12 scenario invokes a command. The separate local driver now sends
actual F12/back and palette input for UX02, with exact editor state and passive
request/response checks. UX03 modifier-click and Peek routes have local
input/render proof; the checkpoint records the platform and revision, while
the remaining local families stay pending. The current catalog has 27 feature-level `editor/smoke` cells.
Neither those cells nor the 1327 initial requirements certify this matrix.

This document extends [the strategy](lsp-test-strategy.md) and is executed through
[execution plan](lsp-test-execution-plan.md). Scenario IDs remain stable.
UX01-UX18 and UX21 are required on the selected registered local development profile;
Windows/macOS runs keep separate evidence while sharing batch progress.
B00 expands their routes and negative cases into explicit obligations. B16-owned
UX19, UX20, UX22, UX23 and UX24 remain deferred follow-up families, without detailed
environment expansion in this goal. Local rows start pending. Existing tests may
supply evidence only for the exact assertions and interaction level they exercise.

## Evidence Levels

| Level | Action and observation | What it proves |
|---|---|---|
| Provider | Call a registered VS Code provider and assert its exact result. | Extension activation, client/server integration and response conversion. |
| Command | Invoke an editor command and observe document, selection or workbench state. | The command produces the expected editor effect. |
| Input | Send keyboard or pointer input through the running workbench, observe focus/widget state, then assert the resulting document and selection. | The user can reach and complete the intended interaction. |
| Render | Inspect visible widgets, accessible labels and rendered editor output against independent expectations. | The result is visible, readable and associated with the correct source. |

Input and Render may share a run, but do not replace exact semantic assertions.
A provider call cannot satisfy Command, Input or Render; a command invocation
cannot prove a keybinding or mouse route. APIs may set up fixtures and inspect
results, but must not perform the action under acceptance. Accepting a completion
must use the suggestion widget, not apply the provider's edit directly.

## Scenario Families And Scope

Each row names its owning batch. B01 supplies shared input/render infrastructure;
B16 is deferred environment work without changing feature ownership. Explicit
alternative routes in each current local row require evidence, not a choice of
one route. Local default keybindings and visible-widget assertions remain required
even though the separate remap and rendering-variant families are deferred.

| ID | Owner | User action and minimum evidence | Exact outcome and negative/recovery case |
|---|---|---|---|
| UX01 | B08 | Install the actual VSIX into an isolated profile; open a Vela file through the workbench. Input + Command. | Correct language mode, installed extension and bundled server activate within a deadline; the first query works. An unrelated file must not be associated with Vela. |
| UX02 | B02 | Use the default definition keybinding, then navigate back; use the command palette for declaration and type definition. Input + Command. | Correct file, selected identifier and UTF-16 range; back restores the original location. Cover unopened targets and dirty Unicode source; unknown targets must not jump elsewhere. |
| UX03 | B02 | Use modifier-click navigation and the Peek Definition context-menu route. Input + Render. | Correct peek content, target and selection; following the target opens the right file. Dismissal restores focus without editing; unknown targets follow the documented empty behavior. |
| UX04 | B03 | Type a completion trigger; move through suggestions, inspect resolved details, accept using Enter and Tab separately; dismiss using Escape. Input + Render. | Correct candidate/docs, exact inserted/replaced text and caret. Undo restores previous text; dismissal makes no edit. Cover a mid-token prefix and Unicode. |
| UX05 | B04 | Open Rename with its default keybinding, enter a name and confirm; separately cancel and submit an invalid/colliding name. Input + Render. | Exact references change across open and unopened files; shadows remain intact. Undo/redo follows the editor's workspace-undo policy. Cancel/rejection produces no partial edit. |
| UX06 | B04 | Open references, select results, and move the caret between identifiers. Input + Render. | Panel entries and destinations match the complete expected set; visible highlights follow the configured read/write styles and exclude shadowed symbols. |
| UX07 | B05 | Type an error, open Problems, select its entry, then repair the source. Input + Render. | Expected code, severity, message and location appear; the correct span is marked. Repair clears the problem and decoration without saving; unrelated errors remain. |
| UX08 | B05 | Open Quick Fix with its default shortcut and through the lightbulb menu, select a fix, then undo. Input + Render. | Correct action and exact source change; target diagnostic clears and unrelated source remains. Dismissal and locations without fixes make no edit. |
| UX09 | B06 | Edit a semantic-token fixture, scroll out and back, and toggle semantic highlighting. Input + Render. | Correct spans receive the theme's intended semantic styles; multiline Unicode edits leave no stale coloring. Full/range/delta equivalence also has provider/protocol proof. |
| UX10 | B07 | Hover with the pointer and invoke hover by keyboard; type a call and move between arguments. Input + Render. | Visible docs/type and active parameter match the source and named/defaulted argument policy. Leaving/dismissing the target clears the widget; unknown receivers expose no invented facts. |
| UX11 | B10 | Select symbols from Outline and document/workspace symbol pickers. Input + Render. | Correct names, kinds and parent structure; selection opens the exact range, including unopened files. No-match queries show no unrelated result. |
| UX12 | B10 | Fold/unfold using gutter controls and keyboard commands; expand/shrink selection by keyboard. Input + Render. | Correct lines hide/reappear, selection follows expected nesting, and text is unchanged. Include malformed neighbors and Unicode boundaries. |
| UX13 | B11 | Open Call Hierarchy, switch incoming/outgoing views, expand an edge and follow it. Input + Render. | Visible edges and navigation match expected ownership and call-site ranges; unknown targets produce no invented edge. |
| UX14 | B12 | Format a document and selection through the workbench; type a supported formatting trigger; undo. Input + Command. | Exact text/caret, selection containment and undo behavior; reformatting is idempotent. Trigger-disabled settings cause no on-type edit. |
| UX15 | B13 | Enable/disable inlay hints, edit a call and scroll it into view. Input + Render. | Correct labels/positions refresh after edits; explicit or unknown facts follow suppression rules. Turning hints off removes them without changing source. |
| UX16 | B15 | Edit without saving, switch files, save, close/reopen, and undo/redo through the workbench. Input + Command. | Navigation, completion and diagnostics agree with current buffers or restored disk and fresh-workspace expectations. Include rapid edits and syntax-error recovery. |
| UX17 | B09 | Create/rename/delete dependencies in Explorer, change roots/settings and replace a static host schema. Input + Command. | Queries use current files/configuration/schema and correct workspace ownership. Missing/invalid inputs follow diagnostic/recovery policy without stale targets. |
| UX18 | B08 | Stop the test-owned server during a session; inspect the failure indication, then reload the window through the workbench. Command + Input + Render. | Bounded request failure, no duplicate servers, observable failure, and restored queries with recoverable edits intact. Test invalid server paths separately. Current policy does not automatically restart a closed connection. |
| UX19 | B16 | Install a pinned prior VSIX, set preferences, upgrade to the candidate, and reload. Input + Command. | Candidate extension/binary load, preferences survive, and navigation/completion/diagnostics work. Disable/re-enable and uninstall/reinstall leave no stale server process. |
| UX20 | B16 | Repeat navigation, completion, rename and formatting with pinned companion extensions and a controlled conflicting-provider fixture. Input + Render. | Contributions remain identifiable, settings/provider selection follow declared policy, and Vela never double-applies edits or corrupts documents. Disabling the fixture restores the isolated baseline. |
| UX21 | B09 | Open an untrusted workspace, inspect supported trust behavior, grant trust, and reload if required. Input + Render. | Activation/configured-executable behavior matches declared trust policy; transition restores permitted features. This lane must not use the current runner's trust-disable flag. |
| UX22 | B16 | Open Remote SSH, WSL and Dev Containers workspaces and run the remote core set below. Input + Command. | Extension/server use the intended host and platform binary and preserve remote URI identity. Disconnect/reconnect yields bounded failure and usable recovery without local/remote path confusion. |
| UX23 | B16 | Exercise default keybindings, then an explicit remap; move focus between editor and other widgets. Input. | Correct dispatch in the intended context; remap works and the previous mapping follows configured policy. Record OS, keyboard layout and resolved keybinding. |
| UX24 | B16 | Inspect completion, hover, diagnostic, rename and hint views under light, dark and high-contrast themes, two zoom levels, and keyboard-only navigation. Input + Render. | Text, focus, labels and selection remain readable/reachable; no clipped critical controls or misplaced decorations. Use pinned rendering baselines and semantic assertions together. |

Host schemas are static metadata. No scenario executes Vela code or accesses
live application state. Failure injection affects only test-owned processes and
profiles. UX18 tests the existing recovery policy, not a new automatic restart
feature. Review the exact supported policy before writing each oracle.

## Current Local Acceptance

Use the available local OS and one exact supported VS Code version, recorded
with architecture, display backend, locale, keyboard layout, theme, zoom and
settings. All UX01-UX18 and UX21 routes and negative/recovery cases are mandatory
on this profile. Unicode, LF/CRLF, dirty buffers, disk lifecycle and schema/config
changes remain functional coverage requirements. B19 closes these local workflows
with the strategy's semantic, stateful, generated and scale gates.

Additional environments, versions and configuration sweeps are deferred. Their
availability is not a prerequisite for B00/B01 or local completion. Preserve
existing CI; adding multi-environment lanes or scheduling is later B16 work.

## Deferred B16 Environment And Scheduling Contract

The following scope is retained for later execution and does not gate B19's
current local acceptance. Activate and expand it in a separate follow-up.

The local release matrix has six profiles: Windows, Linux and macOS, each on
minimum supported and current stable VS Code. Resolve labels to exact versions
at run start; record architecture, display backend, locale, keyboard layout,
theme, zoom and settings. All scenarios except UX22 require local evidence in
each profile. Each named route/variant is mandatory; unlisted combinations may
use constrained pairwise selection rather than a full Cartesian expansion.

Required remote profiles use current stable VS Code: Linux desktop to a pinned
Linux SSH target, Windows desktop to a pinned WSL Linux distribution, and Linux
desktop to a pinned development container. Each runs UX01, UX02, UX04, UX05,
UX07, UX16, UX17 and UX18 remotely, plus UX22 disconnect/reconnect. Record both
desktop and remote extension-host versions, platform/architecture, authority,
fixture image identity and server hash. Local evidence cannot satisfy remote
requirements. Missing infrastructure remains pending acceptance.

B16 pins companion extensions, versions, settings and conflict-fixture behavior
before execution. UX20 certifies that set, not all marketplace extensions.
UX19 uses the previous released artifact; if none exists, build a preserved
baseline revision and record the substitution and distinct package versions.
Two identical packages cannot prove an upgrade.

| Lane | Required work |
|---|---|
| Pull request | Existing Rust/provider gates and installed command tests; Input smoke for UX01, UX02, UX04, UX05, UX07 and UX08 on Windows/Linux stable. Scenario, driver, packaging or evidence changes also run affected scenarios. |
| Nightly | Complete six-profile local suite, remote core suite, upgrade/coexistence checks and rendering variants. Preserve failures even if a later retry succeeds. |
| Future cross-environment release | Every required scenario/route/profile tuple passes for the candidate revision/artifacts, together with all strategy gates. Reuse nightly evidence only when all relevant identities match. |

These lanes are deferred B16 work. Current CI runs the seven-scenario suite on
Windows/Linux stable and Linux 1.90.0; it has no workbench input or remote lane.

## Driver, Evidence And Completion Rules

B01 establishes a separate workbench input/render driver using the installed
VSIX, isolated profiles and shared marker fixtures. Prove one keyboard action,
pointer action, visible-widget assertion and final-document assertion before
relying on the driver. It also implements and validates the local result format
and provenance below so B02 can submit Input/Render proof to the scoped gate.
The pinned local command is `npm --prefix editors/vscode run test:input`. Its
shared-fixture demonstration uses Playwright/CDP keyboard and pointer events,
visible accessibility selectors and exact extension-host document observations.
Feature route coverage remains the owning batches’ work. A child may use
`npm --prefix editors/vscode run test:input -- --proof ux03-modifier-click`
to collect that route with the driver/UX02 prerequisites. The default runs every
implemented route, and strict acceptance still requires all owned obligations;
a scoped run never certifies omitted native Peek routes.

Use stable accessibility/automation selectors, explicit focus checks and bounded
waits for state transitions instead of fixed sleeps or unbounded polling.
Readiness checks must not trigger the action under acceptance. Screenshots aid
diagnosis; an arbitrary screenshot alone cannot prove exact text/ranges/edits.
For canvas content, combine controlled image comparisons with independent source
expectations. Record display/font inputs and review baseline changes; never
auto-approve changed screenshots to pass a failure. Unsupported selectors or
rendering backends leave a route pending/failed, not downgraded to provider proof.

B00 registers every local scenario's owner, required routes, evidence levels,
fixture, positive/negative assertions and local profile. Use stable IDs such as
`vscode/UX04/accept-tab/input/local`; freeze the actual format in the versioned
manifest and bind `local` to the exact recorded profile. Register B16-owned
families separately as deferred; their tuple expansion waits for B16. Local
families may split into children without dropping routes.
Add negative self-tests for missing scenarios/variants, wrong evidence levels,
duplicate owners, stale artifacts and skipped actions.

Results record scenario/route ID, evidence level, driver/fixture hashes, VSIX and
server hashes, source identity, exact environment, actual actions, expected and
observed outcomes, timing and pass/fail/skip state. Preserve action traces,
failure screenshots, extension-host/server logs and applicable protocol traces.
Provenance includes driver, packaging and configuration inputs; B16 later adds
companion-extension inputs and multi-profile aggregation. The current format does
not cover all required local inputs yet.

Feature batches close their interaction requirements on the recorded local
development profile alongside provider proof. B19 requires every local route
plus all local strategy gates; B16 remains explicitly deferred, not accepted.
A `feature/editor/smoke` pass cannot satisfy an Input route or the local matrix.
Unavailable required local tooling, flaky results, unexpected local skips and
missing local artifacts remain acceptance gaps. Deferred environment infrastructure
does not block this goal. Manual testing may supply new regressions; it is not
the required evidence source for these automated scenarios.
