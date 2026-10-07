"use strict";
const fs = require("node:fs"), path = require("node:path"), assert = require("node:assert/strict");
const { fileURLToPath } = require("node:url");
const { workspaceFileModel } = require("../../../../scripts/lsp-matrix/workspace-files-contracts");
const { navigationResponses } = require("../../../../scripts/lsp-matrix/navigation-trace");
const { offsetAt } = require("../../../../scripts/lsp-matrix/fixtures");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { isDeepStrictEqual } = require("node:util");
const { relativeFile, fileUri, canonicalUri } = require("./paths");
const { readTrace, workspaceReadiness } = require("./readiness");
const { readSession, sessionLog } = require("./session");
const { readFileIfPresent } = require("./disk-file");

async function runWorkspaceFiles({ page, bridge, record, root, workspace, contracts, until, onProof, pid, platform }) {
  const native = require("./native-menu").nativeMenu({ root, platform, page, pid });
  for (const contract of contracts.filter(c => c.id.startsWith("ux17-explorer-"))) {
    const route = contract.id.slice("ux17-explorer-".length), m = workspaceFileModel(route), o = m.o;
    const started = Date.now(), actions = [], checks = [];
    const receipt = (kind, id, details) => record(kind, id, { proof: contract.id, ...details });
    const expected = id => { const c = contract.checks.find(c => c.id === id); assert(c, "known check " + id); return c; };
    const check = (id, observed) => {
      const c = expected(id); assert.deepEqual(observed, c.expected, contract.id + "/" + id);
      checks.push({ ...c, observed, status: "passed" }); receipt("assertion", id, { expected: c.expected, observed });
    };
    const disk = file => readFileIfPresent(path.join(workspace, file));
    const state = async () => {
      const active = (await bridge("inspect")).active; if (!active) return null;
      const file = relativeFile(workspace, fileURLToPath(active.uri));
      return { file, text: active.text, dirty: active.dirty, disk: disk(file), languageId: active.languageId, selections: active.selections };
    };
    const awaitState = async id => check(id, await until(id, async () => {
      const s = await state(); receipt("observation", id + "-state", { state: s }); return JSON.stringify(s) === JSON.stringify(expected(id).expected) && s;
    }));
    const trace = () => fs.readFileSync(sessionLog(readSession(root), n => n.endsWith("-Vela LSP Trace.log")), "utf8");
    const relativeUri = uri => relativeFile(workspace, fileURLToPath(canonicalUri(uri)));
    const publications = () => trace().split("\n\n\n").slice(0, -1).flatMap(block => {
      const match = block.match(/Received notification 'textDocument\/publishDiagnostics'\.\nParams: ([\s\S]+)$/);
      if (!match) return [];
      const p = JSON.parse(match[1]);
      assert.equal(Object.hasOwn(p, "version"), false, "unversioned diagnostics contract");
      return [{ ...p, uri: relativeUri(p.uri), diagnostics: p.diagnostics.map(d => ({ ...d,
        data: { ...d.data, labels: d.data.labels.map(l => ({ ...l, uri: relativeUri(l.uri) })) } })) }];
    });
    const diagnostic = async id => check(id, await until(id, () => {
      const p = publications().filter(p => p.uri === o.caller).at(-1);
      receipt("observation", id + "-publication", { publication: p }); return p && isDeepStrictEqual(p, expected(id).expected) && p;
    }));
    const settled = async () => {
      const value = await until("workspace mutations completed", () => workspaceReadiness(readTrace(workspace), { since: started, now: Date.now() }));
      receipt("observation", "workspace-settled", value);
    };
    const location = (uri, range) => {
      const file = relativeUri(uri), text = disk(file); assert.notEqual(text, null, "current definition file exists");
      return { file, range, text: text.slice(offsetAt(text, range.start), offsetAt(text, range.end)) };
    };
    const query = async () => (await bridge("workspace-file-query", { route })).map(r => location(r.uri, r.range));
    const wire = async (id, boundary) => {
      const rows = await until(id, () => {
        const rows = navigationResponses(trace().slice(boundary)).filter(r => canonicalUri(r.params.textDocument.uri) === fileUri(path.join(workspace, o.caller)));
        return rows.length && rows;
      });
      assert.equal(rows.length, 1, "one native query after the recorded input boundary");
      const r = rows[0]; assert(!Array.isArray(r.result), "server's complete Location-or-null contract");
      check(id, { method: r.method, request: { file: relativeUri(r.params.textDocument.uri), position: r.params.position },
        result: r.result === null ? null : location(r.result.uri, r.result.range) });
      receipt("observation", id + "-request-id", { id: r.id });
    };
    const explorer = () => page.locator('[id="workbench.view.explorer"]');
    // Sticky rows are visual copies of ancestor folders with the same aria
    // name and DOM id. Match the unique real tree node, never an arbitrary copy.
    const fileRow = name => explorer().locator('[role="treeitem"]:not(.monaco-tree-sticky-row)').filter({ has: page.locator(".label-name").filter({ hasText: new RegExp("^" + name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + "$") }) });
    const pointer = async a => {
      let target;
      if (a.selector === "Explorer folder scripts") target = fileRow("scripts");
      else if (a.selector.startsWith("Explorer file ")) target = fileRow(a.selector.slice("Explorer file ".length));
      else if (a.selector === "Explorer New File") target = explorer().getByRole("button", { name: /^New File/ });
      else if (a.selector === "Explorer Delete") {
        let visibleAt;
        const menu = await until("native Explorer Delete", async () => {
          try {
            const visible = await native.inspect("Delete");
            visibleAt ??= Date.now();
            // The pinned Monaco menu installs mouse-up listeners 100 ms after
            // render to suppress the context-opening gesture. Visibility alone
            // is insufficient. Observe this guard before the single click.
            return Date.now() - visibleAt >= 150 && visible;
          }
          catch (error) { if (!String(error.stderr).includes("expected one visible native menu item")) throw error; return null; }
        });
        assert.equal(menu.enabled, true); receipt("observation", a.id + "-target", menu);
        receipt("observation", "delete-menu-ready", { visibleAt, observedAt: Date.now(), inputGuardMs: 100 });
        if (platform === "win32") {
          // Click the actual action label after its native menu becomes ready.
          await page.locator('.monaco-menu [role="menuitem"] .action-label').filter({ hasText: /^Delete$/ }).click();
        } else await native.operate("Delete", "click");
        return;
      }
      else if (a.selector === "Confirm file deletion") target = page.getByRole("button", { name: /^(?:Move to Recycle Bin|Move to Trash|Delete)$/ });
      else if (a.selector.startsWith("Quick Open file ")) target = page.locator(".quick-input-widget .label-name").filter({ hasText: new RegExp("^" + path.posix.basename(a.selector.slice("Quick Open file ".length)).replace(".", "\\.") + "$") });
      else throw Error("unknown Explorer input " + a.selector);
      await target.waitFor({ state: "visible" }); assert.equal(await target.count(), 1, "one owned native Explorer target");
      receipt("observation", a.id + "-target", { aria: await target.ariaSnapshot() });
      if (a.selector === "Confirm file deletion") {
        const dialog = page.getByRole("dialog"); assert.equal(await dialog.count(), 1, "one owned file confirmation");
        check("confirmation", { role: await dialog.getAttribute("role"), visible: await dialog.isVisible(),
          name: await dialog.evaluate(element => (element.getAttribute("aria-labelledby") ?? "").split(/\s+/)
            .map(id => document.getElementById(id)).map(label => label?.getAttribute("aria-label") ?? label?.textContent ?? "").join(" ")),
          button: (await target.innerText()).trim() });
        await page.screenshot({ path: path.join(root, contract.id + "-confirmation.png") });
        fs.writeFileSync(path.join(root, contract.id + "-confirmation.aria.txt"), await dialog.ariaSnapshot());
      }
      if (a.button === "right") {
        const b = await target.boundingBox(); assert(b, "visible owned Explorer row");
        receipt("observation", a.id + "-native-pointer", await native.contextClick({ x: b.x + b.width / 2, y: b.y + b.height / 2 }));
      } else await target.click({ button: a.button });
    };
    await bridge("setup", { file: o.caller, line: 0, character: 0, reset: true });
    await native.activate();
    await page.getByRole("textbox", { name: new RegExp("^" + path.posix.basename(o.caller).replace(".", "\\.")) }).focus();
    let nativeBoundary;
    for (const a of contract.actions) {
      if (a.id.endsWith("-command")) {
        const prefix = a.id.slice(0, -"-command".length);
        await settled(); await awaitState(prefix + "-source"); await diagnostic(prefix + "-diagnostics");
      }
      if (a.id.endsWith("-native")) nativeBoundary = trace().length;
      if (a.device === "command") {
        const result = await query();
        actions.push(a); receipt("command", a.id, Object.fromEntries(Object.entries(a).filter(([k]) => k !== "id"))); check(a.id, result);
      } else {
        if (a.device === "pointer") await pointer(a);
        else if (a.text !== undefined) await page.keyboard.type(a.text);
        else await page.keyboard.press(a.key);
        actions.push(a); receipt("input", a.id, Object.fromEntries(Object.entries(a).filter(([k]) => k !== "id")));
      }
      if (a.id.endsWith("-open") || a.id.endsWith("-goto")) await page.locator(".quick-input-widget").waitFor({ state: "visible" });
      if (a.id.endsWith("-name")) {
        const candidate = page.locator(".quick-input-widget .label-name").filter({ hasText: a.text.startsWith("View:") ? /^View: Show Explorer$/ : new RegExp("^" + path.posix.basename(a.text).replace(".", "\\.") + "$") });
        await candidate.waitFor({ state: "visible" }); assert.equal(await candidate.count(), 1, "one exact native command/file");
      }
      if (a.id.endsWith("-accept") || a.id.endsWith("-place")) {
        if (a.id !== "rename-accept") await page.locator(".quick-input-widget").waitFor({ state: "hidden" });
      }
      if (a.id.endsWith("-new") || a.id === "rename-start") await explorer().getByRole("textbox").waitFor({ state: "visible" });
      if (a.id === "rename-accept") {
        await until("completed native file rename", () => disk(o.dependency) === null && disk(o.renamed) === m.dependency.text);
        await explorer().getByRole("textbox").waitFor({ state: "hidden" });
      }
      if (a.id.endsWith("-create")) {
        await until("created file open", async () => (await bridge("inspect")).active?.uri === fileUri(path.join(workspace, o.dependency)));
        await page.getByRole("textbox", { name: new RegExp("^" + path.posix.basename(o.dependency).replace(".", "\\.")) }).focus();
      }
      if (a.id.endsWith("-native")) {
        const prefix = a.id.slice(0, -"-native".length); await wire(prefix + "-wire", nativeBoundary); await awaitState(prefix + "-destination");
        if (prefix === "missing" && route === "delete") check("deleted-disk", { file: o.dependency, text: disk(o.dependency) });
      }
    }
    if (route === "rename") check("disk-membership", { old: disk(o.dependency), current: disk(o.renamed) });
    assert.deepEqual(checks.map(c => c.id).sort(), contract.checks.map(c => c.id).sort());
    await page.screenshot({ path: path.join(root, contract.id + ".png") });
    fs.writeFileSync(path.join(root, contract.id + ".aria.txt"), await page.locator("body").ariaSnapshot());
    const finished = Date.now(); assert(finished - started <= contract.deadlineMs, "bounded workspace proof");
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed", startedAt: new Date(started).toISOString(),
      finishedAt: new Date(finished).toISOString(), durationMs: finished - started, actions, checks }); console.log("PASS " + contract.id);
  }
}
module.exports = { runWorkspaceFiles };
