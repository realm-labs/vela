"use strict";

function foldingResponses(text) {
  const requests = new Map(), completed = new Set(), responses = [];
  const blocks = text.replaceAll("\r\n", "\n").split("\n\n\n"); blocks.pop();
  for (const block of blocks) {
    const sent = block.match(/Sending request 'textDocument\/foldingRange - \((\d+)\)'\.\nParams: ([\s\S]+)$/);
    if (sent) {
      if (requests.has(sent[1])) throw Error("duplicate folding request");
      requests.set(sent[1], JSON.parse(sent[2])); continue;
    }
    const received = block.match(/Received response 'textDocument\/foldingRange - \((\d+)\)' in \d+ms\.\n([\s\S]+)$/);
    if (!received || !requests.has(received[1])) continue;
    if (completed.has(received[1])) throw Error("duplicate folding response");
    completed.add(received[1]);
    const payload = received[2].trim();
    const result = payload === "No result returned." ? null :
      payload.startsWith("Result: ") ? JSON.parse(payload.slice(8)) : undefined;
    if (result === undefined) throw Error("unrecognized folding response log");
    responses.push({ id: received[1], params: requests.get(received[1]), result });
  }
  return responses;
}
module.exports = { foldingResponses };
