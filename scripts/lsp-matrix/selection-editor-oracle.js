"use strict";
const assert = require("node:assert/strict");
const selection = require("./selection-trivia-oracle");
const cmp = (a, b) => a.line - b.line || a.character - b.character;
const same = (a, b) => cmp(a.start, b.start) === 0 && cmp(a.end, b.end) === 0;
const contains = (a, b) => cmp(a.start, b.start) <= 0 && cmp(b.end, a.end) <= 0;
const point = (line, character) => ({ line, character });
const range = (line, start, end) => ({ start: point(line, start), end: point(line, end) });
function flatten(row) { const rows = []; for (; row; row = row.parent) rows.push(row.range); return rows; }
function chain(ranges) {
  return ranges.reduceRight((parent, r) => parent ? { range: r, parent } : { range: r }, undefined);
}
function expected(doc, queries, protocol = true) {
  return selection.expected(doc, queries, protocol).map((parent, index) => {
    const query = queries[index];
    if (query.token !== "[") return parent;
    assert(query.chain.length, "bracket must have independently marked parents");
    const marker = doc.markers[query.position].start;
    assert.equal(Buffer.from(doc.text)[marker.byte], 91, "authored point must select '['");
    const start = selection.point(doc, marker, protocol);
    return { range: { start, end: { line: start.line, character: start.character + 1 } }, parent };
  });
}

// Vela declares no wordPattern. The pinned SDK uses the default separators.
// These are literal-text/UTF16 rules, independent of any live provider/model.
function wordAt(text, column) {
  const separators = "`~!@#$%^&*()-=+[{]}\\|;:'\",.<>/?";
  const pattern = new RegExp("(-?\\d*\\.\\d\\w*)|([^" + [...separators].map(c => "\\" + c).join("") + "\\s]+)", "g");
  for (const m of text.matchAll(pattern)) if (m.index <= column && column <= m.index + m[0].length) {
    return { text: m[0], start: m.index, end: m.index + m[0].length };
  }
  return undefined;
}
function subword(word, column) {
  const cursor = column - word.start, text = word.text;
  const lower = c => c >= 97 && c <= 122, upper = c => c >= 65 && c <= 90;
  let left = cursor, right = cursor, previous = 0;
  for (; left >= 0; left--) {
    const c = text.charCodeAt(left);
    if (left !== cursor && (c === 95 || c === 45)) break;
    if (lower(c) && upper(previous)) break;
    previous = c;
  }
  for (left++; right < text.length; right++) {
    const c = text.charCodeAt(right);
    if ((upper(c) && lower(previous)) || c === 95 || c === 45) break;
    previous = c;
  }
  return left < right ? { start: word.start + left, end: word.start + right } : undefined;
}
function publicExpected(doc, queries) {
  const lines = doc.text.split(/\r?\n/), wire = expected(doc, queries);
  const positions = selection.positions(doc, queries);
  const whole = { start: point(0, 0), end: point(lines.length - 1, lines.at(-1).length) };
  return wire.map((row, index) => {
    const p = positions[index], line = lines[p.line], word = wordAt(line, p.character);
    const candidates = flatten(row);
    if (word) {
      const part = subword(word, p.character);
      if (part) candidates.push(range(p.line, part.start, part.end));
      candidates.push(range(p.line, word.start, word.end));
    }
    if (line.length && !/\S/.test(line)) candidates.push(range(p.line, 0, line.length));
    candidates.push(whole);
    candidates.sort((a, b) => cmp(b.start, a.start) || cmp(a.end, b.end));
    const nested = [];
    for (const r of candidates) if (!nested.length || (contains(r, nested.at(-1)) && !same(r, nested.at(-1)))) nested.push(r);
    const expanded = [nested[0]];
    for (let i = 1; i < nested.length; i++) {
      const child = nested[i - 1], parent = nested[i];
      if (child.start.line !== parent.start.line || child.end.line !== parent.end.line) {
        const first = lines[child.start.line].search(/\S/);
        const lastLine = lines[child.end.line], last = /\S/.test(lastLine) ? lastLine.trimEnd().length : -1;
        const trimmed = { start: point(child.start.line, first), end: point(child.end.line, last) };
        if (contains(trimmed, child) && !same(trimmed, child) && contains(parent, trimmed) && !same(parent, trimmed)) expanded.push(trimmed);
        const full = { start: point(child.start.line, 0), end: point(child.end.line, lastLine.length) };
        if (contains(full, child) && !same(full, trimmed) && contains(parent, full) && !same(parent, full)) expanded.push(full);
      }
      expanded.push(parent);
    }
    return chain(expanded);
  });
}
function publicRanges(rows) {
  assert(Array.isArray(rows), "actual public selection array");
  return rows.map(row => {
    if (!row) return null;
    const ranges = [], seen = new Set();
    for (; row; row = row.parent) {
      assert(!seen.has(row), "finite public parent chain"); seen.add(row);
      const r = row.range;
      ranges.push({ start: point(r.start.line, r.start.character), end: point(r.end.line, r.end.character) });
    }
    return chain(ranges);
  });
}
function resolve(ref) {
  assert(/^selection-[a-z-]+$/.test(ref.fixture));
  const fixture = require(`../../tests/lsp_matrix/fixtures/${ref.fixture}.json`);
  const item = fixture.oracle.cases.find(c => c.id === ref.case);
  assert(item, `authored editor case ${ref.fixture}/${ref.case}`);
  return { fixture, item };
}
module.exports = { ...selection, expected, flatten, publicExpected, publicRanges, wordAt, subword, resolve };
