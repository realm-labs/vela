"use strict";
const assert = require("node:assert/strict"), test = require("node:test"), fs = require("node:fs"), os = require("node:os"), path = require("node:path");
const { AppendOnlyTrace, readTail } = require("./append-only-trace");
const { readTrace } = require("../../editors/vscode/test/input/readiness");
function owned(run) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-trace-")), file = path.join(root, ".vela-lsp-trace.jsonl");
  assert(path.isAbsolute(root)); assert.equal(path.dirname(root), path.resolve(os.tmpdir()));
  assert(path.basename(root).startsWith("vela-trace-"));
  try { run(root, file); } finally { fs.rmSync(root, { recursive: true, force: true }); }
}
test("append-only trace matches complete journal records at every byte boundary including split UTF8 and CRLF", () => owned((root, file) => {
  const cursor = new AppendOnlyTrace(file); assert.deepEqual(cursor.rows(), []);
  const rows = [{ text: "中😀", seq: 1 }, { method: "didChange", seq: 2 }];
  const bytes = Buffer.from(JSON.stringify(rows[0])+"\r\n\n"+JSON.stringify(rows[1])+"\n");
  for (let i=0; i<bytes.length; i++) {
    fs.appendFileSync(file, bytes.subarray(i,i+1));
    assert.deepEqual(cursor.rows(), readTrace(root));
    const before = structuredClone(cursor.rows()); assert.deepEqual(cursor.rows(), before);
  }
  assert.deepEqual(cursor.rows(), rows);
}));
test("append-only trace rejects corruption truncation and disappearance instead of discarding observed evidence", () => {
  owned((root,file) => { const cursor=new AppendOnlyTrace(file);fs.writeFileSync(file,"not json\n");assert.throws(()=>cursor.rows(),SyntaxError);assert.throws(()=>cursor.rows(),SyntaxError); });
  owned((root,file) => { const cursor=new AppendOnlyTrace(file);fs.writeFileSync(file,'{"seq":1}\n');cursor.rows();fs.writeFileSync(file,'');assert.throws(()=>cursor.rows(),/must not truncate/); });
  owned((root,file) => { const cursor=new AppendOnlyTrace(file);fs.writeFileSync(file,'{"seq":1}\n');cursor.rows();fs.unlinkSync(file);assert.throws(()=>cursor.rows(),/must not disappear/); });
});
test("trace byte tails preserve fresh Unicode requests at every boundary without reusing old records", () => owned((root,file) => {
  const bytes=Buffer.from('old 中😀\r\nfresh 中😀 request\n');fs.writeFileSync(file,bytes);
  for(let i=0;i<=bytes.length;i++)assert.deepEqual(readTail(file,i),bytes.subarray(i));
  assert.throws(()=>readTail(file,-1));assert.throws(()=>readTail(file,bytes.length+1));
}));
