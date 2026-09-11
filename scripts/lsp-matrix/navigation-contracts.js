"use strict";
const { parseMarkers } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/input-navigation.json");
const position = ({ line, character }) => ({ line, character });
function navigationModel() {
  const oracle = spec.oracle;
  const caller = parseMarkers(oracle.dirtyPrefix + spec.files[oracle.caller]);
  const definition = parseMarkers(spec.files[oracle.definitionFile]);
  const type = parseMarkers(spec.files[oracle.typeFile]);
  const editor = (file, document, marker, dirty) => {
    const cursor = position(document.markers[marker].start);
    return { file, text: document.text, dirty,
      selections: [{ anchor: cursor, active: cursor }] };
  };
  return {
    spec, caller,
    origin: (marker) => editor(oracle.caller, caller, marker, true),
    definition: editor(oracle.definitionFile, definition, "definition", false),
    type: editor(oracle.typeFile, type, "type", false),
    wire: (kind, marker) => {
      const isType = kind === "type";
      const document = isType ? type : definition;
      const range = document.markers[isType ? "type" : "definition"];
      return { method: `textDocument/${isType ? "typeDefinition" : kind}`,
        request: { file: oracle.caller, position: position(caller.markers[marker].start) },
        result: marker === "unknown" ? null : {
          file: isType ? oracle.typeFile : oracle.definitionFile,
          range: { start: position(range.start), end: position(range.end) },
          text: isType ? "Item" : "make",
        },
      };
    },
  };
}
const key = (id, value) => ({ id, device: "keyboard", key: value });
const command = (id, value) => ({ id, device: "command", command: value });
const palette = (prefix, title) => [
  key(`${prefix}-open`, "Meta+Shift+P"),
  { id: `${prefix}-query`, device: "keyboard", text: title },
  key(`${prefix}-accept`, "Enter"),
];
const commands = {
  definition: "editor.action.revealDefinition",
  declaration: "editor.action.revealDeclaration",
  type: "editor.action.goToTypeDefinition",
  back: "workbench.action.navigateBack",
};
function navigationContracts(requirements) {
  // Unit tests for the original driver may supply just its own obligation.
  if (!requirements.some((item) => item.id.startsWith("vscode/UX02/"))) return [];
  const model = navigationModel();
  const contracts = [];
  for (const level of ["Input", "Command"]) {
    const input = level === "Input";
    const make = (route, marker, actions, checks) => {
      const id = `vscode/UX02/${route}/${level.toLowerCase()}/local`;
      const requirement = requirements.find((item) => item.id === id);
      if (!requirement) throw Error(`missing navigation obligation: ${id}`);
      const proof = `ux02-${route}-${level.toLowerCase()}`;
      contracts.push({ id: proof, fixture: spec.id, deadlineMs: 45000,
        requirements: [{ id, contractHash: requirement.contractHash }],
        actions, checks: [
          { id: "origin", level, expected: model.origin(marker) },
          { id: "editor-focus", level, expected: { focused: true } },
          ...checks.map(([id, expected]) => ({ id, level, expected })),
        ],
        artifacts: ["trace.json", `${proof}.png`, "workbench.log", "lsp-trace.log", "extension-host.log", "server-trace.jsonl"],
      });
    };
    make("definition-back", "call", input ? [
      key("dirty-home", "Meta+Home"),
      { id: "dirty-prefix", device: "keyboard", text: spec.oracle.dirtyPrefix },
      key("definition-key", "F12"), key("back-key", "Control+-"),
    ] : [command("definition-command", commands.definition), command("back-command", commands.back)],
    [...(input ? [["unopened-target", { unopened: true }]] : []),
      ["wire-definition", model.wire("definition", "call")], ["destination", model.definition], ["restored-origin", model.origin("call")]]);
    make("declaration-palette", "call", input ? palette("declaration", "Go to Declaration") :
      [command("declaration-command", commands.declaration)], [["wire-declaration", model.wire("declaration", "call")], ["destination", model.definition]]);
    make("type-definition-palette", "typed-use", input ? palette("type", "Go to Type Definition") :
      [command("type-command", commands.type)], [["wire-type", model.wire("type", "typed-use")], ["destination", model.type]]);
    make("unknown-target", "unknown", input ? [
      key("unknown-definition", "F12"), ...palette("unknown-declaration", "Go to Declaration"),
      ...palette("unknown-type", "Go to Type Definition"),
    ] : [command("unknown-definition", commands.definition),
      command("unknown-declaration", commands.declaration), command("unknown-type", commands.type)],
    ["definition", "declaration", "type"].flatMap((kind) => [
      ...(input ? [[`empty-${kind}`, { text: `No ${kind === "type" ? "type definition" : kind} found for 'missing'`, visible: true }]] : []),
      [`wire-${kind}`, model.wire(kind, "unknown")], [`unchanged-${kind}`, model.origin("unknown")],
    ]));
  }
  return contracts;
}
module.exports = { navigationContracts, navigationModel, commands };
