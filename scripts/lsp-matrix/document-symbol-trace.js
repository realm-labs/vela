"use strict";

// Observe complete installed-client pairs; never create a provider or request.
function symbolResponses(text) {
  const requests = new Map(), completed = new Set(), responses = [];
  const blocks = text.replaceAll("\r\n", "\n").split("\n\n\n");
  blocks.pop();
  for (const block of blocks) {
    const sent = block.match(/Sending request 'textDocument\/documentSymbol - \((\d+)\)'\.\nParams: ([\s\S]+)$/);
    if (sent) {
      if (requests.has(sent[1])) throw Error("duplicate document symbol request");
      requests.set(sent[1], JSON.parse(sent[2]));
      continue;
    }
    const received = block.match(/Received response 'textDocument\/documentSymbol - \((\d+)\)' in \d+ms\.\n([\s\S]+)$/);
    if (!received || !requests.has(received[1])) continue;
    if (completed.has(received[1])) throw Error("duplicate document symbol response");
    completed.add(received[1]);
    const payload = received[2].trim();
    const result = payload === "No result returned." ? null :
      payload.startsWith("Result: ") ? JSON.parse(payload.slice(8)) : undefined;
    if (result === undefined) throw Error("unrecognized document symbol response log");
    responses.push({ id: received[1], params: requests.get(received[1]), result });
  }
  return responses;
}
module.exports = { symbolResponses };
