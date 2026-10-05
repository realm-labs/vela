"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const { quickOpenFileLabels } = require("../../editors/vscode/test/input/quick-open");

test("installation Quick Open rejects same-basename stale and sibling rows on both directory separator profiles", () => {
  for (const slash of ["/", "\\"]) {
    const rows = [
      { name: "main.vela", directory: "scripts" },
      { name: "main.vela", directory: "scripts" + slash + "game" },
      { name: "main.vela", directory: "scripts" + slash + "installation" },
      { name: "main.vela", directory: "other" + slash + "installation" },
      { name: "main.vela.bak", directory: "scripts" + slash + "installation" },
    ];
    const labels = quickOpenFileLabels("scripts/installation/main.vela");
    assert.deepEqual(rows.filter(r => labels.name.test(r.name) && labels.directory.test(r.directory)), [rows[2]]);
  }
});
test("installation Quick Open escapes complete Unicode space percent and regex-significant path labels", () => {
  const labels = quickOpenFileLabels("中文 % (root)/scripts[1]/main+.vela");
  assert(labels.name.test("main+.vela"));
  assert(labels.directory.test("中文 % (root)/scripts[1]"));
  assert(labels.directory.test("中文 % (root)\\scripts[1]"));
  for (const name of ["mainX.vela", "main+.vela.bak", "main+.vela "]) assert(!labels.name.test(name));
  for (const directory of ["中文 % root/scripts1", "中文 % (root)/scripts[1]/nested", "other/中文 % (root)/scripts[1]"])
    assert(!labels.directory.test(directory));
});
test("installation Quick Open refuses ambiguous or escaping fixture paths", () => {
  for (const file of ["", "main.vela", "/scripts/main.vela", "../main.vela", "scripts/../main.vela", "scripts/./main.vela", "scripts//main.vela", "scripts\\main.vela", "C:/scripts/main.vela"])
    assert.throws(() => quickOpenFileLabels(file), /owned relative file path/);
});
