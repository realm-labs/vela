"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { safeFile } = require("../../../scripts/lsp-matrix/fixtures");
const { readTrace, workspaceReadiness } = require("./input/readiness");
const { findLog } = require("./input/logs");
class EditorDocumentSession {
  constructor(vscode, workspace, root, crlf, prefix = "symbols") {
    this.prefix = prefix;
    this.vscode = vscode; this.workspace = workspace; this.root = root; this.crlf = crlf;
    this.originalTrace = vscode.workspace.getConfiguration("vela").inspect("trace.server").workspaceValue;
    this.firstFolder = vscode.workspace.workspaceFolders[0].uri.toString();
    this.receipts = [];
    this.trace = findLog(process.env.VELA_TEST_RESULT_DIR, name => name.endsWith("-Vela LSP Trace.log"));
    this.modelStates = new Map();
  }
  uri(file) { return this.vscode.Uri.file(path.join(this.root, safeFile(file))); }
  rows() { return readTrace(this.workspace.fsPath); }
  async bounded(label, action) {
    let timer;
    try {
      return await Promise.race([action(), new Promise((_, reject) => {
        timer = setTimeout(() => reject(Error(`${label}: timed out after 15000ms`)), 15000);
      })]);
    } finally { clearTimeout(timer); }
  }
  async until(label, observe) {
    const deadline = Date.now() + 15000;
    while (Date.now() < deadline) {
      const value = observe(); if (value) return value;
      await new Promise(resolve => setTimeout(resolve, 25));
    }
    throw Error(`${label}: no complete installed-client mutation`);
  }
  sameUri(text, uri) {
    if (!text) return false;
    const actual = this.vscode.Uri.parse(text).fsPath, wanted = uri.fsPath;
    return process.platform === "win32" ? actual.toLowerCase() === wanted.toLowerCase() : actual === wanted;
  }
  async mutation(boundary, since, method, uri, quietMs = 750) {
    const receipt = await this.until(method, () => {
      const rows = this.rows().slice(boundary);
      if (!rows.some(row => row.event === "message_received" && row.method === method &&
          (!uri || this.sameUri(row.documentUri, uri)))) return false;
      return workspaceReadiness(rows, { since, now: Date.now(), quietMs });
    });
    this.receipts.push({ mutation: method, uri: uri?.toString(), receipt });
  }
  async setting(key, value) {
    if (Object.is(this.vscode.workspace.getConfiguration("vela").inspect(key).workspaceValue, value)) return;
    const boundary = this.rows().length, since = Date.now();
    await this.vscode.workspace.getConfiguration("vela").update(key, value,
      this.vscode.ConfigurationTarget.Workspace);
    await this.mutation(boundary, since, "workspace/didChangeConfiguration");
  }
  async addRoot() {
    const folders = this.vscode.workspace.workspaceFolders;
    assert.equal(folders[0].uri.toString(), this.firstFolder, "preserve first folder and extension host");
    assert(!folders.some(folder => this.sameUri(folder.uri.toString(), this.vscode.Uri.file(this.root))));
    const boundary = this.rows().length, since = Date.now();
    assert(this.vscode.workspace.updateWorkspaceFolders(folders.length, 0,
      { uri: this.vscode.Uri.file(this.root), name: `${this.prefix}-${path.basename(this.root)}` }));
    await this.mutation(boundary, since, "workspace/didChangeWorkspaceFolders");
    await this.setting("trace.server", "verbose");
  }
  async removeRoot() {
    const folders = this.vscode.workspace.workspaceFolders;
    const index = folders.findIndex(folder => this.sameUri(folder.uri.toString(), this.vscode.Uri.file(this.root)));
    if (index < 0) return;
    assert(index > 0, "test root never replaces the user's first folder");
    const boundary = this.rows().length, since = Date.now();
    assert(this.vscode.workspace.updateWorkspaceFolders(index, 1));
    await this.mutation(boundary, since, "workspace/didChangeWorkspaceFolders");
    assert.equal(this.vscode.workspace.workspaceFolders[0].uri.toString(), this.firstFolder);
  }
  async open(file) {
    const uri = this.uri(file);
    const previous = this.vscode.workspace.textDocuments.find(doc => !doc.isClosed && this.sameUri(doc.uri.toString(), uri));
    const boundary = fs.readFileSync(this.trace, "utf8").length;
    await this.bounded(`open ${file}`, () => this.vscode.commands.executeCommand("vscode.open", uri, { preview: false }));
    const current = this.vscode.window.activeTextEditor?.document;
    assert(current && this.sameUri(current.uri.toString(), uri), `actual owned editor opens ${file}`);
    if (!previous) this.modelStates.set(file, { boundary, version: current.version });
    return current;
  }
  async replace(file, doc) {
    const current = await this.open(file), editor = await this.vscode.window.showTextDocument(current);
    const boundary = fs.readFileSync(this.trace, "utf8").length;
    assert(await editor.edit(builder => {
      builder.replace(new this.vscode.Range(current.positionAt(0), current.positionAt(current.getText().length)), doc.text);
      builder.setEndOfLine(this.crlf ? this.vscode.EndOfLine.CRLF : this.vscode.EndOfLine.LF);
    }));
    assert.equal(current.getText(), doc.text, "exact edited Unicode source");
    assert.equal(current.eol, this.crlf ? this.vscode.EndOfLine.CRLF : this.vscode.EndOfLine.LF);
    this.modelStates.set(file, { boundary, version: current.version });
    return current;
  }
  async close(file) {
    const uri = this.uri(file);
    const current = this.vscode.workspace.textDocuments.find(doc => doc.uri.toString() === uri.toString() && !doc.isClosed);
    if (!current) return;
    await this.vscode.window.showTextDocument(current);
    const boundary = this.rows().length, since = Date.now();
    await this.bounded(`close ${file}`, () => this.vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor"));
    await this.until(`closed ${file}`, () => current.isClosed);
    await this.mutation(boundary, since, "textDocument/didClose", uri, 0);
    this.modelStates.delete(file);
  }
  async write(file, text) {
    const boundary = this.rows().length, since = Date.now();
    if (text === null) fs.unlinkSync(this.uri(file).fsPath);
    else fs.writeFileSync(this.uri(file).fsPath, text);
    await this.mutation(boundary, since, "workspace/didChangeWatchedFiles");
  }
  async finish() {
    try {
      for (const current of [...this.vscode.workspace.textDocuments]) {
        const relative = path.relative(this.root, current.uri.fsPath);
        if (relative && !relative.startsWith("..") && !path.isAbsolute(relative) && !current.isClosed) {
          await this.close(relative.split(path.sep).join("/"));
        }
      }
      await this.removeRoot();
      await this.setting("trace.server", this.originalTrace);
    } finally {
      fs.writeFileSync(path.join(process.env.VELA_TEST_RESULT_DIR, `${this.prefix}-${path.basename(this.root)}-observations.json`),
        JSON.stringify(this.receipts, null, 2));
      const clientLog = findLog(process.env.VELA_TEST_RESULT_DIR, name => name.endsWith("-Vela LSP Trace.log"));
      fs.copyFileSync(clientLog, path.join(process.env.VELA_TEST_RESULT_DIR, `${this.prefix}-${path.basename(this.root)}-client.log`));
    }
  }
}
module.exports = { EditorDocumentSession };
