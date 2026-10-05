"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { isDeepStrictEqual } = require("node:util");
const { safeFile } = require("../../../scripts/lsp-matrix/fixtures");
const oracle = require("../../../scripts/lsp-matrix/workspace-symbol-oracle");
const { workspaceSymbolResponses } = require("../../../scripts/lsp-matrix/workspace-symbol-trace");
const { readTrace, workspaceReadiness } = require("./input/readiness");
const { findLog } = require("./input/logs");
const specs = Object.fromEntries(["declarations", "ownership"].map(name =>
  [name, require(`../../../tests/lsp_matrix/fixtures/workspace-symbol-${name}.json`)]));
const checkName = "workspace symbol provider preserves complete source and schema sets through Unicode dirty close restoration";
const workspaceFor = root => path.join(root, "中文 % workspace symbols");
const familyFor = (root, name, crlf) => path.join(workspaceFor(root), `${name}-${crlf ? "crlf" : "lf"}`);
const documents = (spec, crlf, shifted = false) => Object.fromEntries(Object.entries(spec.files)
  .filter(([file]) => file.endsWith(".vela")).map(([file, text]) => [file,
    oracle.document(shifted ? text.replace("[[file:start]]", "[[file:start]]// shifted 中😀\n/* second 😀 */\n") : text, crlf)]));

function materializeWorkspaceSymbols(root) {
  const workspace = workspaceFor(root); assert(!fs.existsSync(workspace), "fresh private workspace");
  for (const [name, spec] of Object.entries(specs)) for (const crlf of [false, true]) {
    const family = familyFor(root, name, crlf);
    for (const [file, doc] of Object.entries(documents(spec, crlf))) {
      const destination = path.join(family, safeFile(file));
      fs.mkdirSync(path.dirname(destination), { recursive: true }); fs.writeFileSync(destination, doc.text);
    }
    if (name === "ownership") fs.writeFileSync(path.join(family, "schema.json"),
      JSON.stringify(oracle.metadataArtifact(spec.oracle.schema)));
  }
  const settings = { "vela.trace.server": "verbose", "vela.host.schema": "",
    "vela.workspace.roots": [path.join(familyFor(root, "declarations", false), "scripts")],
    "files.autoSave": "off", "chat.disableAIFeatures": true,
    "workbench.startupEditor": "none", "workbench.secondarySideBar.defaultVisibility": "hidden" };
  const file = path.join(root, "workspace-symbols.code-workspace");
  fs.writeFileSync(file, JSON.stringify({ folders: [{ path: workspace }], settings }));
  return file;
}

