"use strict";
const assert = require("node:assert/strict"), fs = require("node:fs"), path = require("node:path");
const { EditorDocumentSession } = require("./editor-document-session");
const oracle = require("../../../scripts/lsp-matrix/selection-editor-oracle");
const { selectionTrace } = require("../../../scripts/lsp-matrix/selection-editor-trace");
const { AppendOnlyTrace, readTail } = require("../../../scripts/lsp-matrix/append-only-trace");

class SelectionSession extends EditorDocumentSession {
  constructor(vscode, workspace, root, crlf, prefix) {
    super(vscode, workspace, root, crlf, prefix); this.requestIds = new Set();
    this.journal = new AppendOnlyTrace(path.join(workspace.fsPath, ".vela-lsp-trace.jsonl"));
  }
  rows() { return this.journal.rows(); }
  async openSettled(file) {
    const uri = this.uri(file);
    const previous = this.vscode.workspace.textDocuments.some(doc => !doc.isClosed && this.sameUri(doc.uri.toString(), uri));
    const boundary = this.rows().length, since = Date.now(), current = await this.open(file);
    if (!previous) await this.mutation(boundary, since, "textDocument/didOpen", uri);
    return current;
  }
  async replaceSettled(file, doc) {
    const previous = await this.openSettled(file), version = previous.version, text = previous.getText(), eol = previous.eol;
    const boundary = this.rows().length, since = Date.now(), current = await this.replace(file, doc);
    if (current.version !== version) await this.mutation(boundary, since, "textDocument/didChange", current.uri);
    else {
      assert.equal(text, doc.text, "unchanged version requires an actual no-op edit");
      assert.equal(current.eol, eol);
      this.receipts.push({ unchangedEdit: true, file, version, text, eol });
    }
    return current;
  }
  disk(file) { const path = this.uri(file).fsPath; return fs.existsSync(path) ? fs.readFileSync(path, "utf8") : null; }
  async verify(file, doc, queries, label) {
    const current = await this.openSettled(file), version = current.version;
    assert.equal(current.languageId, "vela"); assert.equal(current.getText(), doc.text);
    // Empty/single-line models default to LF even when their fixture is CRLF.
    if (doc.text.includes("\n")) assert.equal(current.eol, this.crlf ? this.vscode.EndOfLine.CRLF : this.vscode.EndOfLine.LF);
    const disk = this.disk(file), positions = oracle.positions(doc, queries);
    const expectedWire = oracle.expected(doc, queries), expectedEditor = oracle.publicExpected(doc, queries);
    const observed = [], rawPublic = [], pairs = [], boundaries = [];
    this.receipts.push({ label, file, uri: current.uri.toString(), text: doc.text, disk, version,
      dirty: current.isDirty, positions, expectedWire, expectedEditor, observed, rawPublic, pairs, boundaries, repeats: 3 });
    for (let repeat = 0; repeat < 3; repeat++) {
      const boundary = fs.statSync(this.trace).size; boundaries.push(boundary);
      const actual = await this.bounded(label, () => this.vscode.commands.executeCommand("vscode.executeSelectionRangeProvider",
        current.uri, positions.map(p => new this.vscode.Position(p.line, p.character))));
      const projected = oracle.publicRanges(actual); rawPublic.push(projected); observed.push(projected);
      const records = await this.until(`${label}: fresh wire selection`, () => {
        const parsed = selectionTrace(readTail(this.trace, boundary).toString("utf8"));
        const rows = parsed.responses.filter(r => this.sameUri(r.params.textDocument?.uri, current.uri));
        return rows.length ? rows : null;
      });
      pairs.push(records);
      assert.deepEqual(projected, expectedEditor, `${label}: complete public selection parent chains, repeat ${repeat}`);
      for (const row of records) {
        assert(!this.requestIds.has(row.id), "fresh wire request ID per invocation"); this.requestIds.add(row.id);
        assert.deepEqual(row.params, { textDocument: { uri: row.params.textDocument.uri }, positions });
        assert.deepEqual(row.result, expectedWire, "complete UTF16 wire vectors preserve order, points and []");
      }
      assert.equal(current.version, version); assert.equal(current.getText(), doc.text);
      assert.equal(this.disk(file), disk, "query preserves exact physical bytes/absence");
    }
  }
  async verifyAbsent(file, label) {
    const uri = this.uri(file); assert.equal(this.disk(file), null);
    assert(!this.vscode.workspace.textDocuments.some(d => !d.isClosed && this.sameUri(d.uri.toString(), uri)));
    const attempts = [];
    this.receipts.push({ label, file, uri: uri.toString(), absent: true, attempts, repeats: 3 });
    for (let repeat = 0; repeat < 3; repeat++) {
      const boundary = fs.statSync(this.trace).size;
      let error;
      await assert.rejects(this.bounded(label, () => this.vscode.commands.executeCommand("vscode.executeSelectionRangeProvider",
        uri, [new this.vscode.Position(0, 0)])), e => {
        error = e.message; return /Unable to resolve|Unable to read|does not exist|not found|nonexistent/i.test(error);
      });
      const sent = selectionTrace(readTail(this.trace, boundary).toString("utf8")).requests
        .filter(r => this.sameUri(r.params.textDocument?.uri, uri));
      assert.deepEqual(sent, [], "unresolvable public model never issues an owned selection request");
      attempts.push({ error, boundary, requests: sent }); assert.equal(this.disk(file), null);
    }
  }
}
module.exports = { SelectionSession };
