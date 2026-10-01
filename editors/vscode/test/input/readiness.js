"use strict";
const fs = require("node:fs");
const path = require("node:path");

const mutationMethods = new Set([
  "textDocument/didOpen", "textDocument/didChange", "textDocument/didClose",
  "textDocument/didSave", "workspace/didChangeWatchedFiles",
  "workspace/didChangeConfiguration", "workspace/didChangeWorkspaceFolders",
]);

function readTrace(workspace) {
  const file = path.join(workspace, ".vela-lsp-trace.jsonl");
  if (!fs.existsSync(file)) return [];
  // Only newline-terminated records are complete while the server is writing.
  return fs.readFileSync(file, "utf8").split(/\r?\n/).slice(0, -1)
    .filter(Boolean).map(line => JSON.parse(line));
}

function workspaceReadiness(rows, { since, now, quietMs = 750 }) {
  const received = rows.filter(row => row.event === "message_received" &&
    row.lane === "main" && mutationMethods.has(row.method));
  if (!received.length) return false;
  let lastCompletedAt = since;
  for (const request of received) {
    const completed = rows.find(row => row.event === "response_sent" &&
      row.lane === "main" && row.seq === request.seq && row.method === request.method &&
      row.status === "completed");
    if (!completed) return false;
    lastCompletedAt = Math.max(lastCompletedAt, completed.timestampMs);
  }
  // The installed client batches filesystem events for 250 ms. Observe a longer
  // quiet window, restarting it for every completed mutation. No accepted input
  // is issued or retried here, and ordinary provider traffic does not reset it.
  if (now - lastCompletedAt < quietMs) return false;
  return { mutations: received.length, lastSeq: received.at(-1).seq,
    lastCompletedAt, quietMs, observedAt: now };
}

function completedResponse(row) {
  // Main-loop dispatch also emits response_sent with zero output for worker
  // requests. Only a real completed response proves the provider has finished.
  return row.event === "response_sent" && row.status === "completed" &&
    row.resultKind === "response" && row.outputMessages > 0;
}

module.exports = { readTrace, workspaceReadiness, completedResponse };
