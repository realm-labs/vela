"use strict";
const { parseMarkers } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/input-hover-signature.json");

function hoverSignatureModel() {
  return { spec, file: spec.oracle.file, disk: parseMarkers(spec.files[spec.oracle.file]),
    typed: Object.fromEntries(Object.entries(spec.oracle.typed).map(([id, text]) => [id, parseMarkers(text)])) };
}
function hoverSignatureContracts(requirements) {
  if (!requirements.some(r => r.id.startsWith("vscode/UX10/"))) return [];
  const m = hoverSignatureModel();
  const point = marker => ({ line: marker.start.line, character: marker.start.character });
  const state = (source, marker, dirty = false) => ({ file: m.file, text: source.text, dirty,
    disk: m.disk.text, selections: [{ anchor: point(source.markers[marker]), active: point(source.markers[marker]) }] });
  const check = (id, level, expected) => ({ id, level, expected });
  const hover = id => check(id, "Render", { visible: true, ...spec.oracle.hover });
  const hidden = id => check(id, "Render", { visible: false });
  const key = (id, binding) => ({ id, device: "keyboard", key: binding });
  const show = prefix => [key(`${prefix}-chord`, "Meta+k"), key(`${prefix}-invoke`, "Meta+i")];
  const pointer = id => ({ id, device: "pointer", selector: "authored target glyph", marker: "target" });
  const base = marker => [check("origin", "Input", state(m.disk, marker)), check("editor-focus", "Input", { focused: true })];
  const ending = marker => [check("final-source", "Input", state(m.disk, marker)), check("final-focus", "Input", { focused: true })];
  const signature = (id, index) => check(id, "Render", { visible: true, label: spec.oracle.signature.label,
    active: spec.oracle.signature.parameters[index] });
  const contract = (route, actions, checks) => ({ id: `ux10-${route}`, fixture: spec.id, deadlineMs: 45000,
    requirements: ["input", "render"].map(level => {
      const id = `vscode/UX10/${route}/${level}/local`, r = requirements.find(r => r.id === id);
      if (!r) throw Error(`missing hover/signature obligation ${id}`);
      return { id, contractHash: r.contractHash };
    }), actions, checks: [...checks, check("disk-inputs", "Input", Object.fromEntries(Object.entries(spec.files).map(([f,s]) => [f,parseMarkers(s).text])))],
    artifacts: ["trace.json", `ux10-${route}-open.png`, `ux10-${route}-open.aria.txt`, `ux10-${route}-final.png`,
      "workbench.log", "lsp-trace.log", "extension-host.log", "server-trace.jsonl"] });
  return [
    contract("pointer-hover", [pointer("point-target"), key("dismiss", "Escape")],
      [...base("target"), check("target-range", "Render", { marker: "target", text: "combine", aligned: true }), hover("visible-hover"), hidden("dismissed"), ...ending("target")]),
    contract("keyboard-hover", [...show("show"), key("dismiss", "Escape")],
      [...base("target"), hover("visible-hover"), hidden("dismissed"), ...ending("target")]),
    contract("signature-arguments", [
      { id: "type-open", device: "keyboard", text: "combine(" },
      { id: "type-right", device: "keyboard", text: "right = 4" },
      { id: "type-left", device: "keyboard", text: ", left = 1" },
      { ...key("first-home", "Meta+ArrowLeft"), count: 2 }, { ...key("first-argument", "ArrowRight"), count: 12 }, key("show-first", "Meta+Shift+Space"),
      key("last-end", "Meta+ArrowRight"), key("last-argument", "ArrowLeft"), key("show-last", "Meta+Shift+Space"), key("dismiss", "Escape")],
      [...base("cursor"), ...[ ["open",0], ["right",1], ["both",0], ["first",1] ].flatMap(([id,index]) => [
        check(`${id}-source`, "Input", state(m.typed[id], "active", true)), signature(`${id}-signature`, index)]),
      check("last-source", "Input", state(m.typed.both,"active",true)), signature("last-signature",0), hidden("dismissed"),
      check("final-source", "Input", state(m.typed.both,"active",true)), check("final-focus","Input",{focused:true})]),
    contract("dismiss-hover", [pointer("point-target"), { id: "leave-target", device: "pointer", selector: "editor blank corner" },
      ...show("show"), key("dismiss", "Escape")],
      [...base("target"), hover("pointer-hover"), hidden("left-target"), hover("keyboard-hover"), hidden("dismissed"), ...ending("target")]),
    contract("unknown-receiver", [...show("show"), { ...key("enter-call", "ArrowRight"), count: 8 }, key("show-signature", "Meta+Shift+Space")],
      [...base("unknown"), check("hover-request", "Input", { method: "textDocument/hover", completed: true }), hidden("unknown-hover"),
      check("argument-source", "Input", state(m.disk,"unknown-argument")), check("signature-request","Input",{method:"textDocument/signatureHelp",completed:true}),
      hidden("unknown-signature"), ...ending("unknown-argument")]),
  ];
}
module.exports = { hoverSignatureModel, hoverSignatureContracts };
