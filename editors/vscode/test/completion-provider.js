"use strict";
const assert = require("node:assert/strict");
const { completionModel } = require("../../../scripts/lsp-matrix/completion-contracts");

async function runCompletionProvider(vscode, workspace) {
  for (const [route, crlf] of [["accept-enter", false], ["accept-tab", true]]) {
    const model = completionModel(route), oracle = model.spec.oracle;
    const uri = vscode.Uri.joinPath(workspace, model.file);
    const document = await vscode.workspace.openTextDocument(uri);
    const editor = await vscode.window.showTextDocument(document);
    const source = crlf ? model.typed.text.replaceAll("\n", "\r\n") : model.typed.text;
    assert.ok(await editor.edit((builder) => {
      builder.replace(new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length)), source);
      builder.setEndOfLine(crlf ? vscode.EndOfLine.CRLF : vscode.EndOfLine.LF);
    }));
    assert.equal(document.getText(), source);
    const cursor = model.typed.selections[0].active;
    const query = (resolveCount) => vscode.commands.executeCommand("vscode.executeCompletionItemProvider", uri,
      new vscode.Position(cursor.line, cursor.character), undefined, resolveCount);
    const initial = await query(0);
    assert.deepEqual(initial.items.map((item) => item.label.label ?? item.label).sort(), [...oracle.labels].sort());
    assert.ok(initial.items.every((item) => item.documentation === undefined), "docs must be lazy");
    const resolved = await query(2);
    assert.deepEqual(resolved.items.map((item) => item.label.label ?? item.label).sort(), [...oracle.labels].sort());
    for (const item of resolved.items) {
      const label = item.label.label ?? item.label;
      assert.equal(item.kind, vscode.CompletionItemKind.Function);
      assert.equal(item.detail, oracle.detail);
      assert.ok(item.insertText instanceof vscode.SnippetString);
      assert.equal(item.insertText.value, `${label}($0)`);
      assert.deepEqual([item.range.start.line, item.range.start.character, item.range.end.line, item.range.end.character],
        [model.range.start.line, model.range.start.character, model.range.end.line, model.range.end.character + oracle.typedText.length]);
      assert.equal(item.documentation.value, label === oracle.labels[1] ? oracle.documentation : "Alpha completion documentation.");
    }
    assert.equal(document.getText(), source, "resolving completion must not edit source");
    assert.ok(document.isDirty);
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
  }
}
module.exports = { runCompletionProvider };
