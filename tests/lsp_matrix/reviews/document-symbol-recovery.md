# Document symbol recovery review

B10.2 closes document-symbol S1 negative, S9 positive/negative at service and
protocol layers, plus protocol recovery and repeat states. Other syntax,
lifecycle, schema and installed-editor obligations remain open.

The shared independently authored 56-case fixture pins complete ordered trees:
names, kinds, optional details, full extents, actual selections, child ancestry
and canonical source identities. Healthy neighbors surround damaged items;
unclosed owners occur at EOF after both healthy declarations. A separate helper
has a same-named function and unrelated type; neither appears in the requested
file. Missing names and missing impl targets produce no guessed outline owners.
Named partial declarations retain current CST extents and available metadata.

Declaration failures include const name/value/type, VM/extern state name/type/
initializer policy, function names/parameter lists/parameter names/defaults/
return types/bodies, nameless struct/enum/trait owners, nameless fields/methods,
unclosed struct/enum tuple/trait/impl owners, field/tuple/required-method defaults,
inherent and trait impl targets and method defaults. Body recovery covers member,
named argument, constructor, pattern, type and expression failures, unclosed
bodies, unresolved and dynamic owners. Empty, trivia-only and stray-token files
have exact empty or healthy-neighbor trees; local names and malformed inner
syntax never become top-level symbols.

Each case runs damage -> clean repair -> same damage. Both layers check the
authored whole tree at every step against incremental and fresh instances, then
repeat queries three times. Protocol uses real didOpen/didChange messages and
whole JSON-RPC responses, with monotonically increasing versions and encoded
private-root URIs. The service joins existing HIR identities; the protocol
converts the same independently marked ranges to UTF-16. Literal prefix goldens
pin `before` at line 0, UTF-16 13..19 and bytes 17..23. No expected value comes
from a provider result. Existing declaration tests remain separate.

The initial red run exposed missing declaration diagnostics for required names,
bodies, type annotations and initializers/defaults while outline trees remained
recoverable. Focused declaration validation reports these missing children
without discarding lossless nodes or changing language/runtime semantics.
Named partial owners remain usable while declaration errors appear, clear on
repair and reappear on damage. Direct parser tests pin each added message,
owned source bounds, lossless text and valid counterparts.

A separate real stdio probe exposed later-identifier name theft: `fn () -> Ghost`
and malformed struct/enum/trait headers appeared as `Ghost` declarations.
The red whole-tree cases also reject punctuation-separated names and contextual
state paths. Header readers now accept only the first significant token after
the declaration keyword, stopping at punctuation or child nodes. Direct AST
tests cover use aliases as well. Another independently authored red case pins
the available `() -> Ghost` detail for `pub fn Broken() -> Ghost` without a
body; the parser now retains that return hint while diagnosing the missing body.

The complete accepted-batch regression exposed an overbroad initial diagnostic
rule: B03-B08 already require quiet recovery for unfinished member, argument,
constructor, pattern and expression input. Those existing assertions stay fixed.
The final validation preserves that policy; the new fixture explicitly expects
no E_PARSE for those partial expressions, while all whole-tree, ownership, range,
state and repeat assertions remain unchanged. Quiet partials have direct parser
regressions too. Missing closing delimiters retain their existing diagnostics.
An unfinished function parameter list keeps its existing delimiter diagnostic;
a missing body is diagnosed after a closed header, without downstream duplicates.
Compact annotation/initializer `>=` remains valid. The fixture's inherent impl
identity was corrected against the existing canonical module-qualified identity
contract, and named-call source uses the existing `=` grammar.

Validation requires focused service/protocol tests, full syntax checks, relevant
all-target Clippy and formatting, Node matrix self-tests, fresh Windows installed
VSIX/native regression and strict B09 acceptance. Prior accepted snapshots and
native contracts remain fixed. macOS requires independent fresh evidence;
B16 remains deferred.

The first complete native regression retains a baseline hover observation
failure: the correct `HostCell.value` code label and field detail coexist with
an unrelated tab path tooltip. Markdown content alone is insufficient to
identify the language widget. The observer now requires the same tokenized code
label it already checks, or marker diagnostics for diagnostic-only hovers.
Actual actions and complete expected label/paragraph/diagnostic facts stay fixed;
two real language widgets still fail. Collisions retain bounded DOM metadata
for diagnosis. A selected investigation omitted its preceding roots/workspace
creation route and failed before reaching the hover; that attempt is retained
separately and cannot be used for acceptance.
