"use strict";
const fs = require("node:fs"), path = require("node:path"), assert = require("node:assert/strict");
const { fileURLToPath } = require("node:url"), { isDeepStrictEqual } = require("node:util");
const { offsetAt, safeFile } = require("../../../../scripts/lsp-matrix/fixtures");
const { invalidConfigSchemaModel } = require("../../../../scripts/lsp-matrix/invalid-config-schema-contracts");
const { hoverResponses } = require("../../../../scripts/lsp-matrix/hover-trace");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { relativeFile, canonicalUri, fileUri } = require("./paths");
const { readSession, sessionLog, snapshot } = require("./session");

// Normalize only the independently known owned base. Preserve the complete
// filename, error body, bytes, labels and all other diagnostic fields.
function ownedMessage(message, workspace, platform = process.platform) {
  const escaped = workspace.replaceAll("\\", "/").replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return message.replace(new RegExp(escaped + "(?=/)", platform === "win32" ? "gi" : "g"), "<workspace>");
}
async function runInvalidConfigSchema({ page, bridge, record, root, workspace, contracts, until, onProof, pid, platform }) {
  const native = require("./native-menu").nativeMenu({ root, platform, page, pid });
  for (const contract of contracts.filter(c => c.id === "ux17-invalid-config-schema")) {
    const m = invalidConfigSchemaModel(platform), o = m.o, ro = m.roots.o, started = Date.now(), actions = [], checks = [], sessions = [];
    const receipt = (kind, id, details) => record(kind, id, { proof: contract.id, ...details });
    const expected = id => { const c = contract.checks.find(c => c.id === id); assert(c, "known invalid configuration check " + id); return c; };
    const check = (id, observed) => { const c = expected(id); assert.deepEqual(observed, c.expected, contract.id + "/" + id);
      checks.push({ ...c, observed, status: "passed" }); receipt("assertion", id, { expected: c.expected, observed }); };
    const disk = file => { const physical = path.join(workspace, safeFile(file)); return fs.existsSync(physical) ? fs.readFileSync(physical, "utf8") : null; };
    const uriFile = uri => relativeFile(workspace, fileURLToPath(canonicalUri(uri)));
    const state = async () => {
      const active = (await bridge("inspect")).active; if (!active) return null;
      const relative = path.relative(workspace, fileURLToPath(canonicalUri(active.uri)));
      if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) { receipt("observation", "transitional-editor", { uri: active.uri }); return null; }
      const file = uriFile(active.uri);
      return { file, text: active.text, disk: disk(file), dirty: active.dirty, selections: active.selections };
    };
    const awaitState = async id => {
      const fields = Object.keys(expected(id).expected);
      check(id, await until(id, async () => {
        const s = await state(), value = s && Object.fromEntries(fields.map(field => [field, s[field]]));
        receipt("observation", id + "-state", { state: value }); return isDeepStrictEqual(value, expected(id).expected) && value;
      }));
    };
    const trace = () => fs.readFileSync(sessionLog(readSession(root), name => name.endsWith("-Vela LSP Trace.log")), "utf8");
    const output = () => fs.readFileSync(sessionLog(readSession(root), name => /-Vela\.log$/.test(name)), "utf8");
    const traceDirectory = () => {
      const paths = [...output().matchAll(/Language server trace log: (.+)$/gm)]; assert.equal(paths.length, 1);
      const physical = fs.realpathSync(path.resolve(paths[0][1].trim()));
      assert(!path.isAbsolute(path.relative(fs.realpathSync(workspace), physical)) && !path.relative(fs.realpathSync(workspace), physical).startsWith(".."));
      return path.dirname(physical);
    };
    const saveSession = () => { sessions.push(snapshot(root, contract.id, sessions.length, readSession(root), traceDirectory()));
      fs.writeFileSync(path.join(root, contract.id + "-sessions.json"), JSON.stringify(sessions, null, 2)); };
    const publications = source => source.split("\n\n\n").slice(0, -1).flatMap(block => {
      const match = block.match(/Received notification 'textDocument\/publishDiagnostics'\.\nParams: ([\s\S]+)$/); if (!match) return [];
      const p = JSON.parse(match[1]); assert.equal(Object.hasOwn(p, "version"), false);
      return [{ ...p, uri: uriFile(p.uri), diagnostics: p.diagnostics.map(d => ({ ...d, message: ownedMessage(d.message, workspace, platform),
        data: { ...d.data, labels: d.data.labels.map(l => ({ ...l, uri: uriFile(l.uri) })) } })) }];
    });
    const messages = source => source.split("\n\n\n").slice(0, -1).flatMap(block => {
      const match = block.match(/Received notification 'window\/logMessage'\.\nParams: ([\s\S]+)$/); return match ? [JSON.parse(match[1])] : [];
    });
    let mutationBoundary = trace().length, nativeBoundary, phase;
    const diagnostic = async id => {
      const wanted = expected(id + "-diagnostics").expected;
      // A rejected notification has no publications. Check the exact rejection
      // and retain previously established complete facts from this same client.
      if (id === "rejected") {
        const error = expected(id + "-rejection").expected;
        check(id + "-rejection", await until("exact configuration rejection", () => {
          const matches = messages(trace().slice(mutationBoundary)); receipt("observation", id + "-messages", { messages: matches });
          return matches.length === 1 && isDeepStrictEqual(matches[0], error) && matches[0];
        }));
      }
      check(id + "-diagnostics", await until("complete current owned diagnostics", () => {
        const rows = publications(id === "rejected" ? trace() : trace().slice(mutationBoundary));
        const relevant = rows.filter(p => p.uri.startsWith("ux17-roots/"));
        assert(relevant.every(p => wanted.some(w => w.uri === p.uri)), "no extra owned source or metadata diagnostic owner");
        const latest = wanted.map(w => relevant.filter(p => p.uri === w.uri).at(-1));
        receipt("observation", id + "-diagnostics", { publications: latest }); return isDeepStrictEqual(latest, wanted) && latest;
      }));
    };
    const folderDialog = () => page.locator(".quick-input-widget:visible"), input = () => folderDialog().locator("input").filter({ visible: true });
    const explorer = () => page.locator('[id="workbench.view.explorer"]');
    const row = name => explorer().locator('[role="treeitem"]:not(.monaco-tree-sticky-row)').filter({ has: page.locator(".label-name").filter({ hasText: new RegExp("^" + name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + "$") }) });
    const pointer = async a => {
      let target;
      if (a.selector.startsWith("Quick Open file ") || a.selector.startsWith("Command palette ")) {
        const label = a.selector.startsWith("Quick Open file ") ? path.posix.basename(a.selector.slice(16)) : a.selector.slice(16);
        target = folderDialog().locator(".label-name").filter({ hasText: new RegExp("^" + label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + "$") });
      } else if (a.selector === "Explorer folder left") target = row("left");
      else if (a.selector.startsWith("Explorer file ")) target = row(a.selector.slice(14));
      else if (a.selector === "Explorer New File") target = explorer().getByRole("button", { name: /^New File/ });
      else if (a.selector === "Explorer Delete") {
        let visibleAt;
        const menu = await until("native Explorer Delete ready", async () => {
          try { const visible = await native.inspect("Delete"); visibleAt ??= Date.now(); return Date.now() - visibleAt >= 150 && visible; }
          catch (error) { if (!String(error.stderr).includes("expected one visible native menu item")) throw error; return null; }
        });
        assert(menu.enabled); receipt("observation", a.id + "-target", menu); await native.operate("Delete", "click"); return;
      } else if (a.selector === "Confirm file deletion") target = page.getByRole("button", { name: /^(?:Move to Recycle Bin|Move to Trash|Delete)$/ });
      else throw Error("unknown invalid configuration pointer " + a.selector);
      await target.waitFor({ state: "visible" }); assert.equal(await target.count(), 1, "unique owned input target");
      receipt("observation", a.id + "-target", { aria: await target.ariaSnapshot() });
      if (a.selector === "Confirm file deletion") {
        const dialog = page.getByRole("dialog"); assert.equal(await dialog.count(), 1);
        check(a.id.slice(0, -8) + "-confirmation", { role: await dialog.getAttribute("role"), visible: await dialog.isVisible(),
          name: await dialog.evaluate(element => (element.getAttribute("aria-labelledby") ?? "").split(/\s+/).map(id => document.getElementById(id))
            .map(label => label?.getAttribute("aria-label") ?? label?.textContent ?? "").join(" ")), button: (await target.innerText()).trim() });
      }
      if (a.button === "right") { const b = await target.boundingBox(); assert(b); receipt("observation", a.id + "-pointer", await native.contextClick({ x: b.x + b.width / 2, y: b.y + b.height / 2 })); }
      else await target.click();
      if (a.selector.startsWith("Quick Open file ")) {
        const file = a.selector.slice(16); await until("authored owned file opened", async () => {
          const active = (await bridge("inspect")).active; return active && canonicalUri(active.uri) === fileUri(path.join(workspace, file));
        });
      }
    };
    const hover = page.locator(".monaco-hover:visible");
    const widget = async id => check(id, await until(id, async () => {
      const value = await hover.count() === 0 ? { visible: false } : await hover.count() === 1 ? { visible: await hover.isVisible(),
        label: (await hover.locator(".monaco-tokenized-source").allTextContents()).join("").trim(),
        paragraphs: (await hover.locator(".markdown-hover p").allTextContents()).map(s => s.replaceAll("\u00a0", " ").trim()), diagnostics: await hover.locator(".marker.hover-contents").allTextContents() } : null;
      receipt("observation", id + "-widget", { widget: value }); return isDeepStrictEqual(value, expected(id).expected) && value;
    }));
    const currentSession = readSession(root); saveSession(); await native.activate();
    for (const a of contract.actions) {
      const prefix = a.id.split("-")[0];
      if (o.phases.some(p => p.id === prefix) && phase !== prefix) { saveSession(); mutationBoundary = trace().length; phase = prefix; }
      if (a.id.endsWith("-clear")) {
        const id = a.id.slice(0, -6);
        check(id + "-selected", await until("complete physical editor selection", async () => {
          const s = await state(); if (!s) return null;
          const selection = s.selections[0], start = offsetAt(s.text, selection.anchor), end = offsetAt(s.text, selection.active);
          const value = { file: s.file, allSelected: s.selections.length === 1 && Math.min(start, end) === 0 && Math.max(start, end) === s.text.length };
          receipt("observation", id + "-selection", { ...value, selections: s.selections }); return isDeepStrictEqual(value, expected(id + "-selected").expected) && value;
        }));
      }
      if (a.id === "probe-replace") await awaitState("probe-before");
      if (a.id.endsWith("-command")) {
        if (a.field === "value") {
          assert.equal(readSession(root).pid, currentSession.pid, "settings do not restart current client");
          assert.equal(readSession(root).token, currentSession.token);
          check(phase + "-folders", await bridge("workspace-roots-inspect"));
          check(phase + "-client", { serverFolder: relativeFile(workspace, traceDirectory()), started: output().includes("Language server started.") });
          check(phase + "-workspace-json", { file: ro.workspace, document: JSON.parse(disk(ro.workspace)) }); await diagnostic(phase);
        }
        await awaitState(a.id.slice(0, -8) + "-source");
      }
      if (a.id.endsWith("-chord")) nativeBoundary = trace().length;
      if (a.device === "command") {
        const value = await bridge("invalid-config-schema-query", { field: a.field }); actions.push(a);
        receipt("command", a.id, Object.fromEntries(Object.entries(a).filter(([k]) => k !== "id"))); check(a.id, value);
      } else {
        if (a.device === "pointer") await pointer(a);
        else if (a.path !== undefined) {
          const owned = path.resolve(workspace, safeFile(a.path)); assert(owned.startsWith(path.resolve(workspace) + path.sep));
          receipt("observation", a.id + "-resolved", { base: workspace, path: a.path, text: owned.replaceAll("\\", "/") }); await page.keyboard.insertText(owned.replaceAll("\\", "/"));
        } else if (a.text !== undefined) await page.keyboard.insertText(a.text);
        else await page.keyboard.press(a.key);
        actions.push(a); receipt("input", a.id, Object.fromEntries(Object.entries(a).filter(([k]) => k !== "id")));
      }
      if (a.id.endsWith("-open") || a.id.endsWith("-goto")) { await input().waitFor({ state: "visible" }); assert(await input().evaluate(element => document.activeElement === element)); }
      if (a.id.endsWith("-place")) await folderDialog().waitFor({ state: "hidden" });
      if (a.id.endsWith("-new")) { await explorer().getByRole("textbox").waitFor({ state: "visible" }); assert(await explorer().getByRole("textbox").evaluate(element => document.activeElement === element)); }
      if (a.id.endsWith("-create")) {
        const file = phase === "empty" ? o.manifest : ro.left + "/schema-one.json";
        await until("created owned editor is ready", async () => {
          const active = (await bridge("inspect")).active; return active && canonicalUri(active.uri) === fileUri(path.join(workspace, file));
        });
        await awaitState(phase + "-created");
        await page.getByRole("textbox", { name: new RegExp("^" + path.posix.basename(file).replaceAll(".", "\\.")) }).focus();
      }
      if (a.id.endsWith("-confirm")) {
        const file = phase === "deleted" ? ro.left + "/schema-one.json" : o.manifest;
        check(phase + "-deleted", await until("physically removed owned file", () => disk(file) === null && { file, disk: null }));
      }
      if (a.id.endsWith("-clear")) await awaitState(a.id.slice(0, -6) + "-empty");
      if (a.id.endsWith("-replace")) await awaitState(a.id.slice(0, -8) + "-inserted");
      if (a.id.endsWith("-save")) await awaitState(a.id.slice(0, -5) + "-saved");
      if (a.id.endsWith("-native")) {
        const id = a.id.slice(0, -7), rows = await until("complete current native hover", () => {
          const rows = hoverResponses(trace().slice(nativeBoundary)).filter(r => canonicalUri(r.params.textDocument.uri) === fileUri(path.join(workspace, o.file))); return rows.length && rows;
        });
        assert.equal(rows.length, 1); const r = rows[0]; check(id + "-wire", { method: r.method, request: { file: uriFile(r.params.textDocument.uri), position: r.params.position }, result: r.result });
        receipt("observation", id + "-request-id", { id: r.id }); await widget(id + "-widget");
      }
      if (a.id === "rejected-rank-dismiss") assert.deepEqual(publications(trace().slice(mutationBoundary)).filter(p => p.uri.startsWith("ux17-roots/")), [], "rejected settings publish no changed facts");
    }
    const documents = (await bridge("inspect")).documents;
    check("final-overlays", expected("final-overlays").expected.map(row => {
      const doc = documents.find(d => canonicalUri(d.uri) === fileUri(path.join(workspace, row.file))); assert(doc, "retained dirty buffer");
      return { file: row.file, text: doc.text, disk: disk(row.file), dirty: doc.dirty };
    }));
    saveSession(); assert.deepEqual(checks.map(c => c.id).sort(), contract.checks.map(c => c.id).sort());
    await page.screenshot({ path: path.join(root, contract.id + ".png") }); fs.writeFileSync(path.join(root, contract.id + ".aria.txt"), await page.locator("body").ariaSnapshot());
    const finished = Date.now(); assert(finished - started <= contract.deadlineMs, "bounded invalid configuration proof");
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed", startedAt: new Date(started).toISOString(),
      finishedAt: new Date(finished).toISOString(), durationMs: finished - started, actions, checks }); console.log("PASS " + contract.id);
  }
}
module.exports = { runInvalidConfigSchema, ownedMessage };
