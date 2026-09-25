# Installed diagnostic and code-action providers

B05.3 closes the two installed-editor smoke cells using the B05 shared method
typo fixture. The ordinary VSIX runner materializes its stripped source inside
the isolated workspace and opens it through the installed Vela extension. This
is current installed-package proof, separate from protocol and native-input
tests.

The diagnostic check compares the complete two-item VS Code diagnostic array:
code, error severity, exact message, marker-derived UTF-16 range and no related
locations. An unsaved editor edit repairs only `frist`; the full document equals
the independently marked expected source and only `foobar` remains published.

The code-action check requests actions at the marked typo and requires exactly
three ordered quick-fix titles. Each action has one edit in the correct URI,
with the exact marked range and replacement. Applying `first` through the VS
Code WorkspaceEdit API leaves the complete expected source dirty, clears only
the target diagnostic and yields no action at the repaired location. Both tests
revert and close their editor so later work begins from disk.

These provider checks do not certify visible Problems or lightbulb UI, keyboard
and pointer input, undo, dismissal or no-fix routes. Those remain UX07/UX08
Input/Render obligations. Other B05 syntax and state cells also remain open;
macOS needs independent current evidence when used, and B16 stays deferred.
