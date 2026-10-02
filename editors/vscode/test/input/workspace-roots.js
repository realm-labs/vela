"use strict";
const fs = require("node:fs"), path = require("node:path"), assert = require("node:assert/strict");
const { fileURLToPath } = require("node:url");
const { isDeepStrictEqual } = require("node:util");
const { offsetAt, safeFile } = require("../../../../scripts/lsp-matrix/fixtures");
const { navigationResponses } = require("../../../../scripts/lsp-matrix/navigation-trace");
const { workspaceRootsModel } = require("../../../../scripts/lsp-matrix/workspace-roots-contracts");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { relativeFile, canonicalUri, fileUri } = require("./paths");
const { readSession, sessionLog, snapshot } = require("./session");

async function runWorkspaceRoots({ page, bridge, record, root, workspace, contracts, until, onProof }) {
  for (const contract of contracts.filter(c => c.id === "ux17-roots-settings")) {
    const m = workspaceRootsModel(process.platform), started = Date.now(), actions = [], checks = [], sessions = [];
    const receipt = (kind, id, details) => record(kind, id, { proof: contract.id, ...details });
    const expected = id => { const c = contract.checks.find(c => c.id === id); assert(c, "known roots check " + id); return c; };
    const check = (id, observed) => {
      const c = expected(id); assert.deepEqual(observed, c.expected, contract.id + "/" + id);
      checks.push({ ...c, observed, status: "passed" }); receipt("assertion", id, { expected: c.expected, observed });
    };
    const disk = file => fs.readFileSync(path.join(workspace, safeFile(file)), "utf8");
    const uriFile = uri => relativeFile(workspace, fileURLToPath(canonicalUri(uri)));
    const state = async () => {
      const active = (await bridge("inspect")).active;
      if (!active) return null;
      const relative = path.relative(workspace, fileURLToPath(canonicalUri(active.uri)));
      if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) {
        receipt("observation", "transitional-editor", { uri: active.uri });
        return null;
      }
      const file = uriFile(active.uri);
      return { file, text: active.text, disk: disk(file), dirty: active.dirty, selections: active.selections };
    };
    const awaitState = async id => {
      const fields = Object.keys(expected(id).expected);
      check(id, await until(id, async () => {
        const s = await state(), value = s && Object.fromEntries(fields.map(field => [field, s[field]]));
        receipt("observation", id + "-state", { state: value });
        return isDeepStrictEqual(value, expected(id).expected) && value;
      }));
    };
    const trace = () => fs.readFileSync(sessionLog(readSession(root), name => name.endsWith("-Vela LSP Trace.log")), "utf8");
    const output = () => fs.readFileSync(sessionLog(readSession(root), name => /-Vela\.log$/.test(name)), "utf8");
    let mutationBoundary = 0;
    const publications = () => trace().slice(mutationBoundary).split("\n\n\n").slice(0, -1).flatMap(block => {
      const match = block.match(/Received notification 'textDocument\/publishDiagnostics'\.\nParams: ([\s\S]+)$/);
      if (!match) return [];
      const p = JSON.parse(match[1]); assert.equal(Object.hasOwn(p, "version"), false, "unversioned publication contract");
      return [{ ...p, uri: uriFile(p.uri), diagnostics: p.diagnostics.map(d => ({ ...d,
        data: { ...d.data, labels: d.data.labels.map(l => ({ ...l, uri: uriFile(l.uri) })) } })) }];
    });
    const location = async (uri, range) => {
      const file = uriFile(uri), document = (await bridge("inspect")).documents.find(d => uriFile(d.uri) === file);
      const source = document?.text ?? disk(file);
      return { file, range, text: source.slice(offsetAt(source, range.start), offsetAt(source, range.end)) };
    };
    const serverTraceDirectory = () => {
      const paths = [...output().matchAll(/Language server trace log: (.+)$/gm)];
      assert.equal(paths.length, 1, "one launched server trace belongs to this observer");
      const file = path.resolve(paths[0][1].trim());
      const relative = path.relative(fs.realpathSync(workspace), fs.realpathSync(file));
      assert(relative && !relative.startsWith("..") && !path.isAbsolute(relative), "server trace stays within this run's workspace");
      return path.dirname(file);
    };
    const saveSession = () => {
      sessions.push(snapshot(root, contract.id, sessions.length, readSession(root), serverTraceDirectory()));
      fs.writeFileSync(path.join(root, contract.id + "-sessions.json"), JSON.stringify(sessions, null, 2));
    };
    let beforeHost, nativeBoundary, phase, activeSession = readSession(root);
    const settleClient = async () => {
      await until("current installed language client", () => {
        const s = readSession(root), file = sessionLog(s, name => /-Vela\.log$/.test(name), true);
        return file && fs.readFileSync(file, "utf8").includes("Language server started.");
      });
    };
    const refreshSession = async () => {
      const current = readSession(root);
      if (current.pid !== activeSession.pid || current.token !== activeSession.token) {
        await settleClient();
        receipt("observation", "workspace-host-transition", { previous: activeSession.pid, current: current.pid });
        activeSession = current;
        mutationBoundary = 0;
        saveSession();
      }
    };
    const newHost = async () => {
      await until("new observer after native workspace transition", () => {
        try { const s = readSession(root); return s.pid !== beforeHost.pid && s.token !== beforeHost.token && s; }
        catch (error) { if (error.code === "ENOENT" || error instanceof SyntaxError) return null; throw error; }
      }, 20000);
      await settleClient();
      activeSession = readSession(root); mutationBoundary = 0;
      saveSession();
    };
    const folderDialog = () => page.locator(".quick-input-widget:visible");
    const input = () => folderDialog().locator("input").filter({ visible: true });
    const pointer = async a => {
      let target;
      if (a.selector.startsWith("Quick Open file ")) {
        const file = a.selector.slice("Quick Open file ".length);
        target = folderDialog().locator(".label-name").filter({ hasText: new RegExp("^" + path.posix.basename(file).replaceAll(".", "\\.") + "$") });
      } else if (a.selector.startsWith("Command palette ")) {
        const name = a.selector.slice("Command palette ".length).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
        target = folderDialog().locator(".label-name").filter({ hasText: new RegExp("^" + name + "$") });
      } else if (a.selector === "Workspace folder left") target = folderDialog().locator(".label-name").filter({ hasText: /^left$/ });
      else if (a.selector === "Folder dialog Add") target = folderDialog().getByRole("button", { name: "Add", exact: true });
      else if (a.selector === "Workspace dialog OK") target = folderDialog().getByRole("button", { name: "OK", exact: true });
      else throw Error("unknown roots pointer " + a.selector);
      await target.waitFor({ state: "visible" }); assert.equal(await target.count(), 1, "unique owned workbench target");
      receipt("observation", a.id + "-target", { aria: await target.ariaSnapshot() });
      await target.click({ button: a.button });
      if (a.selector.startsWith("Quick Open file ")) {
        const wanted = a.selector.slice("Quick Open file ".length);
        await until("authored file opened by native picker", async () => {
          const active = (await bridge("inspect")).active;
          return active && canonicalUri(active.uri) === fileUri(path.join(workspace, wanted));
        });
      }
    };
    saveSession();
    for (const a of contract.actions) {
      if (["both-replace", "selected-replace", "restored-replace", "remove-left", "readd-confirm"].includes(a.id)) {
        saveSession(); mutationBoundary = trace().length;
      }
      // Saving the first-folder replacement and removing that first folder
      // restart the installed host. Wait for the new owned observer before
      // sending subsequent UI actions or inspecting an about-to-close socket.
      if (["workspace-confirm", "save-workspace-confirm", "both-save", "remove-left"].includes(a.id)) { beforeHost = readSession(root); saveSession(); }
      if (a.id.endsWith("-path-select")) {
        const title = a.id.startsWith("save-workspace") ? "Save Workspace" : "Add Folder to Workspace";
        await folderDialog().locator(".quick-input-title").filter({ hasText: title }).waitFor({ state: "visible" });
        await input().waitFor({ state: "visible" }); assert.equal(await input().count(), 1);
        assert(await input().evaluate(element => document.activeElement === element), "native dialog already owns input focus");
      }
      if (a.id.endsWith("-accept")) {
        const name = contract.actions.find(item => item.id === a.id.slice(0, -7) + "-name")?.text;
        if (name?.startsWith("Workspaces:") || name?.startsWith("Preferences:")) {
          const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
          const target = folderDialog().locator(".label-name").filter({ hasText: new RegExp("^" + escaped + "$") });
          await target.waitFor({ state: "visible" }); assert.equal(await target.count(), 1, "exact authored palette command");
          receipt("observation", a.id + "-command-target", { aria: await target.ariaSnapshot() });
        }
      }
      if (a.id.endsWith("-clear")) {
        const id = a.id.slice(0, -6);
        check(id + "-selected", await until("full native source selection", async () => {
          const s = await state(); if (!s) return null;
          const selection = s.selections[0], start = offsetAt(s.text, selection.anchor), end = offsetAt(s.text, selection.active);
          const value = { file: s.file, allSelected: Math.min(start, end) === 0 && Math.max(start, end) === s.text.length && s.selections.length === 1 };
          receipt("observation", id + "-selection", { ...value, selections: s.selections, length: s.text.length });
          return isDeepStrictEqual(value, expected(id + "-selected").expected) && value;
        }));
      }
      if (a.id.endsWith("-command")) {
        await refreshSession();
        const id = a.id.slice(0, -8), currentPhase = id.split("-")[0];
        if (phase !== currentPhase) {
          phase = currentPhase;
          check(phase + "-folders", await until("current workspace folders/settings", async () => {
            const s = await bridge("workspace-roots-inspect"); receipt("observation", phase + "-folders", s);
            return isDeepStrictEqual(s, expected(phase + "-folders").expected) && s;
          }));
          check(phase + "-client", { serverFolder: relativeFile(workspace, serverTraceDirectory()), started: output().includes("Language server started.") });
          check(phase + "-workspace-json", { file: m.o.workspace, document: JSON.parse(disk(m.o.workspace)) });
          check(phase + "-diagnostics", await until("complete current diagnostic ownership", () => {
            const rows = publications(), wanted = expected(phase + "-diagnostics").expected;
            const latest = wanted.map(w => rows.filter(p => p.uri === w.uri).at(-1));
            receipt("observation", phase + "-diagnostics", { publications: latest });
            return isDeepStrictEqual(latest, wanted) && latest;
          }));
        }
        await awaitState(id + "-source");
      }
      if (a.id.endsWith("-native")) nativeBoundary = trace().length;
      if (a.device === "command") {
        const rows = await bridge("workspace-roots-query", { file: a.file, marker: a.marker });
        const value = await Promise.all(rows.map(r => location(r.uri, r.range)));
        actions.push(a); receipt("command", a.id, Object.fromEntries(Object.entries(a).filter(([key]) => key !== "id"))); check(a.id, value);
      } else {
        if (a.device === "pointer") await pointer(a);
        else if (a.path !== undefined) {
          const owned = path.resolve(workspace, safeFile(a.path));
          assert(owned.startsWith(path.resolve(workspace) + path.sep), "physical input path belongs to this run");
          const value = owned.replaceAll("\\", "/") + (a.directory ? "/" : "");
          receipt("observation", a.id + "-resolved", { base: workspace, path: a.path, text: value });
          await page.keyboard.insertText(value);
          await until("file dialog reflects authored owned path", async () => (await input().inputValue()).replaceAll("\\", "/") === value);
          await until("file dialog finished resolving path", async () => await folderDialog().locator(".quick-input-progress.infinite:visible").count() === 0);
        } else if (a.text !== undefined) await page.keyboard.insertText(a.text);
        else await page.keyboard.press(a.key);
        actions.push(a); receipt("input", a.id, Object.fromEntries(Object.entries(a).filter(([key]) => key !== "id")));
      }
      if (["workspace-confirm", "save-workspace-confirm", "both-save", "remove-left"].includes(a.id)) await newHost();
      if (a.id.endsWith("-open") || a.id.endsWith("-goto")) {
        await input().waitFor({ state: "visible" });
        assert(await input().evaluate(element => document.activeElement === element), "native picker owns input focus");
      }
      if (a.id.endsWith("-place")) await folderDialog().waitFor({ state: "hidden" });
      if (a.id.endsWith("-settings-accept")) {
        await until("owned workspace JSON opened by Preferences", async () => {
          const active = (await bridge("inspect")).active;
          return active && canonicalUri(active.uri) === fileUri(path.join(workspace, m.o.workspace));
        });
      }
      if (a.id === "save-workspace-confirm") {
        const file = m.o.workspace, physical = path.join(workspace, file);
        check("workspace-created", { file, exists: fs.existsSync(physical), contained: fs.realpathSync(physical).startsWith(fs.realpathSync(workspace) + path.sep) });
      }
      if (a.id.endsWith("-clear")) await awaitState(a.id.slice(0, -6) + "-empty");
      if (a.id.endsWith("-replace")) await awaitState(a.id.slice(0, -8) + "-inserted");
      if (a.id.endsWith("-save")) await awaitState(a.id.slice(0, -5) + "-saved");
      if (a.id.endsWith("-native")) {
        const id = a.id.slice(0, -7), expectedWire = expected(id + "-wire").expected;
        const rows = await until("one complete native definition", () => {
          const rows = navigationResponses(trace().slice(nativeBoundary));
          return rows.length && rows;
        });
        assert.equal(rows.length, 1); const r = rows[0];
        assert(!Array.isArray(r.result), "complete server Location-or-null contract");
        check(id + "-wire", { method: r.method, request: { file: uriFile(r.params.textDocument.uri), position: r.params.position },
          result: r.result === null ? null : await location(r.result.uri, r.result.range) });
        receipt("observation", id + "-request-id", { id: r.id });
        if (expectedWire.result === null) {
          const message = page.locator('[id="workbench.parts.editor"]').getByText(/^No definition found for/);
          await message.waitFor({ state: "visible" }); check(id + "-empty", { text: await message.innerText(), visible: await message.isVisible() });
        }
        await awaitState(id + "-destination");
      }
    }
    saveSession();
    assert.deepEqual(checks.map(c => c.id).sort(), contract.checks.map(c => c.id).sort());
    await page.screenshot({ path: path.join(root, contract.id + ".png") });
    fs.writeFileSync(path.join(root, contract.id + ".aria.txt"), await page.locator("body").ariaSnapshot());
    const finished = Date.now(); assert(finished - started <= contract.deadlineMs, "bounded roots/settings proof");
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed", startedAt: new Date(started).toISOString(),
      finishedAt: new Date(finished).toISOString(), durationMs: finished - started, actions, checks });
    console.log("PASS " + contract.id);
  }
}
module.exports = { runWorkspaceRoots };
