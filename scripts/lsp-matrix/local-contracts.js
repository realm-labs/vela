"use strict";

// Independent, reviewed actions and expected outcomes. Runtime code supplies
// observations; these definitions must never query the provider/workbench.
function localContracts(requirements, fixture) {
  const requirement = requirements.find(
    (item) => item.id === "batch/B01/input-render-driver",
  );
  if (!requirement) throw new Error("driver obligation is missing");
  return [
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
  ];
}
module.exports = { localContracts };
