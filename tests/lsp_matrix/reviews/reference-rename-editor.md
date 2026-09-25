# Reference and rename installed-editor smoke

B04.40 closes the four installed-editor smoke requirements for references,
document highlights, prepare rename and rename. These provider checks run through
the installed VSIX and its bundled server in the isolated Unicode workspace.
The existing UX05 and UX06 Input/Render routes separately check actual F2,
Shift+F12, pointer and caret behavior in the rendered workbench.

The independent UX06 fixture marks a public declaration, an imported name,
a caller and a closed-file qualified caller. The VS Code reference command must
return exactly those four URI/range pairs. Three highlight queries distinguish
local declaration Text, compound Write and Read ranges, then switch to a
same-named shadow and require only its two ranges.

The UX05 fixture marks the corresponding rename sites. Prepare rename returns
the exact call token and placeholder but rejects a keyword. Rename rejects a
colliding function name, returns exactly four Unicode cross-file edits for an
allowed name, and applies them. Every resulting document must match independently
substituted source, including the untouched same-named shadow.

These smoke requirements do not claim remaining syntax or lifecycle cells;
those retain their own service and protocol acceptance obligations.
The downloaded VS Code runtime under the repository root is ignored so it
cannot enter the source fingerprint used by editor evidence.
