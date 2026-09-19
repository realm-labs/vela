# Reference and rename coordinate slice

B04.01 owns the `utf16`, `crlf`, `encoded_uri` and `repeat` protocol cells for
references, highlights, prepare-rename and rename. It does not certify whole
syntax partitions or the native rename/reference UI routes.

The shared `reference-rename-coordinates.json` fixture enumerates 18 positions:
four parameter sites, three local sites, two shadow sites, five source-function
sites across three files, and four empty outcomes (unknown, literal, comment and
keyword). The function set contains declaration, import, bare and qualified calls.
Chinese and emoji precede queried or edited tokens. Both drivers run LF and CRLF;
the protocol workspace path contains Chinese, spaces and percent signs.

Markers define independent exact ranges. Every query is repeated and compared
against the complete reference set with and without declarations, active-document
highlights and their kinds, prepare range/placeholder, and every workspace edit.
The service additionally checks source/local symbol identities. Each of the four
owned groups is actually renamed, the complete resulting files are compared with
marked-source expectations, parsed, queried again and restored. The protocol
uses actual didChange notifications; an initially unopened consumer becomes an
overlay during edits and is closed after restoration. Both workspace edit forms
retain exact ranges/text; open versions come from notifications and closed files
carry null client versions.

This exposed whole-path reference ranges, omitted qualified rename uses, absent
import-site query targets, byte columns leaking into LSP results, and internal
disk versions leaking into client edits. Resolved HIR terminal segments now supply
reference/edit ranges; import terminal tokens resolve through their existing
declaration identity. The server projects all four response families using the
corresponding immutable document text and UTF-16 indexing. Rename projection
preserves risk annotations and assigns client versions only to open documents.

Further B04 work still includes semantic partitions, schema/import alias policy,
collision and lifecycle matrices, installed provider smoke and mandatory UX05/UX06
input/render evidence. B00-B03 snapshots remain fixed; fresh evidence must come
from one registered platform at a time. B16 stays deferred.
