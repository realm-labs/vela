# S3 type-position code actions

B05.19 reuses the shared S3 type-position fixture. Valid primitive, builtin
container, source struct/trait, `Any` and unknown hints have no speculative
actions. Eleven errors involving missing type arguments, keyability or tuple
intent likewise have no automatic replacement: choosing a new type would guess
the author's intent. A complete unsupported source-type argument list such as
`Cell<i64>` has one concrete action, **Remove unsupported type arguments**. Its
edit removes the entire `<i64>` list and leaves the source type and neighboring
syntax intact. The diagnostic keeps its `<` range and publishes the larger,
exact repair-hint range. An unclosed `Cell<i64` receives no removal edit.

Service tests assert the exact title, kind, target, byte edit range and empty
replacement. Protocol tests assert the complete versioned action object with
both workspace-edit forms and UTF-16 range in an encoded Unicode workspace.
Applying the action matches an independent whole-source oracle, clears only
the unsupported-generic diagnostic, leaves the other eleven errors, and makes
the action disappear. The resulting diagnostics and empty actions agree with a
fresh server. Applying all independent manual repairs clears every diagnostic
and action; closing the dirty protocol document restores the original twelve
errors and the one safe action. LF and CRLF run the same assertions.
