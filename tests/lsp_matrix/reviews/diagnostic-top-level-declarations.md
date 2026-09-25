# S1 top-level declaration diagnostics

B05.17 uses a shared three-source fixture for the S1 diagnostic surface. The
valid file combines an imported public function, a public constant, contextual
`state`, `pub extern state`, a public struct, unit/tuple/record enum variants, a
trait with interface and default methods, inherent and trait impls, and a public
function with a default parameter. Service and protocol tests assert the entire
diagnostic set is empty under LF and CRLF. The existing S8 import tests cover
public, private, missing and unused `use` declarations and cross-module changes.

The invalid files exercise duplicate function parameters, struct fields, enum
variants and record-variant fields, trait methods and inherent-impl methods,
plus the separate cross-impl method collision. They also exercise a missing
`state` initializer, an illegal `extern state` initializer and a missing state
type. Service tests pin the complete ordered diagnostic arrays with code,
message, severity, source byte range, both related labels when present, and
empty candidate/repair metadata. Protocol tests pin complete UTF-16
publications in a workspace URI containing Unicode, spaces and a percent sign;
the first invalid declaration and state error follow a Chinese/non-BMP prefix.

Each duplicate name has an independent fixture repair and all state forms have
a corrected source. The repaired sources parse and produce no diagnostics in
the live service and a fresh service. Dirty protocol edits clear both files,
fresh servers agree, and closing either document restores its original disk
diagnostics. This partition concerns diagnostic behavior for the supported S1
forms; type-hint diagnostics are reviewed under S3 and import diagnostics under
S8.
