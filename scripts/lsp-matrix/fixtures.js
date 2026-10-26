"use strict";

// Test-only coordinates. Do not import production LineIndex/provider code here.
const fs = require("node:fs");
const path = require("node:path");

function parseMarkers(source) {
  if (typeof source !== "string" || !source.isWellFormed()) throw new Error("fixture must contain valid Unicode");
  let text = "", byte = 0, line = 0, character = 0;
  const markers = {}, starts = new Map(), stack = [];
  const position = () => ({ byte, line, character });
  for (let index = 0; index < source.length;) {
    if (source.startsWith("[[", index)) {
      const end = source.indexOf("]]", index + 2);
      if (end < 0) throw new Error("unclosed marker");
      const token = source.slice(index + 2, end);
      const match = /^([a-z][a-z0-9-]*)(?::(start|end))?$/.exec(token);
      if (!match) throw new Error(`invalid marker ${token}`);
      const [, name, kind] = match;
      if (text.endsWith("\r") && source[end + 2] === "\n") throw new Error("marker splits CRLF");
      if (kind === "end") {
        if (stack.pop() !== name) throw new Error(`unpaired or crossing marker ${name}`);
        markers[name] = { start: starts.get(name), end: position() };
        starts.delete(name);
      } else {
        if (Object.hasOwn(markers, name) || starts.has(name)) throw new Error(`duplicate marker ${name}`);
        if (kind === "start") { starts.set(name, position()); stack.push(name); }
        else markers[name] = { start: position(), end: position() };
      }
      index = end + 2;
    } else {
      if (source.startsWith("]]", index)) throw new Error("unexpected marker close");
      const ch = String.fromCodePoint(source.codePointAt(index));
      text += ch; byte += Buffer.byteLength(ch); index += ch.length;
      if (ch === "\n") { line++; character = 0; } else character += ch.length;
    }
  }
  if (stack.length) throw new Error("unclosed range marker");
  return { text, markers };
}

function safeFile(file) {
  if (typeof file !== "string" || !file || file.includes("\\") || file.includes(":") || file.includes("\0") ||
      file.split("/").some((part) => !part || part === "." || part === "..")) throw new Error(`invalid fixture path ${file}`);
  return file;
}

class FixtureWorkspace {
  constructor(spec) {
    if (spec.version !== 1 || !spec.id || !spec.files || !Object.keys(spec.files).length || !Array.isArray(spec.actions)) {
      throw new Error("invalid fixture workspace");
    }
    this.spec = spec;
    this.disk = new Map(Object.entries(spec.files).map(([file, source]) => [safeFile(file), parseMarkers(source)]));
    this.open = new Map();
  }
  document(file) { return this.open.get(safeFile(file)) ?? this.disk.get(file); }
  apply(action) {
    const file = safeFile(action.file);
    switch (action.op) {
      case "open":
        if (this.open.has(file) || !this.disk.has(file)) throw new Error("open requires a closed disk file");
        this.open.set(file, this.disk.get(file)); break;
      case "change":
        if (!this.open.has(file)) throw new Error("change requires an open file");
        this.open.set(file, parseMarkers(action.source)); break;
      case "save":
        if (!this.open.has(file)) throw new Error("save requires an open file");
        this.disk.set(file, this.open.get(file)); break;
      case "close":
        if (!this.open.delete(file)) throw new Error("close requires an open file"); break;
      case "write": this.disk.set(file, parseMarkers(action.source)); break;
      case "delete":
        if (!this.disk.delete(file)) throw new Error("delete requires a disk file"); break;
      default: throw new Error(`unknown fixture action ${action.op}`);
    }
  }
  materialize(root) {
    if (fs.existsSync(root)) throw new Error("fixture root must be new and isolated");
    fs.mkdirSync(root, { recursive: true });
    this.appendTo(root);
  }
  appendTo(root) {
    root = path.resolve(root); // A trailing separator would make lstat follow a directory symlink.
    if (!fs.lstatSync(root).isDirectory()) throw new Error("fixture append requires an existing directory");
    // Fixture roots are exclusively owned, with uniform filesystem naming rules.
    // Probe the complete planned tree on that filesystem rather than guessing its
    // case/Unicode equivalence. No destination files are written during preflight;
    // this is not a transaction against concurrent mutation or later I/O failures.
    const probeRoot = fs.realpathSync(fs.mkdtempSync(path.join(root, ".fixture-append-")));
    try {
      for (const file of this.disk.keys()) {
        const parts = safeFile(file).split("/");
        for (let i = 1; i <= parts.length; i++) {
          const relative = parts.slice(0, i).join("/");
          const target = path.join(root, relative);
          const stat = fs.lstatSync(target, { throwIfNoEntry: false });
          if (i === parts.length ? stat : this.disk.has(relative) ||
              (stat && (!stat.isDirectory() || fs.realpathSync(target) === probeRoot))) {
            throw new Error(`fixture append collision: ${file} at ${relative}`);
          }
        }
        const probe = path.join(probeRoot, file);
        fs.mkdirSync(path.dirname(probe), { recursive: true });
        fs.writeFileSync(probe, "", { flag: "wx" });
      }
    } finally { fs.rmSync(probeRoot, { recursive: true, force: true }); }
    for (const [file, document] of this.disk) {
      const target = path.join(root, file);
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.writeFileSync(target, document.text, { flag: "wx" });
    }
  }
}

function offsetAt(text, position) {
  if (![position.line, position.character].every((n) => Number.isSafeInteger(n) && n >= 0)) throw new Error("invalid position");
  const lines = text.split("\n");
  const row = lines[position.line];
  if (row === undefined) throw new Error("line out of bounds");
  const content = row.endsWith("\r") ? row.slice(0, -1) : row;
  if (position.character > content.length) throw new Error("character out of bounds");
  const prefix = content.slice(0, position.character);
  if (!prefix.isWellFormed()) throw new Error("position splits surrogate pair");
  return lines.slice(0, position.line).reduce((n, line) => n + line.length + 1, 0) + position.character;
}

function applyEdits(text, edits) {
  const ordered = edits.map(({ range, newText }) => {
    if (typeof newText !== "string" || !newText.isWellFormed()) throw new Error("invalid replacement");
    const start = offsetAt(text, range.start), end = offsetAt(text, range.end);
    if (end < start) throw new Error("reversed range");
    return { start, end, newText };
  }).sort((a, b) => a.start - b.start || a.end - b.end);
  for (let i = 1; i < ordered.length; i++) {
    if (ordered[i].start < ordered[i - 1].end || ordered[i].start === ordered[i - 1].start) throw new Error("overlapping edits");
  }
  return ordered.reverse().reduce((result, edit) => result.slice(0, edit.start) + edit.newText + result.slice(edit.end), text);
}

module.exports = { parseMarkers, safeFile, FixtureWorkspace, offsetAt, applyEdits };
