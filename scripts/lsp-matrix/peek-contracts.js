"use strict";
const { navigationModel } = require("./navigation-contracts");
function peekContracts(requirements) {
  if (!requirements.some((item) => item.id.startsWith("vscode/UX03/"))) return [];
  const model = navigationModel();
  const key = (id, key, event) => ({ id, device: "keyboard", key, ...(event ? { event } : {}) });
  const click = (id, target, button = "left", clickCount = 1) => ({ id, device: "pointer", target, button, clickCount });
  const modifier = [{ id: "modifier-hover", device: "pointer", target: "source-identifier", event: "hover" }, key("modifier-down", "Meta", "down"), click("modifier-click", "source-identifier"), key("modifier-up", "Meta", "up")];
  const menu = [click("context-menu", "source-identifier", "right"),
    { id: "peek-submenu", device: "pointer", target: "Peek", event: "hover" },
    click("peek-definition", "Peek Definition")];
  const check = (id, level, expected) => ({ id, level, expected });
  const preview = [check("peek-visible", "Render", { visible: true }),
    check("preview-line", "Render", { text: model.definition.text.split("\n")[1], visible: true }),
    check("preview-highlight", "Render", { text: "make", visible: true, aligned: true }),
    check("peek-source", "Input", model.origin("call"))];
  return ["modifier-click", "peek-follow", "peek-dismiss", "unknown-target"].map((route) => {
    const unknown = route === "unknown-target", id = `ux03-${route}`;
    const refs = ["input", "render"].map((level) => {
      const name = `vscode/UX03/${route}/${level}/local`, item = requirements.find((item) => item.id === name);
      if (!item) throw Error(`missing Peek obligation: ${name}`);
      return { id: name, contractHash: item.contractHash };
    });
    const result = { id, fixture: model.spec.id, deadlineMs: 45000, requirements: refs,
      actions: route === "modifier-click" ? [...modifier] : unknown ? [...modifier, ...menu] :
        [...menu, route === "peek-follow" ? click("follow-target", "peek-target-title") : key("dismiss-peek", "Escape")],
      checks: [check("origin", "Input", model.origin(unknown ? "unknown" : "call")),
        check("editor-focus", "Input", { focused: true }),
        check("source-line", "Render", { text: unknown ? model.caller.text.split("\n")[model.origin("unknown").selections[0].active.line] : model.spec.oracle.renderedCallLine, visible: true })],
      artifacts: ["trace.json", `${id}.png`, `${id}.aria.txt`, "workbench.log", "lsp-trace.log", "extension-host.log", "server-trace.jsonl"],
    };
    if (route !== "modifier-click") {
      result.artifacts.push("native-menu", `${id}-menu.png`);
      result.checks.push(check("native-submenu", "Render", { title: "Peek Definition", role: "AXMenuItem", enabled: true }));
    }
    if (route === "modifier-click") result.checks.push(check("source-link", "Render", { text: "make", visible: true }));
    if (unknown) result.checks.push(
      check("modifier-wire", "Input", model.wire("definition", "unknown")),
      check("modifier-origin", "Input", model.origin("unknown")),
      check("peek-wire", "Input", model.wire("definition", "unknown")),
      check("empty-message", "Render", { text: "No definition found for 'missing'", visible: true }),
      check("peek-hidden", "Render", { visible: false }), check("final-origin", "Input", model.origin("unknown")));
    else {
      result.checks.push(check("wire-definition", "Input", model.wire("definition", "call")));
      if (route !== "modifier-click") {
        result.checks.push(...preview);
        result.artifacts.push(`${id}-open.png`, `${id}-open.aria.txt`);
      }
      if (route === "peek-dismiss") result.checks.push(check("peek-hidden", "Render", { visible: false }),
        check("restored-focus", "Input", { focused: true }), check("final-origin", "Input", model.origin("call")));
      else result.checks.push(check("destination", "Input", route === "peek-follow" ? {
        ...model.definition, selections: [{ anchor: model.wire("definition", "call").result.range.start,
          active: model.wire("definition", "call").result.range.end }],
      } : model.definition),
        check("destination-line", "Render", { text: model.definition.text.split("\n")[1], visible: true }));
    }
    result.actions.unshift(key("clear-prior-message", "Escape"));
    return result;
  });
}
module.exports = { peekContracts };
