"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { safeFile } = require("../../../scripts/lsp-matrix/fixtures");
const oracle = require("../../../scripts/lsp-matrix/document-symbol-oracle");
const { SymbolSession } = require("./document-symbol-session");

const specs = Object.fromEntries(["declarations", "ownership", "recovery", "lifecycle"].map(name =>
  [name, require(`../../../tests/lsp_matrix/fixtures/document-symbol-${name}.json`)]));
const rootFor = (resultRoot, name, crlf) => path.join(resultRoot, "中文 % symbol roots", `${name}-${crlf ? "crlf" : "lf"}`);
const source = (text, crlf) => oracle.document(text, crlf);

function materializeSymbols(resultRoot) {
  for (const [name, spec] of Object.entries(specs)) for (const crlf of [false, true]) {
    const root = rootFor(resultRoot, name, crlf); assert(!fs.existsSync(root), "new private symbol fixture root");
    for (const [file, text] of Object.entries(spec.files)) {
      const target = path.join(root, safeFile(file)); fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.writeFileSync(target, source(text, crlf).text);
    }
    if (!fs.existsSync(path.join(root, "vela.toml"))) {
      fs.writeFileSync(path.join(root, "vela.toml"), `[package]\nid = 'dev.vela.symbol.${name}'\nname = 'symbol_${name}'\nversion = '0.1.0'\n[source]\nroots = ['scripts']\n`);
    }
    if (name === "ownership") {
      fs.writeFileSync(path.join(root, "schema.json"), JSON.stringify(oracle.metadataArtifact(spec.oracle.schema)));
      fs.writeFileSync(path.join(root, "scripts/schema_probe.vela"), "fn schema_probe(host: host::Box) { host.value; }\n");
    }
  }
}

async function withSession(vscode, workspace, name, crlf, action) {
  const session = new SymbolSession(vscode, workspace, rootFor(process.env.VELA_TEST_RESULT_DIR, name, crlf), crlf);
  let failure;
  try {
    await session.addRoot();
    await action(session, specs[name]);
  }
  catch (error) { failure = error; throw error; }
  finally {
    try { await session.finish(); }
    catch (cleanup) {
      if (failure) throw new AggregateError([failure, cleanup], `${failure.stack}\nCleanup also failed: ${cleanup.stack}`);
      throw cleanup;
    }
  }
}

async function runDeclarationSymbols(vscode, workspace) {
  for (const name of ["declarations", "ownership"]) for (const crlf of [false, true]) {
    await withSession(vscode, workspace, name, crlf, async (session, spec) => {
      if (name === "ownership") {
        const probe = await session.open("scripts/schema_probe.vela");
        const hover = await session.bounded("known schema field setup", () => vscode.commands.executeCommand(
          "vscode.executeHoverProvider", probe.uri, probe.positionAt(probe.getText().indexOf("value") + 1)));
        assert.deepEqual(hover?.flatMap(item => item.contents.map(content => content.value)),
          ["```vela\nhost::Box.value\n```\n\n_field_: String"], "loaded known String field, not unknown/missing metadata");
        await session.close("scripts/schema_probe.vela");
        const main = await session.open("scripts/main.vela");
        const target = await session.bounded("resolved source alias setup", () => vscode.commands.executeCommand(
          "vscode.executeTypeDefinitionProvider", main.uri, main.positionAt(main.getText().indexOf("row: Row") + 5)));
        assert.equal(target?.length, 1, "one actual imported source type");
        assert(session.sameUri((target[0].targetUri ?? target[0].uri).toString(), session.uri("scripts/source.vela")));
        const selected = target[0].targetSelectionRange ?? target[0].range;
        const marker = source(spec.files["scripts/source.vela"], crlf).markers["widget-name"];
        assert.deepEqual([selected.start.line, selected.start.character, selected.end.line, selected.end.character],
          [marker.start.line, marker.start.character, marker.end.line, marker.end.character], "source alias keeps actual Widget ownership under schema collision");
        await session.close("scripts/main.vela");
      }
      const trees = name === "declarations" ? { [spec.oracle.file]: spec.oracle.symbols } : spec.oracle.trees;
      for (const [file, rows] of Object.entries(trees)) {
        const initial = source(spec.files[file], crlf);
        await session.verify(file, initial, rows, `${name}/disk/${file}`);
        const shifted = source("// shifted 中😀\n/* second 😀 */\n" + spec.files[file], crlf);
        const edited = await session.replace(file, shifted); assert(edited.isDirty);
        await session.verify(file, shifted, rows, `${name}/dirty/${file}`);
        assert.equal(fs.readFileSync(session.uri(file).fsPath, "utf8"), initial.text);
        await session.close(file);
        await session.verify(file, initial, rows, `${name}/restored/${file}`);
        await session.close(file);
      }
    });
  }
}

