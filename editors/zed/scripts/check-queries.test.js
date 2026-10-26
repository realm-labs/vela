"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { runQueryProcess } = require("./query-process");

const grammar = path.resolve(__dirname, "../../tree-sitter-vela");
const fixture = "../../site/src/syntax/fixtures/complete.vela";
const source = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true })
  .decode(fs.readFileSync(path.resolve(grammar, fixture)));
const lines = source.split("\n").map((line) => Buffer.from(line));

// Prefer the existing local devDependency's native binary, then PATH. An explicit
// executable can select another installed version; this test never downloads one.
const localCli = path.join(grammar, "node_modules/tree-sitter-cli/tree-sitter");
const cli = process.env.VELA_TREE_SITTER_CLI || (fs.existsSync(localCli) ? localCli : "tree-sitter");

async function treeSitter(...args) {
  try {
    const result = await runQueryProcess(cli, args, { cwd: grammar });
    process.stderr.write(result.stderr);
    return result.stdout;
  } catch (error) {
    error.message += `\n${error.result?.stderr ?? ""}\nInstall tree-sitter-cli or set VELA_TREE_SITTER_CLI to an installed executable.`;
    throw error;
  }
}

test.before(async () => console.log((await treeSitter("--version")).trim()));

function offset([row, column]) {
  assert(row >= 0 && row < lines.length);
  assert(column >= 0 && column <= lines[row].length);
  return lines.slice(0, row).reduce((length, line) => length + line.length + 1, 0) + column;
}

async function query(name) {
  const output = await treeSitter("query", `../zed/languages/vela/${name}.scm`, fixture);
  const matches = output.split(/  pattern: \d+\r?\n/).slice(1).map((match) =>
    [...match.matchAll(/capture: (?:\d+ - )?([\w.]+), start: \((\d+), (\d+)\), end: \((\d+), (\d+)\)/g)]
      .map(([, name, row, column, endRow, endColumn]) => {
        const start = [Number(row), Number(column)];
        const end = [Number(endRow), Number(endColumn)];
        assert(offset(end) > offset(start), `${name}: nonempty valid byte range`);
        return { name, start, end, text: Buffer.from(source).subarray(offset(start), offset(end)).toString("utf8") };
      })
  );
  assert(matches.length > 0, `${name}: no query matches`);
  assert(matches.every((match) => match.length > 0), `${name}: unparsed match`);
  assert.equal(matches.flat().length, [...output.matchAll(/capture:/g)].length, `${name}: unparsed capture`);
  return matches;
}

function capture(matches, name, start, end, text) {
  const found = matches.flat().filter((item) => item.name === name &&
    item.start.toString() === start.toString() && item.end.toString() === end.toString());
  assert.equal(found.length, 1, `${name}: expected exactly one capture at ${start}..${end}`);
  if (text !== undefined) assert.equal(found[0].text, text);
  return found[0];
}

function pair(matches, first, second) {
  assert(matches.some((match) => match.includes(first) && match.includes(second)), "captures must belong to the same query match");
}

