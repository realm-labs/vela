"use strict";

// Observe the installed language client's own verbose log; never send a request.
// A complete request/response pair after the action boundary is required.
function navigationResponses(text) {
  const requests = new Map(), responses = [];
  const blocks = text.split("\n\n\n");
  blocks.pop(); // A partly flushed final record is not evidence yet.
  for (const block of blocks) {
    const sent = block.match(/Sending request '(textDocument\/(?:definition|declaration|typeDefinition)) - \((\d+)\)'\.\nParams: ([\s\S]+)$/);
    if (sent) {
      requests.set(`${sent[1]}/${sent[2]}`, JSON.parse(sent[3]));
      continue;
    }
    const received = block.match(/Received response '(textDocument\/(?:definition|declaration|typeDefinition)) - \((\d+)\)' in \d+ms\.\n([\s\S]+)$/);
    if (!received) continue;
    const params = requests.get(`${received[1]}/${received[2]}`);
    if (!params) continue;
    const payload = received[3].trim();
    const result = payload === "No result returned." ? null :
      payload.startsWith("Result: ") ? JSON.parse(payload.slice(8)) : undefined;
    if (result === undefined) throw Error("unrecognized navigation response log");
    responses.push({ method: received[1], id: received[2], params, result });
  }
  return responses;
}
module.exports = { navigationResponses };
