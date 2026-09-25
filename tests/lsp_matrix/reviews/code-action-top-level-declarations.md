# S1 top-level declaration code actions

B05.18 combines exact positive import-action evidence with the S1 declaration
fixture. The existing S8 import-action matrix exercises public function,
constant, state, struct, enum and trait declarations from a second module. It
pins each single-edit title, kind, target, range and replacement at service
byte positions and protocol UTF-16 positions, applies each edit to the whole
source, and verifies the selected unresolved-name error clears. Its private,
ambiguous and missing-owner cases prevent unsafe import suggestions.

The shared S1 fixture adds valid local declarations, seven duplicate errors,
three malformed `state` forms, and a legacy `global` declaration. Valid forms,
all duplicate name sites and all state parse-error ranges produce empty complete
action arrays under LF and CRLF. Manual repairs clear their diagnostics and
actions; protocol edits agree with a fresh server, and closing each dirty
document restores its disk diagnostics. The protocol runs in an encoded
Unicode/space/percent workspace and projects the first state error after a
Chinese and non-BMP prefix to UTF-16.

The legacy `global` diagnostic retains two migration examples as candidate
metadata. They contain placeholder names and values, so replacing only the
`global` token would leave the old declaration tail and produce malformed
source. They are therefore not emitted as Quick Fix edits. The service and
protocol tests assert both candidates remain visible in diagnostics while the
action array is empty at the `global` token.