test("current retained-positive sources parse without ERROR or MISSING recovery", async () => {
  for (const file of [fixture, "../../examples/src/bin/native_function/main.vela", "../../examples/src/bin/container_type_hints/main.vela"]) {
    const tree = await treeSitter("parse", file);
    assert.match(tree, /\(source_file /);
    assert.doesNotMatch(tree, /\b(?:ERROR|MISSING)\b/, file);
  }
});

test("shipped brackets pair exact parentheses, braces and brackets", async () => {
  const matches = await query("brackets");
  for (const match of matches) {
    assert.deepEqual(match.map((item) => item.name), ["open", "close"]);
    const [open, close] = match;
    assert.equal({ "(": ")", "{": "}", "[": "]" }[open.text], close.text);
    assert(offset(open.end) <= offset(close.start));
  }
  pair(matches, capture(matches, "open", [4, 18], [4, 19], "{"), capture(matches, "close", [8, 0], [8, 1], "}"));
  pair(matches, capture(matches, "open", [44, 16], [44, 17], "["), capture(matches, "close", [44, 24], [44, 25], "]"));
  pair(matches, capture(matches, "open", [25, 9], [25, 10], "("), capture(matches, "close", [25, 26], [25, 27], ")"));
});

test("shipped indents pair containing regions with their closing end", async () => {
  const matches = await query("indents");
  for (const match of matches) {
    assert.deepEqual(match.map((item) => item.name), ["indent", "end"]);
    const [indent, end] = match;
    assert(offset(indent.start) < offset(end.start));
    assert.deepEqual(indent.end, end.end);
    assert(["}", "]", ")"].includes(end.text));
  }
  pair(matches, capture(matches, "indent", [40, 49], [62, 1]), capture(matches, "end", [62, 0], [62, 1], "}"));
  pair(matches, capture(matches, "indent", [44, 16], [44, 25], "[1, 2, 3]"), capture(matches, "end", [44, 24], [44, 25], "]"));
  pair(matches, capture(matches, "indent", [25, 9], [25, 27], "(self, delta: i64)"), capture(matches, "end", [25, 26], [25, 27], ")"));
});

test("shipped outline selects declarations and their names, not uses", async () => {
  const matches = await query("outline");
  // Zero-based source coordinates, authored against complete.vela, not a query snapshot.
  const declarations = [
    ["Player", [4, 4], [8, 1], [4, 11], [4, 17]],
    ["Stats", [10, 0], [16, 1], [10, 7], [10, 12]],
    ["Reward", [18, 0], [22, 1], [18, 5], [18, 11]],
    ["Tickable", [24, 0], [26, 1], [24, 6], [24, 14]],
    ["Tickable", [28, 0], [33, 1], [28, 5], [28, 13]],
    ["tick", [29, 2], [32, 3], [29, 5], [29, 9]],
    ["START_LEVEL", [37, 0], [37, 26], [37, 6], [37, 17]],
    ["current_player", [38, 0], [38, 64], [38, 6], [38, 20]],
    ["award", [40, 0], [62, 1], [40, 3], [40, 8]],
  ];
  assert.equal(matches.length, declarations.length);
  assert.equal(matches.flat().length, declarations.length * 2);
  for (const [name, start, end, nameStart, nameEnd] of declarations) {
    pair(matches, capture(matches, "item", start, end), capture(matches, "name", nameStart, nameEnd, name));
  }
});

test("shipped textobjects preserve body/outer declaration and comment boundaries", async () => {
  const matches = await query("textobjects");
  for (const [kind, start, insideStart, end] of [
    ["class", [4, 4], [4, 18], [8, 1]],
    ["class", [10, 0], [10, 13], [16, 1]],
    ["class", [18, 0], [18, 12], [22, 1]],
    ["class", [24, 0], [24, 15], [26, 1]],
    ["class", [28, 0], [28, 25], [33, 1]],
    ["function", [29, 2], [29, 38], [32, 3]],
    ["function", [40, 0], [40, 49], [62, 1]],
  ]) {
    const outer = capture(matches, `${kind}.around`, start, end);
    const inner = capture(matches, `${kind}.inside`, insideStart, end);
    pair(matches, outer, inner);
    assert(inner.text.startsWith("{") && inner.text.endsWith("}"));
    assert(offset(outer.start) < offset(inner.start));
  }
  capture(matches, "comment.around", [2, 0], [2, 44], "// Module-level declarations and attributes.");
  assert.equal(matches.flat().length, 15);
});

test("shipped highlights capture declarations, members, literals and operators", async () => {
  const matches = await query("highlights");
  capture(matches, "type", [4, 11], [4, 17], "Player");
  capture(matches, "property", [5, 2], [5, 4], "id");
  capture(matches, "type.builtin", [5, 6], [5, 9], "i64");
  capture(matches, "function", [40, 3], [40, 8], "award");
  capture(matches, "variable.parameter", [40, 9], [40, 15], "player");
  capture(matches, "constant", [38, 6], [38, 20], "current_player");
  capture(matches, "keyword", [38, 0], [38, 5], "state");
  capture(matches, "number", [14, 14], [14, 19], "12i32");
  capture(matches, "operator", [30, 18], [30, 20], "+=");
  capture(matches, "string.special", [41, 12], [41, 48], 'f"award {{escaped}} {player.name}\\n"');
  capture(matches, "function.method", [44, 26], [44, 29], "map");
  capture(matches, "comment", [2, 0], [2, 44], "// Module-level declarations and attributes.");
});

test("shipped overrides include complete quoted strings and comments", async () => {
  const matches = await query("overrides");
  capture(matches, "comment.inclusive", [2, 0], [2, 44], "// Module-level declarations and attributes.");
  for (const [start, end, text] of [
    [[3, 14], [3, 22], '"Player"'],
    [[3, 32], [3, 38], '"game"'],
    [[3, 40], [3, 47], '"state"'],
    [[6, 17], [6, 24], '"guest"'],
    [[41, 12], [41, 48], 'f"award {{escaped}} {player.name}\\n"'],
    [[42, 14], [42, 17], "'x'"],
    [[43, 22], [43, 28], '"gold"'],
    [[47, 14], [47, 20], '"gold"'],
    [[54, 20], [54, 26], '"item"'],
    [[57, 31], [57, 38], '"empty"'],
    [[58, 20], [58, 29], '"unknown"'],
  ]) capture(matches, "string", start, end, text);
  assert.equal(matches.flat().length, 12);
});
