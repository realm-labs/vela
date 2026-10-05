"use strict";

// Complete fresh installed-client request/response pairs, not synthetic traffic.
function workspaceSymbolResponses(text) {
  const requests = new Map(), completed = new Set(), responses = [];
  const blocks = text.replaceAll("\r\n", "\n").split("\n\n\n"); blocks.pop();
  for (const block of blocks) {
    const sent = block.match(/Sending request 'workspace\/symbol - \((\d+)\)'\.\nParams: ([\s\S]+)$/);
    if (sent) {
      if (requests.has(sent[1])) throw Error("duplicate workspace symbol request");
      requests.set(sent[1], JSON.parse(sent[2])); continue;
    }
    const received = block.match(/Received response 'workspace\/symbol - \((\d+)\)' in \d+ms\.\n([\s\S]+)$/);
    if (!received || !requests.has(received[1])) continue;
    if (completed.has(received[1])) throw Error("duplicate workspace symbol response");
    completed.add(received[1]);
    const payload = received[2].trim();
    const result = payload === "No result returned." ? null :
      payload.startsWith("Result: ") ? JSON.parse(payload.slice(8)) : undefined;
    if (result === undefined) throw Error("unrecognized workspace symbol response log");
    responses.push({ id: received[1], params: requests.get(received[1]), result });
  }
  return responses;
}
module.exports = { workspaceSymbolResponses };
