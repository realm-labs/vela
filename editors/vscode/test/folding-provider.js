"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { EditorDocumentSession } = require("./editor-document-session");
const oracle = require("../../../scripts/lsp-matrix/folding-editor-oracle");
const { foldingResponses } = require("../../../scripts/lsp-matrix/folding-editor-trace");
const spec = require("../../../tests/lsp_matrix/fixtures/folding-editor.json");
const rootFor = (resultRoot, crlf, shifted) => path.join(resultRoot, "中文 % folding roots",
  `${crlf ? "crlf" : "lf"}-${shifted ? "shifted" : "original"}`);

function materializeFolding(resultRoot) {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const root = rootFor(resultRoot, crlf, shifted);
    assert(!fs.existsSync(root), "new private folding root");
    fs.mkdirSync(path.join(root, "scripts"), { recursive: true });
    for (const [file, source] of Object.entries(spec.files)) {
      const baseline = file === spec.oracle.file ? "// disk baseline 中😀\n" + source : source;
      fs.writeFileSync(path.join(root, file), oracle.document(baseline, crlf, shifted).text);
    }
    fs.writeFileSync(path.join(root, "vela.toml"), `[package]\nid = 'dev.vela.folding.${crlf}.${shifted}'\nname = 'folding_fixture'\nversion = '0.1.0'\n[source]\nroots = ['scripts']\n`);
  }
}

class FoldingSession extends EditorDocumentSession {
  constructor(vscode, workspace, root, crlf) {
    super(vscode, workspace, root, crlf, "folding"); this.requestIds = new Set();
  }
  async openSettled(file) {
    const uri = this.uri(file);
    const previous = this.vscode.workspace.textDocuments.some(doc => !doc.isClosed && this.sameUri(doc.uri.toString(), uri));
    const boundary = this.rows().length, since = Date.now();
    const current = await this.open(file);
    if (!previous) await this.mutation(boundary, since, "textDocument/didOpen", uri);
    return current;
  }
  async replaceSettled(file, doc) {
    await this.openSettled(file);
    const boundary = this.rows().length, since = Date.now();
    const current = await this.replace(file, doc);
    await this.mutation(boundary, since, "textDocument/didChange", current.uri);
    return current;
  }
  async verify(file, doc, ranges, label) {
    const current = await this.openSettled(file), version = current.version;
    assert.equal(current.getText(), doc.text, "exact effective source");
    assert.equal(current.eol, this.crlf ? this.vscode.EndOfLine.CRLF : this.vscode.EndOfLine.LF);
    assert.equal(this.vscode.workspace.getConfiguration("editor", current.uri).get("folding"), true);
    assert.equal(this.vscode.workspace.getConfiguration("editor", current.uri).get("foldingStrategy"), "auto");
    const disk = fs.readFileSync(current.uri.fsPath, "utf8");
    const expectedWire = oracle.wire(doc, ranges), expectedEditor = oracle.editor(doc, ranges);
    const observed = [], rawPublic = [], pairs = [], boundaries = [];
    this.receipts.push({ label, file, uri: current.uri.toString(), text: doc.text, disk, version,
      dirty: current.isDirty, expectedWire, expectedEditor, observed, rawPublic, pairs, boundaries, repeats: 3 });
    for (let repeat = 0; repeat < 3; repeat++) {
      const boundary = fs.readFileSync(this.trace, "utf8").length; boundaries.push(boundary);
      const actual = await this.bounded(label, () => this.vscode.commands.executeCommand("vscode.executeFoldingRangeProvider", current.uri));
      assert(Array.isArray(actual), "actual folding provider array");
      rawPublic.push(actual.map(row => ({ start: row.start, end: row.end, kind: row.kind })));
      const projected = oracle.publicRanges(actual, this.vscode.FoldingRangeKind);
      observed.push(projected);
      assert.deepEqual(projected, expectedEditor, `${label}: whole public folding model, repeat ${repeat}`);
      const records = await this.until(`${label}: fresh wire folding`, () => {
        const records = foldingResponses(fs.readFileSync(this.trace, "utf8").slice(boundary))
          .filter(row => this.sameUri(row.params.textDocument?.uri, current.uri));
        return records.length ? records : null;
      });
      // Background UI requests can coincide with the public command. Check
      // every complete owned fresh pair, never select the first passing result.
      for (const row of records) {
        assert(!this.requestIds.has(row.id), "fresh request ID for every invocation"); this.requestIds.add(row.id);
        assert.deepEqual(row.params, { textDocument: { uri: row.params.textDocument.uri } });
        assert.deepEqual(row.result, expectedWire, "whole ordered UTF16 ranges; [] distinct from null/failure");
      }
      pairs.push(records);
      assert.equal(current.version, version, "query preserves model version");
      assert.equal(current.getText(), doc.text, "query preserves buffer");
      assert.equal(fs.readFileSync(current.uri.fsPath, "utf8"), disk, "query preserves physical bytes");
    }
  }
}

async function runFolding(vscode, workspace) {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const session = new FoldingSession(vscode, workspace, rootFor(process.env.VELA_TEST_RESULT_DIR, crlf, shifted), crlf);
    const document = text => oracle.document(text, crlf, shifted);
    const initial = document("// disk baseline 中😀\n" + spec.files[spec.oracle.file]), helper = document(spec.files["scripts/helper.vela"]);
    const verify = async (doc, ranges, label) => {
      await session.verify(spec.oracle.file, doc, ranges, label);
      await session.verify("scripts/helper.vela", helper, [], `${label}/helper`);
      assert.equal(fs.readFileSync(session.uri(spec.oracle.file).fsPath, "utf8"), initial.text);
    };
    let failure;
    try {
      await session.addRoot();
      await verify(initial, spec.oracle.cases[0].ranges, "disk");
      for (const item of spec.oracle.cases) {
        const doc = document(item.source);
        const current = await session.replaceSettled(spec.oracle.file, doc);
        assert(current.isDirty, "every authored corpus case remains unsaved");
        await verify(doc, item.ranges, `dirty/${item.id}`);
      }
      await session.close(spec.oracle.file);
      await verify(initial, spec.oracle.cases[0].ranges, "close-restored");
    } catch (error) { failure = error; throw error; }
    finally {
      try { await session.finish(); }
      catch (cleanup) {
        if (failure) throw new AggregateError([failure, cleanup], `${failure.stack}\nCleanup also failed: ${cleanup.stack}`);
        throw cleanup;
      }
    }
  }
}
module.exports = { materializeFolding, runFolding };
