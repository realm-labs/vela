"use strict";
const assert = require("node:assert/strict"), fs = require("node:fs");
function readTail(file, offset) {
  const size = fs.statSync(file).size;
  assert(Number.isSafeInteger(offset) && offset >= 0 && size >= offset, "valid append-only byte boundary");
  const bytes = Buffer.alloc(size - offset), handle = fs.openSync(file, "r");
  let count = 0;
  try {
    while (count < bytes.length) {
      const read = fs.readSync(handle, bytes, count, bytes.length - count, offset + count);
      assert(read > 0, "snapshot bytes remain readable"); count += read;
    }
  } finally { fs.closeSync(handle); }
  return bytes;
}
class AppendOnlyTrace {
  constructor(file) { this.file = file; this.offset = 0; this.pending = Buffer.alloc(0); this.records = []; }
  rows() {
    if (!fs.existsSync(this.file)) { assert.equal(this.offset, 0, "observed trace must not disappear"); return this.records; }
    const size = fs.statSync(this.file).size;
    assert(size >= this.offset, "observed trace must not truncate");
    if (size === this.offset) return this.records;
    const bytes = readTail(this.file, this.offset);
    const complete = Buffer.concat([this.pending, bytes]), last = complete.lastIndexOf(10);
    const records = last >= 0 ? complete.subarray(0, last + 1).toString("utf8").split(/\r?\n/).filter(Boolean).map(line => JSON.parse(line)) : [];
    for (const record of records) this.records.push(record);
    this.offset += bytes.length;
    this.pending = Buffer.from(complete.subarray(last + 1));
    return this.records;
  }
}
module.exports = { AppendOnlyTrace, readTail };
