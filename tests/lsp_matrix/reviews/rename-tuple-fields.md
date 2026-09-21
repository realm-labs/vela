# Tuple variant parameter ownership

B04.10 advances the service/protocol positive and negative partitions of S4, S5
and S6 for references, highlight, prepare-rename and rename (48 requirement IDs
of the form `<feature>/syntax/<S4|S5|S6>/<positive|negative>/<service|protocol>`).
Broad cells remain unreviewed; signature-only and schema callable ownership,
semantic/lifecycle partitions and UX05/UX06 still need independent proof.

The three-file fixture has 20 query points and five groups: tuple fields on a
variant, its sibling, a same-named enum in another module, a caller parameter and
a positional pattern binding. It covers enum/variant aliases, qualified calls,
closed-file labels, closures, missing argument values and positional constructors.
Unknown labels/owners, string text, wrong constructor forms and a local binding
shadowing the imported variant alias must produce empty results.

Both drivers repeat exact reference sets, kinds, highlights, definitions, prepare
targets and complete edits; apply all five groups, compare whole files, parse,
requery and restore. The field cannot capture its sibling parameter or an unknown
label owned by its variant; another variant can use that spelling independently.
Positional pattern locals remain separate from the variant's parameter name.
Protocol projects LF/CRLF, UTF-16, encoded Unicode paths, both edit forms,
didChange and open/closed versions. Service checks canonical symbols.

References and rename consume shared source-field ownership. Tuple call labels
resolve through the existing definition/signature target, preserving local
shadowing and aliases. Owner identity distinguishes tuple and record syntax so
an invalid record constructor cannot contribute a tuple parameter reference.
This changes analysis only, without changing parser or runtime semantics.

Original B00-B03 snapshots remain fixed. Current Windows evidence stays separate
from fresh macOS evidence when used. B16 remains deferred.
