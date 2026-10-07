# Selection trivia review

B10.37 covers the complete S12 positive/negative service/protocol partition.
The literal corpus contains 63 documents and 458 ordered queries: 155 complete
chains and 303 point fallbacks, plus an actual empty vector. Chinese and non-BMP
text runs in LF/CRLF and shifted forms. Comments, doc comments, nested/multiline
block comments, shebangs, blank lines, mixed/tab indentation, top-level item
headers, imported paths, nested fields/methods/types and named call arguments
have explicit lexical kinds, complete token extents and CST owner ancestry.

Interior trivia returns a point without inventing syntax or semantic parents.
At trivia boundaries, significant tokens retain their complete syntax chain.
The final significant brace at EOF retains its chain, while trivia-only EOF
returns a point. Ordinary/multiline strings, bytes and characters containing
comment or whitespace lookalikes remain significant literal selections.
Equal-span Literal and TypeHint nodes remain required in the complete CST
ancestry even when omitted from the deduplicated selection range chain.

Malformed neighbors occur before and after healthy syntax in distinct damage,
repair and repeated-damage states. Unclosed bodies retain recovered owners;
unterminated and nested block comments require the exact lexical error code set.
Empty, whitespace-only, comment-only and shebang-only documents are separate
negative contexts. Every layer checks complete current/fresh/previous immutable
source and CST facts, main/helper/missing vectors repeated three times, actual
empty vectors, absent schema facts and unchanged query generation/parse/project/
HIR counts. Protocol checks typed complete envelopes, real encoded private roots,
unchanged physical disk and atomic invalid UTF16/outside-document vectors.

The expected values are authored from literal bounds and primary lexer/CST rules.
The first function-name end query incorrectly expected the left name while the
adjacent right token was `(`; whitespace now makes the intended trivia boundary
explicit. The first additional significant-EOF case incorrectly inherited the
generic EOF point expectation; it now explicitly authors the final brace's whole
ancestry. Original red vectors are preserved as author errors, not product bugs.
Param and Argument spans include their delimiter-contained trailing trivia;
expression nodes trim that trivia. CRLF line comments own CR but exclude LF.
Whitespace bounds use only adjacent literal whitespace and authored comment
extents to model merged tokens; they do not inspect provider output.

Shebang transforms retain byte-zero ownership. For a shebang without a final
newline, the shifted form adds its separator inside the shebang token before
adding the two comment lines. Four independent Node tests pin partition counts,
literal byte/UTF16 geometry, equal spans, lexical policy, duplicate order,
complete containment and points/empty vectors. This child adds no product policy.

Existing accepted scopes, native/installed contracts and independent macOS
history stay intact. Fresh same-source Windows native50, installed VSIX25,
automatic whole matrix and strict prior B09 receipts are required before the
child checkpoint. B10 remains open for installed selection and UX11/UX12.
