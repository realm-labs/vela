# Selection declaration ancestry

B10.25 closes only S1 positive/negative service/protocol. S2 body and the other
selection partitions, installed selection smoke and native UX11/UX12 remain open.

Thirty-eight independently marked cases author 132 inputs: 122 complete ancestor
chains and ten point-only responses. Thirty-four documents have positive chains,
three have only points and one has a genuinely empty query vector. Four LF/CRLF
and Unicode-shift forms give 152 whole vectors and 528 inputs. Complete expected
responses come from authored markers before querying a provider; actual stdio
responses match every vector and publish no lexical or parse diagnostics.

The primary CST routines in `cst_parser/cst_items.rs` own declaration, parameter,
field, variant, method, attribute and type spans. The selection policy collects
the significant token and every nonempty CST ancestor, deduplicating identical
byte spans. Empty struct field lists still own delimiters; unit variant and self
parameter spans collapse with their identical token bounds. Trait/impl braces
do not invent an intermediate method-list or block ancestor. A method body has
its own Block; its signature and parameter defaults remain under the method.

Cover public/private/async functions, parameter defaults and sibling separation,
attributes, qualified/tuple/allowed builtin hints and return annotations; const,
state and extern-state declarations; structure fields and defaults; unit/tuple/
record enum variants; required/default trait methods, inherent/trait impl methods
and self parameters. Neighbor declarations with equal-spelled tokens cannot
borrow ranges. Public imports retain their own path and alias chains. Header,
body and standalone comments, whitespace, empty files and EOF after newline own
only their requested zero-width point. Unordered duplicate query positions retain
their original response order and multiplicity. No declaration, field, variant,
parameter or callable semantic lookup is needed to construct syntax ancestry.

First function name spans UTF16 15..23 versus bytes 19..27; its item spans 8..89
versus 12..93. The second parameter retains its leading space, 42..62 versus
46..66, inside the 23..63 versus 27..67 parameter list. The full-file ancestor
starts at zero even after two inserted Unicode lines. An independently marked
helper constant owns name 14..20 versus 18..24 and item 8..26 versus 12..30.
Missing documents keep only the requested zero point or an empty result vector.

Both drivers repeat complete main/helper/missing and empty vectors three times
through disk, every dirty variant and close restoration. Independently fresh
databases/coordinators and every retained immutable snapshot must match their
own authored source, CST and query model. Source records, quiet parse facts,
absent schema, generation and parse/project/HIR counters stay exact on queries.
Protocol dispatch uses real initialize/open/change/close, private physical
encoded roots, unchanged main/helper disk bytes and complete typed envelopes.
Invalid half-character, line/document and mixed vectors retain exact atomic
errors without database changes.

Extract only the unchanged generic marker projection from the import oracle into
shared test modules. Preserve all earlier import assertions and fixture bytes;
the new helper and expected chains are independent. Node checks pin complete
counts/membership, literal UTF16/byte goldens, strict containment, dedup and
point/empty cardinality. An initial test generator placed a marker directly after
an attribute's literal opening bracket, confusing the marker grammar; add a
valid attribute space before the marker and preserve the failed generator. This
was a fixture-authoring error, not a parser or provider defect. No production,
language/runtime/capability change or ignored test is introduced. Platform
audits remain independently fresh and B16 remains deferred.
