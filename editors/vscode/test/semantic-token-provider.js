"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { tokenModel, decodeTokens, within } = require("../../../scripts/lsp-matrix/semantic-token-oracle");
const { tokenResponses, tokenStreams } = require("../../../scripts/lsp-matrix/semantic-token-trace");
const { findLog } = require("./input/logs");

async function runTokenProvider(vscode, workspace, kind) {
  const root = process.env.VELA_TEST_RESULT_DIR;
  const trace = findLog(root, name => name.endsWith("-Vela LSP Trace.log"));
  const receipts = [];
  const uri = vscode.Uri.joinPath(workspace, tokenModel().file);
  const disk = tokenModel().states[0].document.text;
  assert.equal(fs.readFileSync(uri.fsPath, "utf8"), disk, "independent physical disk baseline");
  const responses = () => tokenResponses(fs.readFileSync(trace, "utf8"));
  const forDocument = () => responses().filter(record => record.params.textDocument.uri === uri.toString());
  const full = document => vscode.commands.executeCommand("vscode.provideDocumentSemanticTokens", document.uri);
  const position = (line, character) => new vscode.Position(line, character);
  const range = (start, end) => new vscode.Range(position(start.line, start.character), position(end.line, end.character));
  const wait = async (label, observe) => {
    const deadline = Date.now() + 15000;
    while (Date.now() < deadline) {
      const value = observe();
      if (value) return value;
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    throw Error(`${label}: no complete matching installed-client response`);
  };
  const assertFull = async (document, state, legend) => {
    const result = await full(document);
    assert.deepEqual(decodeTokens(result?.data, legend, document.getText()), state.tokens, `${kind}/${state.id}`);
    assert.deepEqual(Array.from((await full(document)).data), Array.from(result.data), "repeat full agrees");
    return Array.from(result.data);
  };
  const deltaAfter = async (boundary, previous, current, legend) => wait(`${current.id} actual delta`, () => {
    for (const record of tokenStreams(forDocument())) {
      const { result, applied: data, previousData: base } = record;
      if (Number(record.id) <= boundary || !base || !result.edits?.length) continue;
      // Only a transition from the exact preceding state can certify this edit.
      if (JSON.stringify(decodeTokens(base, legend, previous.document.text)) !== JSON.stringify(previous.tokens)) continue;
      assert.deepEqual(decodeTokens(data, legend, current.document.text), current.tokens, "applied actual delta equals independent current stream");
      assert.notEqual(result.resultId, record.params.previousResultId, "changed content has a new result ID");
      return record;
    }
    return null;
  });

  for (const crlf of [false, true]) {
    const model = tokenModel(crlf);
    const document = await vscode.workspace.openTextDocument(uri);
    const editor = await vscode.window.showTextDocument(document);
    const legend = await vscode.commands.executeCommand("vscode.provideDocumentSemanticTokensLegend", uri);
    assert(legend?.tokenTypes.includes("property") && legend.tokenTypes.includes("struct"), "real installed full legend");
    let previous;
    for (const state of model.states) {
      const boundary = Math.max(-1, ...forDocument().map(record => Number(record.id)));
      assert(await editor.edit(builder => {
        builder.replace(new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length)), state.document.text);
        builder.setEndOfLine(crlf ? vscode.EndOfLine.CRLF : vscode.EndOfLine.LF);
      }));
      assert.equal(document.getText(), state.document.text, "exact source state");
      assert.equal(document.eol, crlf ? vscode.EndOfLine.CRLF : vscode.EndOfLine.LF);
      if (previous) assert(document.isDirty, "Unicode and semantic edits remain unsaved");
      let delta;
      if (kind === "delta" && previous) delta = await deltaAfter(boundary, previous, state, legend);
      if (kind === "delta" && !previous) {
        // Establish the renderer's own cache before public full commands run.
        await wait("initial renderer baseline", () => tokenStreams(forDocument()).some(record =>
          Number(record.id) > boundary &&
          JSON.stringify(decodeTokens(record.applied, legend, state.document.text)) === JSON.stringify(state.tokens)));
      }
      const data = await assertFull(document, state, legend);
      if (delta) assert.deepEqual(delta.applied, data, "actual delta also equals fresh public full");
      let ranges = 0;
      if (kind === "range") {
        const rangeLegend = await vscode.commands.executeCommand("vscode.provideDocumentRangeSemanticTokensLegend", uri);
        assert.deepEqual(rangeLegend, legend, "same installed full/range legend");
        const queries = state.tokens.map(token => ({
          start: { line: token.line, character: token.start },
          end: { line: token.line, character: token.start + token.length },
        }));
        for (const line of [...new Set(state.tokens.map(token => token.line))]) {
          queries.push({ start: { line, character: 0 }, end: { line: line + 1, character: 0 } });
        }
        queries.push({ start: { line: 0, character: 0 }, end: { line: 0, character: 0 } });
        for (const query of queries) {
          const expected = query.start.line === query.end.line && query.start.character === query.end.character ? [] : within(state.tokens, query);
          for (let repeat = 0; repeat < 2; repeat++) {
            const result = await vscode.commands.executeCommand("vscode.provideDocumentRangeSemanticTokens", uri, range(query.start, query.end));
            assert.deepEqual(decodeTokens(result?.data, legend, document.getText()), expected, `exact token/line/empty range ${JSON.stringify(query)}`);
          }
          ranges++;
        }
      }
      assert.equal(document.getText(), state.document.text, "queries cannot edit source");
      assert.equal(fs.readFileSync(uri.fsPath, "utf8"), disk, "queries and unsaved edits preserve physical disk");
      receipts.push({ crlf, state: state.id, tokens: state.tokens, data, ranges, delta });
      previous = state;
    }
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    const restored = await vscode.workspace.openTextDocument(uri);
    assert.equal(restored.getText(), tokenModel().states[0].document.text, "close discards edits and restores physical disk");
    const restoredData = await assertFull(restored, model.states[0], legend);
    receipts.push({ crlf, state: "closed-restored", data: restoredData });
    await vscode.window.showTextDocument(restored);
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
  }
  fs.writeFileSync(path.join(root, `tokens-${kind}-observations.json`), JSON.stringify(receipts, null, 2));
  fs.copyFileSync(trace, path.join(root, `tokens-${kind}-client.log`));
}
module.exports = { runTokenProvider };
