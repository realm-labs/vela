"use strict";
const { parseMarkers } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/input-semantic-tokens.json");

function tokenRenderModel() {
  const disk = parseMarkers(spec.files[spec.oracle.file]);
  const shifted = parseMarkers(spec.oracle.shifted),
    unknown = parseMarkers(spec.oracle.unknown);
  const style = (role) => ({
    color: role ? spec.oracle.colors[role] : spec.oracle.plainColor,
    fontStyle: "normal",
    fontWeight: "400",
    textDecorationLine: "none",
  });
  const render = (document, roles = spec.oracle.roles) =>
    Object.entries(roles).map(([marker, role]) => {
      const range = document.markers[marker];
      const text = Buffer.from(document.text)
        .subarray(range.start.byte, range.end.byte)
        .toString();
      return {
        marker,
        text,
        line: range.start.line,
        start: range.start.character,
        end: range.end.character,
        visible: true,
        aligned: true,
        style: style(role),
      };
    });
  return {
    spec,
    file: spec.oracle.file,
    disk,
    shifted,
    unknown,
    render,
    enabled: render(disk),
    disabled: render(disk, spec.oracle.disabledRoles),
    shiftedStyles: render(shifted),
    unknownStyles: render(unknown, { ...spec.oracle.roles, call: "variable" }),
  };
}

function tokenRenderContracts(requirements) {
  if (!requirements.some((row) => row.id.startsWith("vscode/UX09/"))) return [];
  const m = tokenRenderModel();
  const requirementRows = (route) =>
    ["input", "render"].map((level) => {
      const id = `vscode/UX09/${route}/${level}/local`,
        row = requirements.find((row) => row.id === id);
      if (!row) throw Error(`missing UX09 obligation ${id}`);
      return { id, contractHash: row.contractHash };
    });
  const check = (id, level, expected) => ({ id, level, expected });
  const state = (document, dirty) => ({
    file: m.file,
    text: document.text,
    dirty,
    disk: m.disk.text,
  });
  const artifacts = (id) => [
    "trace.json",
    `${id}-before.png`,
    `${id}-after.png`,
    `${id}-observations.json`,
    "workbench.log",
    "lsp-trace.log",
    "extension-host.log",
    "server-trace.jsonl",
  ];
  const make = (route, actions, checks) => ({
    id: `ux09-${route}`,
    fixture: spec.id,
    deadlineMs: 60000,
    requirements: requirementRows(route),
    actions,
    checks,
    artifacts: [
      ...artifacts(`ux09-${route}`),
      ...{
        "edit-scroll": ["ux09-edit-scroll-scrolled-out.png"],
        "toggle-off-on": [
          "ux09-toggle-off-on-disabled.png",
          "ux09-toggle-off-on-settings.aria.txt",
        ],
        "unresolved-token": ["ux09-unresolved-token-unknown.png"],
      }[route],
    ],
  });
  const initial = [
    check("origin", "Input", state(m.disk, false)),
    check("initial-styles", "Render", m.enabled),
  ];
  const call = m.disk.markers.call;
  const point = (pos) => ({ line: pos.line, character: pos.character });
  const wheel = (direction) =>
    Array.from({ length: 36 }, (_, index) => ({
      id: `scroll-${direction}-${index + 1}`,
      device: "pointer",
      selector: "token editor",
      deltaY: direction === "out" ? 3000 : -3000,
    }));
  const inputGroups = ["//", " edited", " 中😀", "\n/*", " 新😀", " */", "\n"];
  if (inputGroups.join("") !== spec.oracle.prefix)
    throw Error("review Unicode input grouping when prefix changes");
  const undoPrefix = Array.from(
    { length: inputGroups.length - 1 },
    (_, index) => inputGroups.slice(0, -index - 1).join(""),
  );
  return [
    make(
      "edit-scroll",
      [
        { id: "home", device: "keyboard", key: "Meta+Home" },
        { id: "type-unicode", device: "keyboard", text: spec.oracle.prefix },
        ...wheel("out"),
        ...wheel("back"),
        ...Array.from({ length: inputGroups.length }, (_, index) => ({
          id: `undo-edit-${index + 1}`,
          device: "keyboard",
          key: "Meta+z",
        })),
      ],
      [
        ...initial,
        check("edited-source", "Input", state(m.shifted, true)),
        check("shifted-styles", "Render", m.shiftedStyles),
        check("scrolled-out", "Render", {
          headVisible: false,
          tailVisible: true,
        }),
        check("scroll-source", "Input", state(m.shifted, true)),
        check("returned-styles", "Render", m.shiftedStyles),
        ...undoPrefix.map((prefix, index) =>
          check(
            `undo-step-${index + 1}`,
            "Input",
            state({ text: prefix + m.disk.text }, true),
          ),
        ),
        check("undo-source", "Input", state(m.disk, false)),
        check("undo-styles", "Render", m.enabled),
      ],
    ),
    make(
      "toggle-off-on",
      [
        { id: "open-settings", device: "keyboard", key: "Meta+," },
        {
          id: "search-setting",
          device: "keyboard",
          text: "@id:editor.semanticHighlighting.enabled",
        },
        {
          id: "open-disabled-values",
          device: "pointer",
          selector: "semantic highlighting select",
          clickCount: 1,
        },
        { id: "first-disabled-value", device: "keyboard", key: "Home" },
        { id: "next-disabled-value", device: "keyboard", key: "ArrowDown" },
        {
          id: "select-disabled",
          device: "keyboard",
          key: "Enter",
          label: "false",
        },
        { id: "return-disabled", device: "keyboard", key: "Control+Tab" },
        { id: "open-settings-again", device: "keyboard", key: "Meta+," },
        {
          id: "open-enabled-values",
          device: "pointer",
          selector: "semantic highlighting select",
          clickCount: 1,
        },
        { id: "first-enabled-value", device: "keyboard", key: "Home" },
        {
          id: "select-enabled",
          device: "keyboard",
          key: "Enter",
          label: "true",
        },
        { id: "return-enabled", device: "keyboard", key: "Control+Tab" },
      ],
      [
        ...initial,
        check("visible-setting", "Render", {
          key: spec.oracle.setting,
          label: "Editor › Semantic Highlighting: Enabled",
          visible: true,
        }),
        check("disabled-setting", "Input", {
          key: spec.oracle.setting,
          value: false,
        }),
        check("disabled-source", "Input", state(m.disk, false)),
        check("disabled-styles", "Render", m.disabled),
        check("enabled-setting", "Input", {
          key: spec.oracle.setting,
          value: true,
        }),
        check("enabled-source", "Input", state(m.disk, false)),
        check("enabled-styles", "Render", m.enabled),
      ],
    ),
    make(
      "unresolved-token",
      [
        {
          id: "select-call",
          device: "pointer",
          selector: "token glyph",
          target: "call",
          clickCount: 2,
        },
        {
          id: "type-unknown",
          device: "keyboard",
          text: spec.oracle.replacement,
        },
        { id: "undo-unknown", device: "keyboard", key: "Meta+z" },
      ],
      [
        ...initial,
        check("selected-call", "Input", {
          anchor: point(call.start),
          active: point(call.end),
        }),
        check("unknown-source", "Input", state(m.unknown, true)),
        check("unknown-styles", "Render", m.unknownStyles),
        check("decoy-unopened", "Input", { opened: false }),
        check("undo-source", "Input", state(m.disk, false)),
        check("undo-styles", "Render", m.enabled),
      ],
    ),
  ];
}
module.exports = { tokenRenderModel, tokenRenderContracts };
