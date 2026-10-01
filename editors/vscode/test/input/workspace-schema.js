"use strict";
const fs = require("node:fs"), path = require("node:path"), assert = require("node:assert/strict");
const { isDeepStrictEqual } = require("node:util");
const { fileURLToPath } = require("node:url");
const { workspaceSchemaModel } = require("../../../../scripts/lsp-matrix/workspace-schema-contracts");
const { hoverResponses } = require("../../../../scripts/lsp-matrix/hover-trace");
const evidence = require("../../../../scripts/lsp-matrix/local-evidence");
const { relativeFile, fileUri, canonicalUri } = require("./paths");
const { readTrace, workspaceReadiness } = require("./readiness");
const { readSession, sessionLog } = require("./session");

async function runWorkspaceSchema({ page, bridge, record, root, workspace, contracts, until, onProof }) {
  for (const contract of contracts.filter(c => c.id === "ux17-schema-replace")) {
    const m = workspaceSchemaModel(), o = m.o, started = Date.now(), actions = [], checks = [];
    const receipt = (kind, id, details) => record(kind, id, { proof: contract.id, ...details });
    const expected = id => { const c = contract.checks.find(c => c.id === id); assert(c, "known schema check " + id); return c; };
    const check = (id, observed) => {
      const c = expected(id); assert.deepEqual(observed, c.expected, contract.id + "/" + id);
      checks.push({ ...c, observed, status: "passed" }); receipt("assertion", id, { expected: c.expected, observed });
    };
    const disk = file => fs.readFileSync(path.join(workspace, file), "utf8");
    const state = async () => {
      const active = (await bridge("inspect")).active; if (!active) return null;
      const file = relativeFile(workspace, fileURLToPath(active.uri));
      return { file, text: active.text, dirty: active.dirty, disk: disk(file), languageId: active.languageId, selections: active.selections };
    };
    const awaitState = async id => check(id, await until(id, async () => {
      const value = await state(); receipt("observation", id + "-state", { state: value });
      return isDeepStrictEqual(value, expected(id).expected) && value;
    }));
    const trace = () => fs.readFileSync(sessionLog(readSession(root), n => n.endsWith("-Vela LSP Trace.log")), "utf8");
    const relativeUri = uri => relativeFile(workspace, fileURLToPath(canonicalUri(uri)));
    const publications = text => text.split("\n\n\n").slice(0, -1).flatMap(block => {
      const match = block.match(/Received notification 'textDocument\/publishDiagnostics'\.\nParams: ([\s\S]+)$/);
      if (!match) return [];
      const p = JSON.parse(match[1]); assert.equal(Object.hasOwn(p, "version"), false, "unversioned publication contract");
      return [{ ...p, uri: relativeUri(p.uri), diagnostics: p.diagnostics.map(d => ({ ...d,
        data: { ...d.data, labels: d.data.labels.map(l => ({ ...l, uri: relativeUri(l.uri) })) } })) }];
    });
    let mutationBoundary = 0, nativeBoundary;
    const diagnostic = async id => check(id, await until(id, () => {
      const p = publications(trace().slice(mutationBoundary)).filter(p => p.uri === expected(id).expected.uri).at(-1);
      receipt("observation", id + "-publication", { publication: p });
      return p && isDeepStrictEqual(p, expected(id).expected) && p;
    }));
    const hover = page.locator(".monaco-hover:visible");
    const widget = async id => check(id, await until(id, async () => {
      const value = await hover.count() === 0 ? { visible: false } : await hover.count() === 1 ? {
        visible: await hover.isVisible(), label: (await hover.locator(".monaco-tokenized-source").allTextContents()).join("").trim(),
        paragraphs: (await hover.locator(".markdown-hover p").allTextContents()).map(s => s.replaceAll("\u00a0", " ").trim()),
        diagnostics: await hover.locator(".marker.hover-contents").allTextContents() } : null;
      receipt("observation", id + "-widget", { widget: value });
      return isDeepStrictEqual(value, expected(id).expected) && value;
    }));
    await bridge("setup", { file: o.file, line: 0, character: 0, reset: true });
    await page.getByRole("textbox", { name: /^schema_observer\.vela/ }).focus();
    for (const a of contract.actions) {
      if (a.id.endsWith("-clear")) await awaitState(a.id.split("-")[0] + "-selection");
      if (a.id.endsWith("-replace")) mutationBoundary = trace().length;
      if (a.id.endsWith("-command")) {
        const id = a.id.slice(0, -8), prefix = id.split("-")[0];
        receipt("observation", id + "-ready", await until("schema workspace settled", () => workspaceReadiness(readTrace(workspace), { since: started, now: Date.now() })));
        await awaitState(id + "-source");
        if (a.field === "value") {
          check(prefix + "-disk", { file: o.schema, text: disk(o.schema) });
          await diagnostic(prefix + "-diagnostics"); await diagnostic(prefix + "-metadata-diagnostics");
        }
      }
      if (a.id.endsWith("-chord")) nativeBoundary = trace().length;
      if (a.device === "command") {
        const value = await bridge("workspace-schema-query", { field: a.field });
        actions.push(a); receipt("command", a.id, Object.fromEntries(Object.entries(a).filter(([k]) => k !== "id"))); check(a.id, value);
      } else {
        if (a.device === "pointer") {
          assert(a.selector.startsWith("Quick Open file "), "known schema picker input");
          const file = a.selector.slice("Quick Open file ".length);
          const target = page.locator(".quick-input-widget .label-name").filter({ hasText: new RegExp("^" + path.posix.basename(file).replace(".", "\\.") + "$") });
          await target.waitFor({ state: "visible" }); assert.equal(await target.count(), 1, "unique owned schema file result");
          receipt("observation", a.id + "-target", { aria: await target.ariaSnapshot() }); await target.click({ button: a.button });
          await until("schema file open", async () => (await bridge("inspect")).active?.uri === fileUri(path.join(workspace, file)));
        } else if (a.text !== undefined) await page.keyboard.insertText(a.text);
        else await page.keyboard.press(a.key);
        actions.push(a); receipt("input", a.id, Object.fromEntries(Object.entries(a).filter(([k]) => k !== "id")));
      }
      if (a.id.endsWith("-clear")) await awaitState(a.id.split("-")[0] + "-empty");
      if (a.id.endsWith("-replace")) await awaitState(a.id.split("-")[0] + "-inserted");
      if (a.id.endsWith("-save")) {
        const prefix = a.id.split("-")[0], target = expected(prefix + "-disk").expected.text;
        await until("schema artifact physically saved", async () => {
          const s = await state(); receipt("observation", a.id + "-state", { state: s });
          return s?.file === o.schema && s.text === target && s.disk === target && !s.dirty && s.languageId === "json";
        });
      }
      if (a.id.endsWith("-native")) {
        const id = a.id.slice(0, -7);
        const rows = await until(id + "-native-response", () => {
          const rows = hoverResponses(trace().slice(nativeBoundary)).filter(r => canonicalUri(r.params.textDocument.uri) === fileUri(path.join(workspace, o.file)));
          return rows.length && rows;
        });
        assert.equal(rows.length, 1, "one complete native hover after the recorded keyboard boundary");
        const r = rows[0]; check(id + "-wire", { method: r.method, request: { file: relativeUri(r.params.textDocument.uri), position: r.params.position }, result: r.result });
        receipt("observation", id + "-request-id", { id: r.id }); await widget(id + "-widget");
        if (id.endsWith("-rank")) await page.screenshot({ path: path.join(root, contract.id + "-" + id.split("-")[0] + ".png") });
      }
    }
    await awaitState("final-source"); check("final-disk", { file: o.schema, text: disk(o.schema) });
    assert.deepEqual(checks.map(c => c.id).sort(), contract.checks.map(c => c.id).sort());
    await page.screenshot({ path: path.join(root, contract.id + ".png") });
    fs.writeFileSync(path.join(root, contract.id + ".aria.txt"), await page.locator("body").ariaSnapshot());
    const finished = Date.now(); assert(finished - started <= contract.deadlineMs, "bounded schema replacement proof");
    onProof({ id: contract.id, fixture: contract.fixture, contractHash: evidence.jsonHash(contract), status: "passed", startedAt: new Date(started).toISOString(),
      finishedAt: new Date(finished).toISOString(), durationMs: finished - started, actions, checks }); console.log("PASS " + contract.id);
  }
}
module.exports = { runWorkspaceSchema };
