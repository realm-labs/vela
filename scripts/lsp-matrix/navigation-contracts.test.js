"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { loadInventory } = require("./inventory");
const { navigationContracts, navigationModel } = require("./navigation-contracts");
const { navigationResponses } = require("./navigation-trace");

test("navigation contracts own all eight UX02 routes with distinct physical and command receipts", () => {
  const requirements = loadInventory(path.resolve(__dirname, "../..")).executionRequirements;
  const contracts = navigationContracts(requirements);
  assert.equal(contracts.length, 8);
  assert.deepEqual(contracts.flatMap((item) => item.requirements.map((item) => item.id)).sort(),
    requirements.filter((item) => item.id.startsWith("vscode/UX02/")).map((item) => item.id).sort());
  for (const contract of contracts) {
    const input = contract.id.endsWith("-input");
    assert.ok(contract.checks.every((check) => check.level === (input ? "Input" : "Command")));
    assert.ok(contract.actions.every((action) => action.device === (input ? "keyboard" : "command")));
    assert.ok(contract.checks.some((check) => check.id.startsWith("wire-")));
    assert.ok(contract.artifacts.includes("lsp-trace.log"));
  }
  assert.throws(() => navigationContracts(requirements.filter((item) =>
    item.id !== "vscode/UX02/definition-back/command/local")), /missing navigation obligation/);
});

test("navigation expectations preserve dirty Unicode byte versus UTF16 locations and unknown nulls", () => {
  const model = navigationModel();
  assert.equal(model.origin("call").dirty, true);
  assert.deepEqual(model.wire("definition", "call"), {
    method: "textDocument/definition", request: { file: "scripts/nav.vela", position: { line: 4, character: 22 } },
    result: { file: "scripts/navigation/definitions.vela", text: "make",
      range: { start: { line: 1, character: 17 }, end: { line: 1, character: 21 } } },
  });
  assert.deepEqual(model.wire("type", "typed-use").result.range,
    { start: { line: 0, character: 21 }, end: { line: 0, character: 25 } });
  for (const kind of ["definition", "declaration", "type"]) assert.equal(model.wire(kind, "unknown").result, null);
});

const sent = (id, method = "definition") => `[Trace - 03:44:26] Sending request 'textDocument/${method} - (${id})'.\nParams: {"textDocument":{"uri":"file:///workspace/%E4%B8%AD/nav.vela"},"position":{"line":4,"character":22}}\n\n\n`;
const response = (id, result = "No result returned.", method = "definition") =>
  `[Trace - 03:44:26] Received response 'textDocument/${method} - (${id})' in 1ms.\n${result}\n\n\n`;
test("passive navigation trace requires a complete matched request response pair after the boundary", () => {
  assert.deepEqual(navigationResponses(response(1)), [], "old request before boundary cannot satisfy a new action");
  assert.deepEqual(navigationResponses(sent(1) + response(2)), [], "different request IDs cannot be paired");
  assert.deepEqual(navigationResponses(sent(1) + response(1, "No result returned.", "declaration")), [], "method identity is required");
  assert.deepEqual(navigationResponses(sent(1) + response(1).slice(0, -1)), [], "partial flush is not evidence");
  const observed = navigationResponses(sent(1) + response(1));
  assert.equal(observed.length, 1);
  assert.equal(observed[0].result, null);
  assert.equal(observed[0].method, "textDocument/definition");
  assert.equal(observed[0].params.textDocument.uri, "file:///workspace/%E4%B8%AD/nav.vela");
});

test("passive navigation trace retains actual ranges and rejects malformed result records", () => {
  const location = { uri: "file:///workspace/target.vela", range: {
    start: { line: 1, character: 17 }, end: { line: 1, character: 21 },
  } };
  const record = sent(3, "typeDefinition") + response(3, `Result: ${JSON.stringify(location)}`, "typeDefinition");
  assert.deepEqual(navigationResponses(record)[0].result, location);
  assert.throws(() => navigationResponses(sent(3) + response(3, "Result: {broken}")), SyntaxError);
  assert.throws(() => navigationResponses(sent(3) + response(3, "unexpected payload")), /unrecognized/);
});
