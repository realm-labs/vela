# Workspace symbol member/import ownership review

B10.7 closes only S4/S8 positive and negative service/protocol requirements.
Type-hint partitions, malformed recovery, lifecycle and installed/native picker
proof remain open; previous declaration/query/range proof remains unchanged.

Eight independently authored sources contain fields, source/trait/inherent and
imported/host-target impls, unit/tuple/record enum variants, constructors,
shorthand fields, member writes, aliased and qualified calls, public/private
imports, nested modules, duplicate short names, stdlib uses, unresolved and
dynamic members. Whole workspace output has 41 source rows: eight files, eight
modules and 25 top-level declarations. Source members and use sites remain inside
their owners and never become additional global workspace declarations.

Static schema metadata adds 19 authored rows: four types, one trait, two
functions, five fields, three methods, one trait method and three variants.
Ten metadata entries have independently marked source spans; nine are metadata
only. All schema workspace locations retain the existing URI-only vela-schema:
contract, including known source spans. Service identities distinguish Schema
from Source and use canonical member identities; source ranges retain complete
declaration/file/module extents. Exact same-name source::Widget Class/Struct
and source::make Function rows retain their separate details and ownership.

Forty-nine explicit ordered query result sets pin all rows, type/function
collisions, short/qualified names, nested modules, source private declarations,
schema fields/methods/trait methods and enum variants. Aliases, locals, parameters,
constructors, field labels, source methods/variants, builtin imports, unknown
owners, dynamic uses and metadata signature/type details cannot invent names.
An impl named Row can match Row, but does not borrow the imported Widget name.

Every query repeats three times at LF/CRLF and with/without two Unicode prefix
lines. Service compares complete results against fresh immutable databases and
checks display parts, source bytes and unchanged source/schema symbol sets.
Protocol uses real typed TestServer dispatch with static schema binding and
encoded private Chinese/space/percent roots, compares complete JSON-RPC envelopes
and all omitted optional fields, and verifies unchanged physical source/schema
bytes. Marker goldens pin source Widget start UTF-16 column 10 versus byte 14;
whole file/module ranges include trailing source text. Three independent Node
tests pin partitions, literal memberships/exclusions and schema/Unicode goldens.

Acceptance requires focused Rust tests, relevant all-target Clippy, fmt, complete
Node infrastructure and fresh frozen Windows VSIX23/native50 plus automatic full
audit and strict B09 refresh. Preserve prior accepted/completed records and all
50 native contracts. macOS needs independent fresh evidence; B16 stays deferred.
No test query executes scripts, host functions or live host state.
