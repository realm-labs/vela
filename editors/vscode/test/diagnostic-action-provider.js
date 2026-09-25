"use strict";

const assert = require("node:assert/strict");
const vscode = require("vscode");
const { parseMarkers } = require("../../../scripts/lsp-matrix/fixtures");
const spec = require("../../../tests/lsp_matrix/fixtures/diagnostic-action-method-typo.json");

const file = "scripts/game/main.vela";
const original = parseMarkers(spec.files[file]);
const applied = parseMarkers(spec.oracle.applied);
const coordinates = (range) => [range.start.line, range.start.character, range.end.line, range.end.character];
const expectedRange = (marker) => {
  const value = original.markers[marker];
  return [value.start.line, value.start.character, value.end.line, value.end.character];
};
const targetRange = (marker) => {
  const [startLine, startCharacter, endLine, endCharacter] = expectedRange(marker);
  return new vscode.Range(startLine, startCharacter, endLine, endCharacter);
};
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function initialDiagnostics(uri) {
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    const values = vscode.languages.getDiagnostics(uri);
    if (values.length === spec.oracle.diagnostics.length) return values;
    await delay(50);
  }
  throw new Error("initial diagnostic publication timed out");
}

async function changedDiagnostics(uri, edit) {
  let timer, subscription;
  const changed = new Promise((resolve, reject) => {
    timer = setTimeout(() => reject(new Error("updated diagnostic publication timed out")), 10000);
    subscription = vscode.languages.onDidChangeDiagnostics((event) => {
      if (event.uris.some((item) => item.toString() === uri.toString())) {
        resolve(vscode.languages.getDiagnostics(uri));
      }
    });
  });
  try {
    assert.ok(await edit(), "source edit must apply");
    return await changed;
  } finally {
    clearTimeout(timer);
    subscription.dispose();
  }
}

function checkDiagnostics(actual, markers) {
  const observed = actual.map((diagnostic) => ({
    code: typeof diagnostic.code === "object" ? diagnostic.code.value : diagnostic.code,
    message: diagnostic.message,
    severity: diagnostic.severity,
    range: coordinates(diagnostic.range),
    related: diagnostic.relatedInformation?.length ?? 0,
  })).sort((left, right) => left.range[0] - right.range[0]);
  const expected = markers.map((marker) => {
    const diagnostic = spec.oracle.diagnostics.find((item) => item.marker === marker);
    assert.ok(diagnostic, `missing oracle for ${marker}`);
    return {
      code: diagnostic.code,
      message: diagnostic.message,
      severity: vscode.DiagnosticSeverity.Error,
      range: expectedRange(marker),
      related: 0,
    };
  });
  assert.deepEqual(observed, expected, "complete diagnostic set");
}

async function open(vscode, workspace) {
  const uri = vscode.Uri.joinPath(workspace, file);
  const document = await vscode.workspace.openTextDocument(uri);
  const editor = await vscode.window.showTextDocument(document);
  assert.equal(document.getText(), original.text, "installed fixture source");
  checkDiagnostics(await initialDiagnostics(uri), ["fix", "unrelated"]);
  return { uri, document, editor };
}

async function checkDiagnosticProvider(vscode, workspace) {
  const { uri, document, editor } = await open(vscode, workspace);
  const remaining = await changedDiagnostics(uri, () => editor.edit((builder) =>
    builder.replace(targetRange("fix"), "first")));
  assert.equal(document.getText(), applied.text, "repair preserves unrelated source");
  assert.ok(document.isDirty, "repair is not saved");
  checkDiagnostics(remaining, ["unrelated"]);
  await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
}

async function checkCodeActionProvider(vscode, workspace) {
  const { uri, document } = await open(vscode, workspace);
  const actions = await vscode.commands.executeCommand("vscode.executeCodeActionProvider", uri, targetRange("fix"));
  assert.deepEqual(actions?.map((item) => item.title), spec.oracle.actionTitles, "exact quick-fix candidates");
  for (const [index, action] of actions.entries()) {
    assert.equal(action.kind?.value, vscode.CodeActionKind.QuickFix.value);
    assert.ok(action.edit instanceof vscode.WorkspaceEdit, "quick fix owns a workspace edit");
    const [entry] = action.edit.entries();
    assert.equal(action.edit.entries().length, 1, "only the target document changes");
    assert.equal(entry[0].toString(), uri.toString(), "client URI");
    assert.equal(entry[1].length, 1, "one exact replacement");
    assert.deepEqual(coordinates(entry[1][0].range), expectedRange("fix"));
    assert.equal(entry[1][0].newText, spec.oracle.actionReplacements[index]);
  }
  const selected = actions.find((action) => action.title === spec.oracle.action.title);
  assert.ok(selected, "selected quick fix");
  const remaining = await changedDiagnostics(uri, () => vscode.workspace.applyEdit(selected.edit));
  assert.equal(document.getText(), applied.text, "applied fix changes only the typo");
  assert.ok(document.isDirty, "applied fix is not saved");
  checkDiagnostics(remaining, ["unrelated"]);
  const fixed = await vscode.commands.executeCommand("vscode.executeCodeActionProvider", uri,
    new vscode.Range(applied.markers.fix.start.line, applied.markers.fix.start.character,
      applied.markers.fix.end.line, applied.markers.fix.end.character));
  assert.deepEqual(fixed, [], "repaired location offers no fix");
  await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
}

module.exports = { checkDiagnosticProvider, checkCodeActionProvider };
