"use strict";
const assert = require("node:assert/strict");
const { document, metadataArtifact } = require("./document-symbol-oracle");

// Expected rows come only from authored IDs, names and source markers. Metadata
// intentionally has no source binding in this installed-client smoke corpus.
function expected(spec, documents, uri, query) {
  const rows = new Map(spec.oracle.symbols.map(row => [row.id, row]));
  return query.symbols.map(id => {
    const row = rows.get(id); assert(row, `authored workspace row ${id}`);
    const location = row.file ? { uri: uri(row.file), range: (() => {
      const marker = documents[row.file].markers[row.range]; assert(marker);
      const point = p => ({ line: p.line, character: p.character });
      return { start: point(marker.start), end: point(marker.end) };
    })() } : { uri: "vela-schema:" };
    return { name: row.name, kind: row.protocolKind,
      ...(row.container === null ? {} : { containerName: row.container }), location,
      ...(row.detail === null ? {} : { data: { detail: row.detail } }) };
  });
}

function editorExpected(rows) {
  return rows.map(row => ({ name: row.name, kind: row.kind - 1,
    containerName: row.containerName ?? "", uri: row.location.uri,
    // The installed language client represents a URI-only WorkspaceSymbol with
    // a zero placeholder Location. The complete wire assertion below retains
    // its actual absent range, so this cannot claim a metadata source jump.
    range: row.location.range ? [row.location.range.start.line, row.location.range.start.character,
      row.location.range.end.line, row.location.range.end.character] : [0, 0, 0, 0] }));
}

// The public command may order provider entries independently of LSP output.
// Compare complete multisets including duplicate source/schema names. Never
// filter providers, deduplicate rows or infer expected entries from responses.
function ordered(rows) {
  return rows.toSorted((a, b) => {
    const x = JSON.stringify(a), y = JSON.stringify(b); return x < y ? -1 : x > y ? 1 : 0;
  });
}
function normalizeWire(rows, normalizeUri) {
  assert(Array.isArray(rows), "wire workspace symbols are an explicit complete array");
  return rows.map(row => ({ ...row, location: { ...row.location, uri: normalizeUri(row.location.uri) } }));
}
module.exports = { document, metadataArtifact, expected, editorExpected, ordered, normalizeWire };
