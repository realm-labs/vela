"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const { EditorDocumentSession } = require("./editor-document-session");
const oracle = require("../../../scripts/lsp-matrix/document-symbol-oracle");
const { symbolResponses } = require("../../../scripts/lsp-matrix/document-symbol-trace");

class SymbolSession extends EditorDocumentSession {
  async verify(file, doc, rows, label) {
    const current = await this.open(file);
    assert.equal(current.getText(), doc.text, `${label}: exact effective text`);
    const wanted = oracle.expected(doc, rows); oracle.assertAncestry(wanted);
    const range = r => [r.start.line, r.start.character, r.end.line, r.end.character];
    const project = symbols => symbols.map(symbol => {
      assert(symbol.range && symbol.selectionRange, "hierarchical DocumentSymbol, not flat SymbolInformation");
      return { name: symbol.name, kind: symbol.kind, detail: symbol.detail,
        range: range(symbol.range), selectionRange: range(symbol.selectionRange), children: project(symbol.children) };
    });
    const disk = fs.readFileSync(current.uri.fsPath, "utf8");
    const observed = [], emptyResponses = [];
    const state = this.modelStates.get(file);
    assert.equal(state?.version, current.version, "owned opening/edit boundary matches current model version");
    for (let repeat = 0; repeat < 3; repeat++) {
      const symbols = await this.bounded(label, () => this.vscode.commands.executeCommand("vscode.executeDocumentSymbolProvider", current.uri));
      // VS Code's command converts an empty outline to undefined. Require the
      // current version's actual [] response. OutlineModel can reuse that pair
      // across repeated public queries; absent/failed providers cannot pass.
      if (wanted.length === 0) {
        const records = await this.until(`${label}: actual empty response`, () => {
          const records = symbolResponses(fs.readFileSync(this.trace, "utf8").slice(state.boundary))
            .filter(record => this.sameUri(record.params.textDocument.uri, current.uri));
          return records.length ? records : null;
        });
        for (const record of records) assert.deepEqual(record.result, [], "wire empty, not null or failed provider");
        emptyResponses.push(records);
      }
      const actual = symbols === undefined && wanted.length === 0 ? [] : project(symbols);
      assert.deepEqual(actual, wanted, `${label}: whole ordered tree, repeat ${repeat}`);
      observed.push(actual);
      assert.equal(current.getText(), doc.text, "outline query preserves buffer");
      assert.equal(fs.readFileSync(current.uri.fsPath, "utf8"), disk, "outline query preserves physical disk");
    }
    this.receipts.push({ label, file, text: doc.text, disk, modelState: state, expected: wanted, observed, emptyResponses, nodes: oracle.count(wanted), repeats: 3 });
  }
}
module.exports = { SymbolSession };
