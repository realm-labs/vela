# Imported const and state ownership

B04.33 advances the S8 positive and negative service/protocol paths for
references, highlight, prepare-rename and rename. The broad S8 cells remain
unreviewed pending the complete module/import partition audit.

The independent fixture has 22 queries across three owners. A public constant
is referenced through direct imports, qualified paths, an explicit alias, a
module alias and a closed consumer, but cannot be renamed because it is public.
A public `extern state` has the same import forms, including compound writes;
rename changes its declaration, import terminals and non-aliased uses while
retaining the explicit alias. Both layers require its hot-reload ABI warning.
A private constant remains renameable inside its own module; cross-module and
missing imports remain unresolved and cannot be prepared or renamed.

The two drivers compare complete reference sets with and without declarations,
same-document highlights, exact prepare results, full versioned edits and
negative outcomes. They apply the writable groups across files, parse the
result, requery and restore under LF/CRLF and Unicode UTF-16 coordinates. The
protocol fixture uses encoded paths and a closed consumer.

The first run exposed omitted `h::BASE_REWARD` references. The shared scoped
module-alias resolver now recognizes source functions, constants and states,
checks visibility and leaves type and variant paths with their existing owners.
Other S8 declaration kinds and UX05/UX06 remain open.
