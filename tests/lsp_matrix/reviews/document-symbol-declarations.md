# Document symbol declaration review

B10.1 closes document-symbol S1 positive service/protocol and the UTF-16,
CRLF and encoded-URI protocol requirements. It does not certify the negative
recovery groups, remaining S4/S8/S9 groups, state cells or editor obligations.

The shared independently authored declaration fixture pins a whole ordered
19-root/33-node tree, with complete names, kinds, optional signature/type details,
full declaration/member ranges and identifier/header selection ranges. Source
identity is independently authored and checked at the service layer; the protocol
test compares the entire JSON-RPC result, including omitted optional fields.
Children and selections must fit within their containing ranges. Imports and
aliases, function parameters, local variables, shorthand and constructor uses
must not add outline declarations or steal ownership from a separate helper.

The declaration partition includes public/private consts with and without hints,
VM and extern state, public/private/zero/untyped/async functions, typed/defaulted
parameters, public/private structs, fields/defaults, public/private enums and
unit/tuple/record variants, tuple defaults, public/private traits, required and
default methods, inherent public/async methods and explicit trait overrides.
Same-named helper symbols remain in their own file.

Ranges enclose the symbol, while selectionRange identifies its name and stays
inside that range, as defined by the primary
[LSP document symbol contract](https://github.com/microsoft/language-server-protocol/blob/gh-pages/_specifications/lsp/3.17/language/documentSymbol.md).
The old implementation used identifier-only spans for fields, variants and
methods, and searched source text for names. Attribute strings containing the
same name exposed incorrect name selections. Outline extents now join existing
HIR owners to the already parsed CST by exact item/name offsets; metadata and
semantic name spans are unchanged. The impl selection is its actual header,
not an attribute string that happens to contain the rendered synthetic name.
The prior protocol projection copied byte columns directly. Both ranges now
convert recursively through the current document's UTF-16 index; invalid source
positions fail projection instead of producing unchecked wire coordinates.

Both layers run LF and CRLF. Service columns remain byte columns; protocol
columns are UTF-16. A required method has Chinese/non-BMP text before its name
on the same line: literal name goldens are line 27, UTF-16 39..43 and bytes 43..47.
Markers independently specify all remaining ranges, including field types,
default initializers, attributes, method signatures and bodies. The protocol
uses a real private root with Chinese, spaces and a percent sign, checks URL
encoding, opens through that URI and compares exact current-file results.
Imports-only and missing service documents have empty trees; broader negative
requirements remain unreviewed.

Current source validation requires focused service/protocol tests, relevant
Clippy and formatting, Node matrix self-tests, fresh installed VSIX/native proof
and strict B09 regression. Existing accepted snapshots and all original native
contracts remain fixed; macOS must collect independent fresh platform evidence.

The initial full native regression exposed an observation collision between an
unrelated tab tooltip and the keyboard language hover in invalid configuration
recovery. That observer now selects visible language widgets containing Markdown
or marker diagnostics. Real keyboard actions, complete hover content and all
prior proof contracts remain unchanged; a second language widget still fails.
