"use strict";
// Read the installed client's complete request/response pairs. No requests are
// sent here, and an unfinished log record cannot prove a native hover action.
function hoverResponses(text) {
  const requests = new Map(), responses = [], blocks = text.split("\n\n\n");
  blocks.pop();
  for (const block of blocks) {
    const sent = block.match(/Sending request 'textDocument\/hover - \((\d+)\)'\.\nParams: ([\s\S]+)$/);
    if (sent) { requests.set(sent[1], JSON.parse(sent[2])); continue; }
    const received = block.match(/Received response 'textDocument\/hover - \((\d+)\)' in \d+ms\.\n([\s\S]+)$/);
    if (!received || !requests.has(received[1])) continue;
    const payload = received[2].trim();
    const result = payload === "No result returned." ? null : payload.startsWith("Result: ") ? JSON.parse(payload.slice(8)) : undefined;
    if (result === undefined) throw Error("unrecognized hover response log");
    responses.push({ method: "textDocument/hover", id: received[1], params: requests.get(received[1]), result });
  }
  return responses;
}
module.exports = { hoverResponses };
