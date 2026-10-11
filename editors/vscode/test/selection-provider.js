"use strict";
const assert = require("node:assert/strict"), fs = require("node:fs"), path = require("node:path");
const { SelectionSession } = require("./selection-session");
const oracle = require("../../../scripts/lsp-matrix/selection-editor-oracle");
const spec = require("../../../tests/lsp_matrix/fixtures/selection-editor.json");
const lifecycle = require("../../../tests/lsp_matrix/fixtures/selection-lifecycle.json");
const rootFor = (resultRoot, kind, crlf, shifted) => path.join(resultRoot, `中文 % selection ${kind} roots`,
  `${crlf ? "crlf" : "lf"}-${shifted ? "shifted" : "original"}`);
function materializeSelection(resultRoot, kinds = ["syntax", "lifecycle"]) {
  for (const kind of kinds) assert(["syntax", "lifecycle"].includes(kind));
  for (const crlf of [false, true]) for (const shifted of [false, true]) for (const kind of kinds) {
    const root = rootFor(resultRoot, kind, crlf, shifted); assert(!fs.existsSync(root));
    fs.mkdirSync(path.join(root, "scripts"), { recursive: true });
    for (const [file, source] of Object.entries(kind === "syntax" ? spec.files : lifecycle.files)) {
      fs.writeFileSync(path.join(root, file), oracle.document(source, crlf, shifted).text);
    }
    fs.writeFileSync(path.join(root, "vela.toml"), `[package]\nid = 'dev.vela.selection.${kind}.${crlf}.${shifted}'\nname = 'selection_fixture'\nversion = '0.1.0'\n[source]\nroots = ['scripts']\n`);
  }
}
async function withSession(vscode, workspace, kind, crlf, shifted, run) {
  const session = new SelectionSession(vscode, workspace, rootFor(process.env.VELA_TEST_RESULT_DIR, kind, crlf, shifted),
    crlf, `selection-${kind}`);
  let failure;
  try { await session.addRoot(); await run(session); }
  catch (error) { failure = error; throw error; }
  finally {
    try { await session.finish(); }
    catch (cleanup) { if (failure) throw new AggregateError([failure, cleanup], `${failure.stack}\nCleanup also failed: ${cleanup.stack}`); throw cleanup; }
  }
}
async function runSelectionSyntax(vscode, workspace) {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    await withSession(vscode, workspace, "syntax", crlf, shifted, async session => {
      const document = s => oracle.document(s, crlf, shifted), main = spec.oracle.file;
      const initial = document(spec.files[main]), base = oracle.resolve(spec.oracle.baseline);
      await session.verify(main, initial, base.item.queries, "disk");
      let helperText = null;
      for (const ref of spec.oracle.cases) {
        const { fixture, item } = oracle.resolve(ref), doc = document(item.source);
        const helper = document(fixture.files["scripts/helper.vela"]);
        if (helper.text !== helperText) {
          await session.close("scripts/helper.vela");
          if (session.disk("scripts/helper.vela") !== helper.text) await session.write("scripts/helper.vela", helper.text);
          helperText = helper.text;
        }
        const current = await session.replaceSettled(main, doc); assert(current.isDirty);
        await session.verify(main, doc, item.queries, `dirty/${ref.id}`);
        await session.verify(main, doc, [], `empty/${ref.id}`);
        await session.verify("scripts/helper.vela", helper, fixture.oracle.helperQueries, `helper/${ref.id}`);
        assert.equal(session.disk(main), initial.text);
      }
      await session.close(main);
      await session.verify(main, initial, base.item.queries, "close-restored");
      await session.verify(main, initial, [], "close-restored-empty");
    });
  }
}
module.exports = { materializeSelection, runSelectionSyntax, rootFor, withSession };
