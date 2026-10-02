"use strict";
const assert = require("node:assert/strict");
const { parseMarkers } = require("./fixtures");

// Authored source markers and rows only. VS Code uses zero-based SymbolKind;
// absent LSP details/children become an empty string/list in DocumentSymbol.
function document(source, crlf = false) {
  return parseMarkers(crlf ? source.replaceAll("\n", "\r\n") : source);
}

function expected(doc, rows) {
  const range = name => {
    const { start, end } = doc.markers[name];
    return [start.line, start.character, end.line, end.character];
  };
  return rows.map(row => ({
    name: row.name, kind: row.protocolKind - 1, detail: row.detail ?? "",
    range: range(row.range), selectionRange: range(row.selection),
    children: expected(doc, row.children),
  }));
}

function count(rows) {
  return rows.reduce((total, row) => total + 1 + count(row.children), 0);
}

function assertAncestry(rows, parent) {
  const point = (range, end) => range.slice(end ? 2 : 0, end ? 4 : 2);
  const before = (a, b) => a[0] < b[0] || (a[0] === b[0] && a[1] <= b[1]);
  for (const row of rows) {
    assert(before(point(row.range), point(row.selectionRange)));
    assert(before(point(row.selectionRange, true), point(row.range, true)));
    assert.notDeepEqual(point(row.selectionRange), point(row.selectionRange, true));
    if (parent) {
      assert(before(point(parent), point(row.range)));
      assert(before(point(row.range, true), point(parent, true)));
    }
    assertAncestry(row.children, row.range);
  }
}

// Metadata-only schema setup for the installed client. Source-backed spans are
// tested separately in the service/protocol fixtures, not guessed from IDs here.
function metadataArtifact(schema) {
  const facts = structuredClone(schema);
  for (const entries of Object.values(facts)) for (const entry of entries) delete entry.sourceSpan;
  return { formatVersion: 1, facts };
}

module.exports = { document, expected, count, assertAncestry, metadataArtifact };
