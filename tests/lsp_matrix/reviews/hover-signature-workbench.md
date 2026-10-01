# Native hover and signature interactions

B07.18 closes all ten UX10 Input/Render cells using the installed VSIX in the
registered Windows x64 or macOS ARM64 profile. Five independent contracts live
in `scripts/lsp-matrix/hover-signature-contracts.js`; `input-hover-signature`
(fixture ID `ux10`) supplies source, docs, types, ranges and caret expectations.
The external Playwright driver sends every accepted pointer/key/type action.
The extension-host bridge only sets up files/carets and observes editor state.

| Route | Independent observable proof |
|---|---|
| Pointer hover | Move onto the marked `combine` glyph after same-line CJK/non-BMP text. Read the exact qualified code-block label, complete type paragraph and Unicode docs from the visible hover. The actual hover highlight matches the marked glyph rectangle. Source, caret and disk remain exact. |
| Keyboard hover | Use the native Show Hover chord at that source position. Render the same complete docs/type as the pointer path; Escape clears the widget and restores usable editor focus. |
| Signature arguments | Physically type a call with automatic closing parentheses. Empty first argument selects `left`; named `right = 4` in position zero selects the defaulted `right`; named `left = 1` in position one selects `left`. Move back to the first and last arguments and explicitly invoke parameter hints. Check the complete visible signature, exactly active parameter, source, dirty state and caret at all five positions. Escape clears hints. |
| Dismiss hover | Open pointer hover, leave its target for blank editor space and verify clearing. Separately reopen by keyboard, check exact docs/type again, dismiss with Escape and check unchanged source/caret plus editor focus. |
| Unknown receiver | Invoke hover and parameter hints on `dynamic.combine(1)`, where `dynamic: Any`. A real completed server response for each method and this owned file must follow the physical invocation. Widgets stay absent after completion; source/caret/disk and focus remain exact. The known imported callable and an unrelated same-spelled String callable cannot supply invented member facts. |

Every route checks all three physical fixture files byte-for-byte, including the
decoy docs and signature. Screenshots, accessibility snapshots, typed receipts,
editor observations, client/server traces and installed package hashes are kept.
Readiness polls only observe; actions execute once (or their explicit bounded
key count), and each proof has the existing 45-second deadline. Negative checks
wait for real protocol completion before observing the absent UI.

Hover code blocks use VS Code's actual `.monaco-tokenized-source` DOM rather than
assuming a plain `pre/code`. Signature observations use the actual visible code
and active parameter span. Pointer placement comes from the independently marked
source and rendered text nodes, without Monaco/provider models. Windows uses
Ctrl+K Ctrl+I and Ctrl+Shift+Space; macOS uses Cmd+K Cmd+I and Cmd+Shift+Space.
Line movement uses Windows Home twice/End or macOS Cmd+Left twice/Cmd+Right.
Profile settings and display remain pinned. B16's extra environments are deferred;
changing machines requires fresh evidence on that machine, without aggregation.
