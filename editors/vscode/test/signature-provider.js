"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { FixtureWorkspace, parseMarkers } = require("../../../scripts/lsp-matrix/fixtures");
const spec = require("../../../tests/lsp_matrix/fixtures/signature-editor.json");

function materializeSignature(workspace) {
  for (const [file, document] of new FixtureWorkspace(spec).disk) {
    const target = path.join(workspace, file);
    assert.ok(!fs.existsSync(target), "signature fixture must not overwrite existing files");
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, document.text);
  }
}

function result(help) {
  if (help == null) return null;
  return {
    activeSignature: help.activeSignature,
    activeParameter: help.activeParameter,
    signatures: help.signatures.map(signature => ({
      label: signature.label,
      parameters: signature.parameters.map(parameter => ({ label: parameter.label }))
    }))
  };
}

async function verify(vscode, document, source, queries) {
  assert.equal(document.getText(), source.text);
  for (const query of queries) {
    const point = source.markers[query.marker].start;
    const read = () => vscode.commands.executeCommand("vscode.executeSignatureHelpProvider", document.uri,
      new vscode.Position(point.line, point.character));
    const actual = result(await read());
    assert.deepEqual(actual, query.expected, query.id);
    assert.deepEqual(result(await read()), actual, `${query.id} repeated signature`);
    assert.equal(document.getText(), source.text, "signature queries preserve buffer bytes");
  }
  return queries.length;
}

async function replace(vscode, editor, source, crlf) {
  const document = editor.document;
  assert.ok(await editor.edit(builder => {
    builder.replace(new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length)), source.text);
    builder.setEndOfLine(crlf ? vscode.EndOfLine.CRLF : vscode.EndOfLine.LF);
  }));
  assert.equal(document.getText(), source.text);
}

async function runSignatureProvider(vscode, workspace) {
  let total = 0;
  const file = spec.oracle.file, uri = vscode.Uri.joinPath(workspace, file);
  const disk = new FixtureWorkspace(spec).disk;
  const unchangedDisk = () => {
    for (const [file, source] of disk) assert.equal(fs.readFileSync(vscode.Uri.joinPath(workspace, file).fsPath, "utf8"), source.text);
  };
  for (const crlf of [false, true]) {
    const source = text => parseMarkers(crlf ? text.replaceAll("\n", "\r\n") : text);
    let document = await vscode.workspace.openTextDocument(uri);
    let editor = await vscode.window.showTextDocument(document);
    const initial = source(spec.files[file]);
    await replace(vscode, editor, initial, crlf);
    total += await verify(vscode, document, initial, spec.oracle.queries);
    const shifted = source("// shifted 中😀\n/* second 😀 */\n" + spec.files[file]);
    await replace(vscode, editor, shifted, crlf);
    assert.ok(document.isDirty);
    total += await verify(vscode, document, shifted, spec.oracle.queries);
    const recovery = source(spec.oracle.recovery.source);
    await replace(vscode, editor, recovery, crlf);
    total += await verify(vscode, document, recovery, [...spec.oracle.queries, spec.oracle.recovery.query]);
    unchangedDisk();
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    document = await vscode.workspace.openTextDocument(uri);
    editor = await vscode.window.showTextDocument(document);
    assert.ok(!document.isDirty);
    total += await verify(vscode, document, disk.get(file), spec.oracle.queries);
    unchangedDisk();
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
  }
  assert.equal(total, 170, "all authored positions at both line endings and every state must run");
}

module.exports = { materializeSignature, runSignatureProvider };
