"use strict";
const test = require("node:test"),
  assert = require("node:assert/strict"),
  path = require("node:path");
const {
  tokenRenderModel,
  tokenRenderContracts,
} = require("./token-render-contracts");
const { localContracts } = require("./local-contracts");
const requirements = require("./inventory").loadInventory(
  path.resolve(__dirname, "../.."),
).executionRequirements;
const driver = require("../../tests/lsp_matrix/fixtures/input-driver.json");

test("UX09 independently pins multiline Unicode shifts, real scroll distance and all six obligations", () => {
  const m = tokenRenderModel(),
    contracts = tokenRenderContracts(requirements);
  assert(m.disk.text.split("\n").length > 90);
  assert.equal(m.shifted.text, m.spec.oracle.prefix + m.disk.text);
  assert.deepEqual(
    contracts.flatMap((c) => c.requirements.map((r) => r.id)).sort(),
    requirements
      .filter((r) => r.id.startsWith("vscode/UX09/"))
      .map((r) => r.id)
      .sort(),
  );
  for (let i = 0; i < m.enabled.length; i++) {
    assert.equal(m.shiftedStyles[i].line, m.enabled[i].line + 2);
    assert.equal(m.shiftedStyles[i].start, m.enabled[i].start);
    assert.deepEqual(m.shiftedStyles[i].style, m.enabled[i].style);
  }
  assert.equal(
    contracts[0].actions.filter((a) => a.id.startsWith("scroll-out-")).length,
    36,
  );
  assert.equal(
    contracts[0].actions.find((a) => a.id === "scroll-out-1").deltaY,
    3000,
  );
  assert.deepEqual(
    contracts[0].checks.find((c) => c.id === "scrolled-out").expected,
    { headVisible: false, tailVisible: true },
  );
  assert.equal(
    contracts[0].actions.filter((a) => a.id.startsWith("undo-edit-")).length,
    7,
  );
  assert.deepEqual(
    contracts[0].checks
      .filter((c) => c.id.startsWith("undo-step-"))
      .map((c) => c.expected.text.slice(0, -m.disk.text.length)),
    [
      "// edited 中😀\n/* 新😀 */",
      "// edited 中😀\n/* 新😀",
      "// edited 中😀\n/*",
      "// edited 中😀",
      "// edited",
      "//",
    ],
  );
});
test("UX09 unknown calls drop resolved function color and retain unrelated exact styles", () => {
  const m = tokenRenderModel(),
    old = m.enabled.find((r) => r.marker === "call"),
    next = m.unknownStyles.find((r) => r.marker === "call");
  assert.equal(old.text, "make");
  assert.equal(next.text, "missing");
  assert.equal(old.style.color, "rgb(220, 220, 170)");
  assert.equal(next.style.color, "rgb(156, 220, 254)");
  for (const name of [
    "struct-name",
    "declaration",
    "unicode",
    "local",
    "typed-field",
    "dynamic-field",
  ])
    assert.deepEqual(
      m.enabled.find((r) => r.marker === name).style,
      m.unknownStyles.find((r) => r.marker === name).style,
    );
  assert(m.spec.files["scripts/token_decoy.vela"].includes("pub fn missing"));
  assert(!m.unknown.text.includes("use token_decoy"));
});
test("UX09 theme toggle has independently different semantic and TextMate styles and native platform keys", () => {
  const m = tokenRenderModel();
  const enabled = (name) => m.enabled.find((r) => r.marker === name),
    disabled = (name) => m.disabled.find((r) => r.marker === name);
  assert.equal(disabled("unknown-owner").style.color, "rgb(78, 201, 176)");
  assert.equal(disabled("unknown-member").style.color, "rgb(220, 220, 170)");
  assert.notDeepEqual(
    enabled("unknown-owner").style,
    disabled("unknown-owner").style,
  );
  assert.notDeepEqual(
    enabled("unknown-member").style,
    disabled("unknown-member").style,
  );
  for (const [platform, key] of [
    ["darwin", "Meta+,"],
    ["win32", "Control+,"],
  ]) {
    const c = localContracts(requirements, driver, platform).find(
      (c) => c.id === "ux09-toggle-off-on",
    );
    assert.equal(c.actions.find((a) => a.id === "open-settings").key, key);
    assert.deepEqual(
      c.actions
        .filter((a) =>
          [
            "first-disabled-value",
            "next-disabled-value",
            "select-disabled",
          ].includes(a.id),
        )
        .map((a) => a.key),
      ["Home", "ArrowDown", "Enter"],
    );
    assert.equal(
      c.checks.find((c) => c.id === "disabled-setting").expected.value,
      false,
    );
    assert.equal(
      c.checks.find((c) => c.id === "enabled-setting").expected.value,
      true,
    );
  }
});