class WorkspaceSymbolSession {
  constructor(vscode, root) {
    this.vscode = vscode; this.root = root; this.workspace = workspaceFor(root);
    this.receipts = []; this.owned = new Map();
  }
  uri(file) { return this.vscode.Uri.file(path.join(this.family, safeFile(file))); }
  rows() { return readTrace(this.workspace); }
  async bounded(label, action) {
    let timer;
    try { return await Promise.race([action(), new Promise((_, reject) => {
      timer = setTimeout(() => reject(Error(`${label}: timed out after 15000ms`)), 15000);
    })]); } finally { clearTimeout(timer); }
  }
  async until(label, observe) {
    const deadline = Date.now() + 15000;
    while (Date.now() < deadline) {
      const value = observe(); if (value) return value;
      await new Promise(resolve => setTimeout(resolve, 25));
    }
    throw Error(`${label}: no complete installed-client evidence`);
  }
  async mutation(boundary, since, method, uri, quietMs = 0) {
    const receipt = await this.until(method, () => {
      const rows = this.rows().slice(boundary);
      if (!rows.some(row => row.event === "message_received" && row.method === method &&
          (!uri || row.documentUri === uri.toString()))) return false;
      return workspaceReadiness(rows, { since, now: Date.now(), quietMs });
    });
    this.receipts.push({ mutation: method, uri: uri?.toString(), receipt });
  }
  async startup() {
    this.family = familyFor(this.root, "declarations", false);
    const uri = this.uri("scripts/api.vela");
    await this.bounded("open startup editor", () => this.vscode.commands.executeCommand("vscode.open", uri, { preview: false }));
    const doc = this.vscode.window.activeTextEditor?.document;
    assert.equal(doc?.uri.toString(), uri.toString()); this.owned.set("scripts/api.vela", doc);
    // Startup alone may retry until the real installed provider is registered.
    const deadline = Date.now() + 14000;
    await this.bounded("workspace provider startup", async () => {
      while (Date.now() < deadline) {
        const rows = await this.vscode.commands.executeCommand("vscode.executeWorkspaceSymbolProvider", "");
        if (rows?.some(row => row.name === "api::choose")) return;
        await new Promise(resolve => setTimeout(resolve, 100));
      }
      throw Error("installed workspace symbol provider did not become ready");
    });
    const extension = this.vscode.extensions.getExtension("vela-lang.vela-vscode");
    assert(extension?.isActive); const relative = path.relative(process.env.VELA_TEST_EXTENSIONS_DIR, extension.extensionPath);
    assert(relative && !relative.startsWith("..") && !path.isAbsolute(relative), "actual installed VSIX");
    assert.equal(this.vscode.workspace.getConfiguration("vela").get("server.path"), "", "bundled server");
    assert.equal(this.vscode.workspace.workspaceFolders.length, 1);
    assert.equal(this.vscode.workspace.workspaceFolders[0].uri.fsPath, this.vscode.Uri.file(this.workspace).fsPath);
    this.trace = findLog(this.root, name => name.endsWith("-Vela LSP Trace.log"));
    await this.closeAll();
  }
  async setting(key, value) {
    if (isDeepStrictEqual(this.vscode.workspace.getConfiguration("vela").get(key), value)) return;
    const boundary = this.rows().length, since = Date.now();
    await this.vscode.workspace.getConfiguration("vela").update(key, value, this.vscode.ConfigurationTarget.Workspace);
    await this.mutation(boundary, since, "workspace/didChangeConfiguration", undefined, 750);
    assert.deepEqual(this.vscode.workspace.getConfiguration("vela").get(key), value);
  }
  async select(name, crlf) {
    assert.equal(this.owned.size, 0, "no open scratch from previous global corpus");
    assert(this.vscode.workspace.textDocuments.filter(doc => doc.languageId === "vela").every(doc => doc.isClosed),
      "fresh isolated session contains no other live Vela documents");
    this.family = familyFor(this.root, name, crlf); this.crlf = crlf;
    await this.setting("workspace.roots", [path.join(this.family, "scripts")]);
    await this.setting("host.schema", name === "ownership" ? path.join(this.family, "schema.json") : "");
  }
  async replace(file, wanted) {
    const boundary = this.rows().length, since = Date.now();
    await this.bounded(`open ${file}`, () => this.vscode.commands.executeCommand("vscode.open", this.uri(file), { preview: false }));
    const editor = this.vscode.window.activeTextEditor, doc = editor?.document;
    assert.equal(doc?.uri.toString(), this.uri(file).toString()); this.owned.set(file, doc);
    await this.mutation(boundary, since, "textDocument/didOpen", doc.uri);
    const changed = this.rows().length, changedAt = Date.now();
    assert(await editor.edit(edit => {
      edit.replace(new this.vscode.Range(doc.positionAt(0), doc.positionAt(doc.getText().length)), wanted.text);
      edit.setEndOfLine(this.crlf ? this.vscode.EndOfLine.CRLF : this.vscode.EndOfLine.LF);
    }));
    assert(doc.isDirty); assert.equal(doc.getText(), wanted.text);
    assert.equal(doc.eol, this.crlf ? this.vscode.EndOfLine.CRLF : this.vscode.EndOfLine.LF);
    await this.mutation(changed, changedAt, "textDocument/didChange", doc.uri);
  }
  async closeAll() {
    for (const [file, doc] of this.owned) {
      await this.vscode.window.showTextDocument(doc);
      const boundary = this.rows().length, since = Date.now();
      await this.bounded(`close ${file}`, () => this.vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor"));
      await this.until(`closed ${file}`, () => doc.isClosed);
      await this.mutation(boundary, since, "textDocument/didClose", doc.uri);
      this.owned.delete(file);
    }
  }
  disk(spec) {
    for (const [file, doc] of Object.entries(documents(spec, this.crlf)))
      assert.equal(fs.readFileSync(this.uri(file).fsPath, "utf8"), doc.text, `unchanged physical ${file}`);
    if (spec.oracle.schema) assert.deepEqual(JSON.parse(fs.readFileSync(path.join(this.family, "schema.json"), "utf8")),
      oracle.metadataArtifact(spec.oracle.schema));
    assert(!fs.existsSync(path.join(this.workspace, "vela.toml")), "no manifest overriding owned fallback roots");
    assert(!fs.existsSync(path.join(this.family, "vela.toml")));
  }
  async verify(spec, docs, label) {
    this.disk(spec);
    for (const [file, doc] of this.owned) assert.equal(doc.getText(), docs[file].text);
    for (const query of spec.oracle.queries) {
      const wire = oracle.expected(spec, docs, file => this.uri(file).toString(), query);
      const wanted = oracle.ordered(oracle.editorExpected(wire));
      const observed = [], pairs = [];
      for (let repeat = 0; repeat < 3; repeat++) {
        const boundary = fs.readFileSync(this.trace, "utf8").length;
        const symbols = await this.bounded(`${label}/${query.id}`, () =>
          this.vscode.commands.executeCommand("vscode.executeWorkspaceSymbolProvider", query.query));
        assert(Array.isArray(symbols), "real public workspace provider returns a complete array");
        const actual = oracle.ordered(symbols.map(row => ({ name: row.name, kind: row.kind,
          containerName: row.containerName, uri: row.location.uri.toString(),
          range: [row.location.range.start.line, row.location.range.start.character,
            row.location.range.end.line, row.location.range.end.character] })));
        assert.deepEqual(actual, wanted, `${label}/${query.id}: complete editor multiset repeat${repeat}`);
        const completed = await this.until(`${label}/${query.id}: fresh wire pair`, () => {
          const pairs = workspaceSymbolResponses(fs.readFileSync(this.trace, "utf8").slice(boundary));
          return pairs.length ? pairs : null;
        });
        assert.equal(completed.length, 1, "one fresh public command request");
        assert.deepEqual(completed[0].params, { query: query.query }, "exact authored query");
        // Rust URL and VS Code URI encode Windows drive letters differently.
        // Normalize only URI spelling through the actual client URI type;
        // every row/field/order/range and original raw response is retained.
        const actualWire = oracle.normalizeWire(completed[0].result, uri => this.vscode.Uri.parse(uri).toString());
        assert.deepEqual(actualWire, wire, `${label}/${query.id}: entire ordered wire rows, including URI-only schema and explicit []`);
        observed.push(actual); pairs.push(completed[0]);
        this.disk(spec); for (const [file, doc] of this.owned) assert.equal(doc.getText(), docs[file].text);
      }
      this.receipts.push({ label, query, expectedWire: wire, expectedEditor: wanted, observed, pairs, repeats: 3 });
    }
  }
  async finish() {
    try { await this.closeAll(); }
    finally {
      fs.writeFileSync(path.join(this.root, "workspace-symbol-observations.json"), JSON.stringify(this.receipts, null, 2));
      if (this.trace) fs.copyFileSync(this.trace, path.join(this.root, "workspace-symbol-client.log"));
      const trace = path.join(this.workspace, ".vela-lsp-trace.jsonl");
      if (fs.existsSync(trace)) fs.copyFileSync(trace, path.join(this.root, "workspace-symbol-server.jsonl"));
    }
  }
}

async function runWorkspaceSymbols(vscode) {
  const session = new WorkspaceSymbolSession(vscode, process.env.VELA_TEST_RESULT_DIR); let failure;
  try {
    await session.startup();
    for (const [name, spec] of Object.entries(specs)) for (const crlf of [false, true]) {
      await session.select(name, crlf);
      const base = documents(spec, crlf), dirty = documents(spec, crlf, true);
      await session.verify(spec, base, `${name}/${crlf ? "crlf" : "lf"}/disk`);
      for (const [file, wanted] of Object.entries(dirty)) await session.replace(file, wanted);
      await session.verify(spec, dirty, `${name}/${crlf ? "crlf" : "lf"}/dirty`);
      await session.closeAll();
      await session.verify(spec, base, `${name}/${crlf ? "crlf" : "lf"}/closed-restored`);
    }
  } catch (error) { failure = error; throw error; }
  finally {
    try { await session.finish(); }
    catch (cleanup) { if (failure) throw new AggregateError([failure, cleanup], `${failure.stack}\nCleanup also failed: ${cleanup.stack}`); throw cleanup; }
  }
}
module.exports = { checkName, materializeWorkspaceSymbols, runWorkspaceSymbols, workspaceFor };
