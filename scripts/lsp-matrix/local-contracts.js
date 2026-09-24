"use strict";

// Independent, reviewed actions and expected outcomes. Runtime code supplies
// observations; these definitions must never query the provider/workbench.
function localContracts(requirements, fixture, platform = "darwin") {
  if (!["darwin", "win32"].includes(platform)) throw Error(`unsupported interaction platform ${platform}`);
  const requirement = requirements.find(
    (item) => item.id === "batch/B01/input-render-driver",
  );
  if (!requirement) throw new Error("driver obligation is missing");
  const contracts = [
    {
      id: "input-driver",
      fixture: fixture.id,
      deadlineMs: 45000,
      requirements: [
        { id: requirement.id, contractHash: requirement.contractHash },
      ],
      actions: [
        {
          id: "type-prefix",
          device: "keyboard",
          text: fixture.oracle.typedText,
        },
        { id: "open-suggestions", device: "keyboard", key: "Control+Space" },
        {
          id: "accept-candidate",
          device: "pointer",
          selector: "suggest-widget option",
          label: fixture.oracle.candidate,
          clickCount: 1,
        },
      ],
      checks: [
        { id: "editor-focus", level: "Input", expected: { focused: true } },
        {
          id: "visible-candidate",
          level: "Render",
          expected: {
            label: fixture.oracle.candidate,
            role: "option",
            visible: true,
          },
        },
        {
          id: "final-document",
          level: "Input",
          expected: {
            text: fixture.oracle.finalText,
            dirty: true,
            selections: [
              {
                anchor: fixture.oracle.finalSelection,
                active: fixture.oracle.finalSelection,
              },
            ],
          },
        },
      ],
      artifacts: [
        "trace.json",
        "suggestions.png",
        "suggestions.aria.txt",
        "final.png",
        "workbench.log",
      ],
    },
    ...require("./navigation-contracts").navigationContracts(requirements),
    ...require("./peek-contracts").peekContracts(requirements),
    ...require("./completion-contracts").completionContracts(requirements),
    ...require("./rename-contracts").renameContracts(requirements),
  ];
  if (platform === "win32") {
    const keys = { "Meta+Shift+P": "Control+Shift+P", "Meta+Home": "Control+Home", "Control+-": "Alt+ArrowLeft", "Meta+z": "Control+z", "Meta+a": "Control+a", "Meta+Shift+z": "Control+y", Meta: "Control" };
    for (const contract of contracts) {
      for (const action of contract.actions) {
        if (keys[action.key]) action.key = keys[action.key];
        if (action.id === "accept-candidate") action.selector = "suggest-widget listitem";
      }
      for (const check of contract.checks) {
        if (check.id === "native-submenu") check.expected.role = "menuitem";
        if (check.id === "visible-candidate") check.expected.role = "listitem";
      }
      contract.artifacts = contract.artifacts.filter((file) => file !== "native-menu");
    }
  }
  return contracts;
}
module.exports = { localContracts };
