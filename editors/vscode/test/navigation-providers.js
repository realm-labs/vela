"use strict";
const assert = require("node:assert/strict");
const { parseMarkers } = require("../../../scripts/lsp-matrix/fixtures");
const spec = require("../../../tests/lsp_matrix/fixtures/input-navigation.json");

async function runNavigationProvider(vscode, workspace, kind) {
  const type = kind === "type";
  const command = type ? "vscode.executeTypeDefinitionProvider" : "vscode.executeDeclarationProvider";
  const targetFile = type ? spec.oracle.typeFile : spec.oracle.definitionFile;
  const targetMarker = type ? "type" : "definition";
  const mainUri = vscode.Uri.joinPath(workspace, spec.oracle.caller);
  const targetUri = vscode.Uri.joinPath(workspace, targetFile);
  const main = await vscode.workspace.openTextDocument(mainUri);
  for (const crlf of [false, true]) {
    const source = (file) => parseMarkers(("// provider 中😀\n" + spec.files[file]).replaceAll("\n", crlf ? "\r\n" : "\n"));
    const caller = source(spec.oracle.caller), target = source(targetFile);
    const replace = async (document, text) => {
      const editor = await vscode.window.showTextDocument(document);
      assert.ok(await editor.edit((builder) => {
        builder.replace(new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length)), text);
        builder.setEndOfLine(crlf ? vscode.EndOfLine.CRLF : vscode.EndOfLine.LF);
      }));
      assert.equal(document.getText(), text);
      assert.ok(document.isDirty);
    };
    await replace(main, caller.text);
    const destination = await vscode.workspace.openTextDocument(targetUri);
    await replace(destination, target.text);
    for (const marker of type ? ["call", "typed-use", "unknown"] : ["call", "unknown"]) {
      const cursor = caller.markers[marker].start;
      const targets = await vscode.commands.executeCommand(command, mainUri, new vscode.Position(cursor.line, cursor.character));
      if (marker === "unknown") assert.equal(targets?.length ?? 0, 0, "unknown names must not borrow a target");
      else {
        assert.equal(targets?.length, 1);
        const location = targets[0], range = location.targetSelectionRange ?? location.range;
        assert.equal((location.targetUri ?? location.uri).toString(), targetUri.toString());
        const expected = target.markers[targetMarker];
        assert.deepEqual([range.start.line, range.start.character, range.end.line, range.end.character],
          [expected.start.line, expected.start.character, expected.end.line, expected.end.character]);
        assert.equal(destination.getText(range), type ? "Item" : "make");
      }
      assert.equal(main.getText(), caller.text, "provider must not edit caller");
      assert.equal(destination.getText(), target.text, "provider must not edit target");
    }
    await vscode.window.showTextDocument(destination);
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
  }
  await vscode.window.showTextDocument(main);
  await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
}
module.exports = { runNavigationProvider };