async function runRecoverySymbols(vscode, workspace) {
  for (const crlf of [false, true]) await withSession(vscode, workspace, "recovery", crlf, async (session, spec) => {
    const file = spec.oracle.file, initial = source(spec.files[file], crlf);
    await session.verify(file, initial, spec.oracle.symbols, "recovery/disk");
    assert.equal(spec.oracle.cases.length, 56, "all authored recovery partitions");
    for (const item of spec.oracle.cases) {
      const damaged = source("// shifted 中😀\n" + item.source, crlf);
      assert((await session.replace(file, damaged)).isDirty);
      await session.verify(file, damaged, item.symbols, `recovery/${item.id}/damage`);
      await session.replace(file, initial);
      await session.verify(file, initial, spec.oracle.symbols, `recovery/${item.id}/repair`);
      assert.equal(fs.readFileSync(session.uri(file).fsPath, "utf8"), initial.text, "all recovery remains unsaved");
    }
    await session.close(file);
    await session.verify(file, initial, spec.oracle.symbols, "recovery/closed-restored");
  });
}

async function runLifecycleSymbols(vscode, workspace) {
  for (const crlf of [false, true]) await withSession(vscode, workspace, "lifecycle", crlf, async (session, spec) => {
    const phases = spec.oracle.phases.filter(phase => !phase.schemaAction);
    assert.equal(phases.length, 13, "all source lifecycle phases; schema phases have separate backend proof");
    const variant = id => source(spec.oracle.variants[id].source, crlf);
    for (const phase of phases) {
      for (const action of phase.actions) {
        switch (action.op) {
          case "open": await session.open(action.file); break;
          case "change": assert((await session.replace(action.file, variant(action.variant))).isDirty); break;
          case "close": await session.close(action.file); break;
          case "write": await session.write(action.file, variant(action.variant).text); break;
          case "delete": await session.write(action.file, null); break;
          default: throw Error(`unsupported finite symbol action ${action.op}`);
        }
      }
      for (const [file, id] of Object.entries(phase.views)) {
        if (id === null) {
          assert(!fs.existsSync(session.uri(file).fsPath), "deleted resource absent from physical disk");
          await assert.rejects(session.bounded(`${phase.id}/deleted resource`, () => vscode.commands.executeCommand(
            "vscode.executeDocumentSymbolProvider", session.uri(file))), error => {
            assert.match(error.message, /(?:FileNotFound|not found|does not exist|Unable to resolve nonexistent file)/i);
            const normalize = text => process.platform === "win32" ? text.toLowerCase() : text;
            assert(normalize(error.message).includes(normalize(session.uri(file).fsPath)), "resource error names the actual deleted owned file");
            return true;
          });
        } else {
          await session.verify(file, variant(id), spec.oracle.variants[id].symbols, `lifecycle/${phase.id}/${file}`);
          if (!Object.hasOwn(phase.open, file)) await session.close(file);
        }
        const disk = phase.disk[file];
        if (disk !== null) assert.equal(fs.readFileSync(session.uri(file).fsPath, "utf8"), variant(disk).text, `independent disk state ${phase.id}/${file}`);
      }
    }
  });
}

module.exports = { materializeSymbols, runDeclarationSymbols, runRecoverySymbols, runLifecycleSymbols };
