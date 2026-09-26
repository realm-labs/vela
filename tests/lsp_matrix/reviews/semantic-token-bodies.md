# B06 S2 body token review

The independent `semantic-token-bodies` fixture specifies every token's text,
type and complete modifier list: 325 positive tokens and 238 negative tokens.
Marker spans or cursor positions and literal token text determine coordinates;
the oracle does not invoke the lexer, token classifier or provider. Cursor
markers avoid the fixture delimiter ambiguity around literal opening brackets.
The shared oracle separately checks their byte and UTF-16 lengths after Unicode.

| S2 body partition | Positive and negative assertions |
| --- | --- |
| Locals and explicit hints | `let total: i64 = seed` pins local declaration, builtin hint and parameter use. Initializers of `let seed = seed` refer to the outer parameter; uses in the block refer to the local and later uses return to the parameter. A local named `apply` shadows the imported function and is a variable, while later calls remain source functions. |
| Assignments | Ordinary assignment and all five compound writes retain source-variable classification and exact operator tokens. Missing ordinary and compound write targets carry unresolved-reference classification, without declaration/source flags. |
| Returns and nested blocks | Function, inherent method, default trait method and trait implementation bodies retain exact local/parameter use kinds and control-flow keyword modifiers. References to a closed block's local remain unresolved. |
| Branches and guarded match | An `if/else` result, guarded arm binding, wildcard and selected result keep their exact independent types/modifiers. An arm binding cannot classify another arm's use or a use after the match. |
| Loops and exits | Array iterator literals, loop bindings, body writes, `continue` and `break` are fully decoded. Neither the iterator binding nor a body local remains resolved after the loop. |
| Lambdas, captures and callbacks | Typed lambda parameters, closure writes to outer locals, explicit lambda returns, expression callbacks and nested captured locals retain complete streams. The imported callback function is defined in an unopened dependency. Captured outer parameters remain parameters; shadow locals remain variables. Parameters cannot escape their lambda or callback and unresolved captures gain no source facts. |

The shared service and protocol scenario drivers check positive, negative and
restored streams in LF and CRLF. Each state pins the entire stream, applies the
actual delta to the previous data, checks the result ID, repeats unchanged
queries and compares with a fresh database/server. The protocol driver opens an
encoded Unicode/space/percent workspace URI, makes the negative unsaved edit,
then closes it to restore the original disk stream and ID.

Range assertions select every complete line and every individual token and
require an empty range at every token end. Expected subsets come from the
manually authored oracle, so extra neighbors, leaked owners and incorrect
coordinates fail even when the full/delta paths share an implementation bug.

These tests close the twelve S2 positive/negative service/protocol syntax cells
for full, delta and range. The S1 tests retain their original identities and all
assertions while reusing the same scenario driver. This review does not certify
remaining syntax partitions, lifecycle cells or native UX09 render/input routes.
No language/runtime or protocol product behavior changed in this child.
