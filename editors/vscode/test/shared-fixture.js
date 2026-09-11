"use strict";
const assert = require("node:assert/strict");
const { FixtureWorkspace, parseMarkers } = require("../../../scripts/lsp-matrix/fixtures");
const spec = require("../../../tests/lsp_matrix/fixtures/shared-unicode-lifecycle.json");

async function replace(vscode, document, text) {
  const editor = await vscode.window.showTextDocument(document);
  assert.ok(await editor.edit((builder) => {
    builder.replace(new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length)), text);
    builder.setEndOfLine(text.includes("\r\n") ? vscode.EndOfLine.CRLF : vscode.EndOfLine.LF);
  }));
  assert.equal(document.getText(), text, "fixture setup must preserve exact line endings");
  return editor;
}

async function runSharedFixture(vscode, workspace) {
  const mainUri = vscode.Uri.joinPath(workspace, "scripts/main.vela");
  const helperUri = vscode.Uri.joinPath(workspace, "scripts/helper.vela");
  const main = await vscode.workspace.openTextDocument(mainUri);
  let helper = await vscode.workspace.openTextDocument(helperUri);
  const initial = new FixtureWorkspace(spec);
  for (const crlf of [false, true]) {
    const endings = (source) => crlf ? source.replaceAll("\n", "\r\n") : source;
    const caller = parseMarkers(endings(spec.files["scripts/main.vela"]));
    await replace(vscode, main, caller.text);
    await replace(vscode, helper, endings(initial.disk.get("scripts/helper.vela").text));
    const query = async (expected, targetText) => {
      const cursor = caller.markers.call.start;
      const targets = await vscode.commands.executeCommand("vscode.executeDefinitionProvider", mainUri,
        new vscode.Position(cursor.line, cursor.character));
      assert.equal(targets?.length, 1, JSON.stringify(targets));
      const target = targets[0], range = target.targetSelectionRange ?? target.range;
      assert.equal((target.targetUri ?? target.uri).toString(), helperUri.toString());
      assert.deepEqual([range.start.line,range.start.character,range.end.line,range.end.character],
        [expected.line,expected.character,expected.line,expected.endCharacter]);
      assert.equal(targetText.getText(range), spec.oracle.selectedText);
      assert.equal(main.getText(), caller.text, "navigation query must not edit caller");
    };
    await query(spec.oracle.afterEachAction[0], helper);
    const dirty = parseMarkers(endings(spec.actions[2].source));
    await replace(vscode, helper, dirty.text);
    assert.ok(helper.isDirty);
    await query(spec.oracle.afterEachAction[2], helper);
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    // Observe the disk restoration through the installed language client before
    // reopening the target. The expected range comes from the fixture, not a provider.
    const restored = await vscode.commands.executeCommand("vscode.executeDefinitionProvider", mainUri,
      new vscode.Position(caller.markers.call.start.line,caller.markers.call.start.character));
    const range = restored?.[0]?.targetSelectionRange ?? restored?.[0]?.range;
    assert.ok(range, "closed target should still resolve from disk");
    assert.deepEqual([range.start.line,range.start.character,range.end.line,range.end.character],[0,7,0,16]);
    helper = await vscode.workspace.openTextDocument(helperUri);
    assert.equal(helper.getText(), initial.disk.get("scripts/helper.vela").text);
    await query(spec.oracle.afterEachAction[0],helper);
  }
  await vscode.window.showTextDocument(main);
  await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
}
module.exports = { runSharedFixture };
