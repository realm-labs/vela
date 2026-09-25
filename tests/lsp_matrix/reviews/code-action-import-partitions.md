# S8 import code actions

B05.13 reviews source, schema and stdlib imports in the shared
`code-action-import-partitions` fixture. A source module exports a function,
const, state, struct, enum and trait. For each unresolved unqualified use, the
service and protocol return exactly one Quick Fix that inserts the corresponding
public import after the existing import block. The tests pin the target URI,
single edit, insertion range, title and replacement. They apply every returned
edit, compare the whole source with an independent expected insertion, and
verify that the selected unresolved-name diagnostic clears. A valid but unused
aliased import has exactly one removal action; applying it removes its whole
line and clears only that warning.

Valid source, source-backed schema and stdlib aliases yield no speculative
actions. A misspelled import, private import, absent module, private unqualified
name and name shared by two public modules return empty action sets. The tests
repeat under LF and CRLF. A Chinese and non-BMP prefix precedes representative
unqualified names; protocol assertions use exact UTF-16 edit coordinates and
versioned document changes in a percent-encoded workspace URI. Long-lived
publications after application agree with fresh servers at the same source.
