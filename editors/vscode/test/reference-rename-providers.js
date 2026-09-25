"use strict";

const assert = require("node:assert/strict");
const { parseMarkers } = require("../../../scripts/lsp-matrix/fixtures");
const references = require("../../../tests/lsp_matrix/fixtures/input-references.json");
const rename = require("../../../tests/lsp_matrix/fixtures/input-rename.json");

const coords = (value) => [value.start.line, value.start.character, value.end.line, value.end.character];
const expected = (spec, file, marker) => parseMarkers(spec.files[file]).markers[marker];
const uri = (vscode, workspace, file) => vscode.Uri.joinPath(workspace, file);
const cursor = (vscode, spec, file, marker) => {
  const site = expected(spec, file, marker).start;
  return new vscode.Position(site.line, site.character + 1);
};
const siteKey = (file, value) => `${file}:${coords(value).join(":")}`;
const sourceText = (spec, file) => parseMarkers(spec.files[file]).text;

async function checkReferences(vscode, workspace) {
  const file = references.oracle.openFile;
  const origin = uri(vscode, workspace, file);
  const document = await vscode.workspace.openTextDocument(origin);
  assert.equal(document.getText(), sourceText(references, file));
  const sites = await vscode.commands.executeCommand(
    "vscode.executeReferenceProvider", origin, cursor(vscode, references, file, "call")
  );
  const actual = (sites || []).map((location) => siteKey(location.uri.toString(), location.range)).sort();
  const wanted = references.oracle.referenceSites.map(({ file: target, marker }) =>
    siteKey(uri(vscode, workspace, target).toString(), expected(references, target, marker))).sort();
  assert.deepEqual(actual, wanted, "reference provider must return the complete cross-file set");
  for (const location of sites) {
    const target = await vscode.workspace.openTextDocument(location.uri);
    assert.equal(target.getText(location.range), "grant");
  }
  assert.equal(document.getText(), sourceText(references, file));
}

async function checkHighlights(vscode, workspace) {
  const file = references.oracle.openFile;
  const target = uri(vscode, workspace, file);
  const document = await vscode.workspace.openTextDocument(target);
  const kinds = {
    "local-decl": vscode.DocumentHighlightKind.Text,
    "local-write": vscode.DocumentHighlightKind.Write,
    "local-read": vscode.DocumentHighlightKind.Read,
    "shadow-decl": vscode.DocumentHighlightKind.Text,
    "shadow-read": vscode.DocumentHighlightKind.Read,
  };
  for (const [cursorMarker, markers] of [
    ["local-write", references.oracle.localSites],
    ["local-read", references.oracle.localSites],
    ["shadow-read", references.oracle.shadowSites],
  ]) {
    const highlights = await vscode.commands.executeCommand(
      "vscode.executeDocumentHighlights", target, cursor(vscode, references, file, cursorMarker)
    );
    const actual = (highlights || []).map((item) => `${coords(item.range).join(":")}:${item.kind}`).sort();
    const wanted = markers.map((marker) => `${coords(expected(references, file, marker)).join(":")}:${kinds[marker]}`).sort();
    assert.deepEqual(actual, wanted, `wrong highlight set for ${cursorMarker}`);
    for (const item of highlights) assert.equal(document.getText(item.range), "score");
  }
  assert.equal(document.getText(), sourceText(references, file));
}

async function checkPrepareRename(vscode, workspace) {
  const file = rename.oracle.openFile;
  const target = uri(vscode, workspace, file);
  const document = await vscode.workspace.openTextDocument(target);
  const prepared = await vscode.commands.executeCommand(
    "vscode.prepareRename", target, cursor(vscode, rename, file, "cursor")
  );
  assert.ok(prepared, "source call must be renameable");
  assert.deepEqual(coords(prepared.range ?? prepared), coords(expected(rename, file, "cursor")));
  if (prepared.placeholder !== undefined) assert.equal(prepared.placeholder, rename.oracle.source);
  const keyword = document.positionAt(document.getText().indexOf("fn call"));
  let rejected;
  try { rejected = await vscode.commands.executeCommand("vscode.prepareRename", target, keyword); }
  catch { rejected = undefined; }
  assert.equal(rejected, undefined, "a keyword cannot acquire a rename range");
  assert.equal(document.getText(), sourceText(rename, file));
}

async function checkRename(vscode, workspace) {
  const file = rename.oracle.openFile;
  const target = uri(vscode, workspace, file);
  const document = await vscode.workspace.openTextDocument(target);
  const replacement = rename.oracle.rename;
  let collision;
  try {
    collision = await vscode.commands.executeCommand(
      "vscode.executeDocumentRenameProvider", target, cursor(vscode, rename, file, "cursor"), "call"
    );
  } catch { collision = undefined; }
  assert.equal(collision, undefined, "colliding function name must not produce workspace edits");
  assert.equal(document.getText(), sourceText(rename, file), "rejected rename must preserve caller");
  const edit = await vscode.commands.executeCommand(
    "vscode.executeDocumentRenameProvider", target, cursor(vscode, rename, file, "cursor"), replacement
  );
  assert.ok(edit instanceof vscode.WorkspaceEdit, "rename must return a workspace edit");
  const actual = edit.entries().flatMap(([editedUri, edits]) => edits.map((entry) => ({
    site: siteKey(editedUri.toString(), entry.range), text: entry.newText,
  }))).sort((a, b) => a.site.localeCompare(b.site));
  const wanted = rename.oracle.sites.map(({ file: editedFile, marker }) => ({
    site: siteKey(uri(vscode, workspace, editedFile).toString(), expected(rename, editedFile, marker)),
    text: replacement,
  })).sort((a, b) => a.site.localeCompare(b.site));
  assert.deepEqual(actual, wanted, "workspace edit must target only the owned symbol");
  assert.ok(await vscode.workspace.applyEdit(edit));
  for (const editedFile of Object.keys(rename.files)) {
    const edited = await vscode.workspace.openTextDocument(uri(vscode, workspace, editedFile));
    const source = sourceText(rename, editedFile);
    const positions = rename.oracle.sites.filter((site) => site.file === editedFile)
      .map((site) => expected(rename, editedFile, site.marker));
    const offsets = positions.map((site) => [sourceOffset(source, site.start), sourceOffset(source, site.end)])
      .sort((a, b) => b[0] - a[0]);
    const expectedText = offsets.reduce((text, [start, end]) => text.slice(0, start) + replacement + text.slice(end), source);
    assert.equal(edited.getText(), expectedText, `applied rename changed unrelated source in ${editedFile}`);
  }
  assert.equal(document.getText().includes("let grant = 3; grant;"), true, "shadow must remain unchanged");
}

function sourceOffset(text, position) {
  const lines = text.split("\n");
  return lines.slice(0, position.line).reduce((offset, line) => offset + line.length + 1, 0) + position.character;
}

module.exports = { checkReferences, checkHighlights, checkPrepareRename, checkRename };
