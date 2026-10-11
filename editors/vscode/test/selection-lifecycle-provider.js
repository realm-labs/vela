"use strict";
const assert = require("node:assert/strict"), fs = require("node:fs"), path = require("node:path");
const oracle = require("../../../scripts/lsp-matrix/selection-editor-oracle");
const spec = require("../../../tests/lsp_matrix/fixtures/selection-lifecycle.json");
const { withSession } = require("./selection-provider");

async function runSelectionLifecycle(vscode, workspace) {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const variant = name => {
      const item = spec.oracle.variants[name]; assert(item, "literal lifecycle variant");
      return { item, doc: oracle.document(item.source, crlf, shifted) };
    };
    await withSession(vscode, workspace, "lifecycle", crlf, shifted, async session => {
      for (const phase of spec.oracle.phases) {
        for (let index = 0; index < phase.actions.length; index++) {
          const a = phase.actions[index], next = phase.actions[index + 1];
          if (a.op === "delete" && next?.op === "write" && next.file !== a.file) {
            const from = session.uri(a.file).fsPath, to = session.uri(next.file).fsPath;
            for (const file of [from, to]) {
              const relative = path.relative(session.root, file);
              assert(relative && !relative.startsWith("..") && !path.isAbsolute(relative));
            }
            assert.equal(session.disk(a.file), variant(next.variant).doc.text);
            assert.equal(session.disk(next.file), null);
            const boundary = session.rows().length, since = Date.now();
            fs.renameSync(from, to);
            await session.mutation(boundary, since, "workspace/didChangeWatchedFiles"); index++;
          } else if (a.op === "open") await session.openSettled(a.file);
          else if (a.op === "change") await session.replaceSettled(a.file, variant(a.variant).doc);
          else if (a.op === "close") await session.close(a.file);
          else if (a.op === "write") await session.write(a.file, variant(a.variant).doc.text);
          else if (a.op === "delete") await session.write(a.file, null);
          else if (a.op === "save") {
            const doc = await session.openSettled(a.file), boundary = session.rows().length, since = Date.now(), dirty = doc.isDirty;
            assert(await doc.save(), "real installed document save");
            if (dirty) await session.mutation(boundary, since, "workspace/didChangeWatchedFiles");
            assert.equal(session.disk(a.file), doc.getText(), "dirty/clean saves preserve exact model bytes");
          } else assert.fail("unknown finite lifecycle operation " + a.op);
        }
        for (const [file, name] of Object.entries(phase.disk)) {
          assert.equal(session.disk(file), name ? variant(name).doc.text : null, phase.id + "/disk/" + file);
        }
        for (const [file, name] of Object.entries(phase.views)) {
          if (!name) { await session.verifyAbsent(file, `${phase.id}/absent`); continue; }
          const { doc, item } = variant(name);
          const current = await session.openSettled(file);
          assert.equal(current.getText(), doc.text, `${phase.id}/source/${file}`);
          if (phase.open[file] && phase.disk[file] !== phase.open[file]) assert(current.isDirty);
          await session.verify(file, doc, item.queries, `${phase.id}/${name}`);
          await session.verify(file, doc, [], `${phase.id}/${name}/empty`);
          // Public providers require a live model. Restore closed owners after
          // each observation so subsequent watcher operations retain their role.
          if (!Object.hasOwn(phase.open, file)) await session.close(file);
        }
      }
    });
  }
}
module.exports = { runSelectionLifecycle };
