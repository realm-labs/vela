"use strict";
const { parseMarkers, offsetAt } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/input-completion.json");

function completionModel(route) {
  const item = spec.oracle.routes.find((item) => item.id === route);
  if (!item) throw Error(`unknown completion route ${route}`);
  const document = parseMarkers(spec.files[item.file]);
  const point = ({ line, character }) => ({ line, character });
  const cursor = point(document.markers.cursor.start);
  const range = { start: point(document.markers.replace.start), end: point(document.markers.replace.end) };
  const state = (text, caret, dirty) => ({ file: item.file, text, dirty,
    selections: [{ anchor: caret, active: caret }] });
  const offset = offsetAt(document.text, cursor);
  const typedText = document.text.slice(0, offset) + spec.oracle.typedText + document.text.slice(offset);
  const typedCursor = { ...cursor, character: cursor.character + spec.oracle.typedText.length };
  const accepted = document.text.slice(0, offsetAt(document.text, range.start)) + spec.oracle.acceptedText +
    document.text.slice(offsetAt(document.text, range.end));
  return { spec, file: item.file, cursor, range,
    origin: state(document.text, cursor, false),
    typed: state(typedText, typedCursor, true),
    accepted: state(accepted, { ...range.start, character: range.start.character + spec.oracle.acceptedText.length - 1 }, true),
  };
}

function completionContracts(requirements) {
  if (!requirements.some((item) => item.id.startsWith("vscode/UX04/"))) return [];
  const key = (id, key) => ({ id, device: "keyboard", key });
  const check = (id, level, expected) => ({ id, level, expected });
  return spec.oracle.routes.map(({ id: route }) => {
    const id = `ux04-${route}`, model = completionModel(route);
    const dismiss = route === "dismiss-escape";
    const refs = ["input", "render"].map((level) => {
      const name = `vscode/UX04/${route}/${level}/local`, requirement = requirements.find((item) => item.id === name);
      if (!requirement) throw Error(`missing completion obligation: ${name}`);
      return { id: name, contractHash: requirement.contractHash };
    });
    return { id, fixture: spec.id, deadlineMs: 45000, requirements: refs,
      actions: [key("clear-prior-widget", "Escape"),
        { id: "type-prefix", device: "keyboard", text: spec.oracle.typedText },
        key("open-suggestions", "Control+Space"),
        ...(!dismiss ? [key("select-next", "ArrowDown"), key("open-details", "Control+Space"), key("close-details", "Control+Space")] : []),
        key("finish-completion", dismiss ? "Escape" : route === "accept-tab" ? "Tab" : "Enter"),
        ...(!dismiss ? [key("undo-completion", "Meta+z")] : [])],
      checks: [check("origin", "Input", model.origin), check("editor-focus", "Input", { focused: true }),
        check("typed-source", "Input", model.typed),
        check("visible-candidates", "Render", { labels: spec.oracle.labels, visible: true }),
        ...(!dismiss ? [check("selected-candidate", "Render", { label: spec.oracle.labels[1], visible: true }),
          check("resolved-documentation", "Render", { text: spec.oracle.documentation, visible: true })] : []),
        check("widget-hidden", "Render", { visible: false }),
        check("final-source", "Input", dismiss ? model.typed : model.accepted),
        check("restored-focus", "Input", { focused: true }),
        ...(!dismiss ? [check("undo-source", "Input", model.typed)] : [])],
      artifacts: ["trace.json", `${id}-open.png`, `${id}-open.aria.txt`, `${id}.png`, `${id}.aria.txt`,
        "workbench.log", "lsp-trace.log", "extension-host.log", "server-trace.jsonl"],
    };
  });
}
module.exports = { completionModel, completionContracts };
