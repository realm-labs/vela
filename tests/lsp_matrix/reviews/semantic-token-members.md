# B06 S4 member and constructor token review

The `semantic-token-members` fixture independently marks every token in complete
423-token positive and 248-token negative streams. Expected text, classification,
modifiers and byte/UTF-16 coordinates are authored independently of providers.
Both layers load the same source files and static schema artifact.

| Partition | Exact expectations |
| --- | --- |
| Source members | Struct field declarations, reads, writes and compound writes; inherent and trait implementation methods, required trait calls and inherited defaults carry source ownership. Enum impl targets and methods on a constructed variant retain the enum owner. |
| Schema members | Host fields and writes, host methods and schema trait methods retain host/schema flags. A schema enum record payload read carries schema. Source members win over exact same-owner schema entries. |
| Constructors | Local and qualified source structs and schema host records pin owner paths, explicit field labels and value bindings. Explicit labels carry property ownership; shorthand expressions retain the source local use. Missing explicit fields receive no guessed ownership; an unresolved shorthand local remains unresolved. |
| Variants | Source and schema unit, tuple and record constructors and corresponding match patterns pin every owner and enumMember token. Record payload labels, explicit bindings, shorthand bindings, branch uses and source payload writes are exact. |
| Scope and absent facts | A second module declares the same struct and trait names. Its foreign fields, methods and defaults cannot leak to local receivers, while its qualified constructor and members resolve normally. Unknown members, Missing and Any receivers, a wrong qualified schema hint, missing variants and unknown constructor labels retain conservative classifications. |

The existing scenario drivers check full streams under LF/CRLF, apply actual
deltas and compare fresh analysis, repeat unchanged queries, and query every
line, token and empty token-end range. Protocol requests use encoded Unicode,
space and percent URIs; negative overlays are followed by close-to-disk
restoration with the original stream and result ID.

Exposed defects are fixed in focused token projection modules. Explicit record
labels no longer inherit an enclosing constructor's declaration classification.
Constructor and variant paths use exact syntax spans and module/import ownership.
Known source owners block schema member fallback; receiver owner matching never
strips a qualified namespace. Local hint fallback uses the existing scoped
analysis resolver. Trait defaults resolve in the impl module instead of picking
the first global same-named trait. Enum impl headers and variant method receivers
retain their enum type owner. These changes stay read-only and do not alter
runtime, parser or language semantics.

This evidence closes only the twelve S4 positive/negative full/delta/range cells
in the service and protocol layers. Other B06 cells still need their own proof;
macOS evidence remains independent and B16 remains deferred.
