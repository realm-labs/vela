"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const fs = require("node:fs"), os = require("node:os"), path = require("node:path");
const { readTrace, workspaceReadiness, completedResponse } = require("../../editors/vscode/test/input/readiness");
const received = (seq, method = "textDocument/didOpen") =>
  ({ event: "message_received", lane: "main", seq, method });
const finished = (seq, timestampMs, method = "textDocument/didOpen") =>
  ({ event: "response_sent", lane: "main", seq, method, status: "completed", timestampMs });

test("workspace readiness requires actual notification completion and a quiet window", () => {
  const options = { since: 1000, now: 1750 };
  assert.equal(workspaceReadiness([], options), false);
  assert.equal(workspaceReadiness([received(1)], options), false);
  const rows = [received(1), finished(1, 900)];
  assert.equal(workspaceReadiness(rows, { ...options, now: 1749 }), false);
  assert.deepEqual(workspaceReadiness(rows, options), {
    mutations: 1, lastSeq: 1, lastCompletedAt: 1000, quietMs: 750, observedAt: 1750,
  });
});

test("late watched changes restart readiness and mismatched acknowledgements cannot unlock it", () => {
  const watched = "workspace/didChangeWatchedFiles";
  const rows = [received(1), finished(1, 900), received(2, watched)];
  assert.equal(workspaceReadiness(rows, { since: 1000, now: 3000 }), false);
  rows.push(finished(2, 1200)); // Wrong method.
  assert.equal(workspaceReadiness(rows, { since: 1000, now: 3000 }), false);
  rows.push(finished(2, 1600, watched));
  assert.equal(workspaceReadiness(rows, { since: 1000, now: 2349 }), false);
  assert.deepEqual(workspaceReadiness(rows, { since: 1000, now: 2350 }), {
    mutations: 2, lastSeq: 2, lastCompletedAt: 1600, quietMs: 750, observedAt: 2350,
  });
});

test("provider traffic cannot hide a pending mutation or prolong a settled workspace", () => {
  const rows = [received(1), finished(1, 900), received(2, "textDocument/references"),
    finished(2, 3000, "textDocument/references")];
  assert.ok(workspaceReadiness(rows, { since: 1000, now: 1750 }));
  rows.push(received(3, "textDocument/didChange"));
  assert.equal(workspaceReadiness(rows, { since: 1000, now: 4000 }), false);
});

test("trace observation ignores only a final incomplete record and preserves malformed-record failures", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "vela-readiness-"));
  try {
    assert.deepEqual(readTrace(root), []);
    const file = path.join(root, ".vela-lsp-trace.jsonl");
    fs.writeFileSync(file, JSON.stringify(received(1)) + '\r\n{"event":');
    assert.deepEqual(readTrace(root), [received(1)]);
    fs.appendFileSync(file, "\n");
    assert.throws(() => readTrace(root), SyntaxError);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});

test("provider completion rejects empty dispatch acknowledgements, stale results and failures", () => {
  const result = { event: "response_sent", status: "completed", resultKind: "response", outputMessages: 1 };
  assert.equal(completedResponse({ ...result, resultKind: "none", outputMessages: 0 }), false);
  assert.equal(completedResponse({ ...result, status: "stale_discarded" }), false);
  assert.equal(completedResponse({ ...result, status: "failed" }), false);
  assert.equal(completedResponse(result), true);
});
