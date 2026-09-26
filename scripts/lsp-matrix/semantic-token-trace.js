"use strict";
const assert = require("node:assert/strict");
const { applyTokenDelta } = require("./semantic-token-oracle");
// Read the installed client's actual verbose request/response log. No transport
// or provider is created here; incomplete records cannot supply evidence.
function tokenResponses(text) {
  const requests = new Map(), responses = [], completed = new Set();
  const blocks = text.replaceAll("\r\n", "\n").split("\n\n\n");
  blocks.pop();
  for (const block of blocks) {
    const sent = block.match(/Sending request '(textDocument\/semanticTokens\/(?:full(?:\/delta)?|range)) - \((\d+)\)'\.\nParams: ([\s\S]+)$/);
    if (sent) {
      const key = `${sent[1]}/${sent[2]}`;
      if (requests.has(key)) throw Error("duplicate semantic token request");
      requests.set(key, JSON.parse(sent[3]));
      continue;
    }
    const received = block.match(/Received response '(textDocument\/semanticTokens\/(?:full(?:\/delta)?|range)) - \((\d+)\)' in \d+ms\.\n([\s\S]+)$/);
    if (!received) continue;
    const key = `${received[1]}/${received[2]}`, params = requests.get(key);
    if (!params) continue;
    if (completed.has(key)) throw Error("duplicate semantic token response");
    completed.add(key);
    const payload = received[3].trim();
    if (payload === "No result returned.") continue;
    if (!payload.startsWith("Result: ")) throw Error("unrecognized semantic token response log");
    responses.push({ method: received[1], id: received[2], params, result: JSON.parse(payload.slice(8)) });
  }
  return responses;
}
function tokenStreams(records) {
  const bases = new Map(), streams = [];
  for (const record of records) {
    if (record.method.endsWith("/range") || !record.result) continue;
    const { result } = record;
    let data, previousData;
    if (Array.isArray(result.data)) data = result.data;
    else if (record.method.endsWith("/delta")) {
      previousData = bases.get(record.params.previousResultId);
      assert(previousData, "actual delta must name an observed full/delta predecessor");
      data = applyTokenDelta(previousData, result);
    } else continue;
    assert(typeof result.resultId === "string" && result.resultId.length, "observed token result ID");
    bases.set(result.resultId, data);
    streams.push({ ...record, previousData, applied: data });
  }
  return streams;
}
module.exports = { tokenResponses, tokenStreams };
