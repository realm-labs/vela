"use strict";
const { parseMarkers } = require("./fixtures");
const selection = require("./selection-oracle");

function transform(source, crlf = false, shifted = false) {
  if (shifted) {
    if (source.startsWith("[[file:start]][[shebang:start]]")) {
      const end = source.indexOf("[[shebang:end]]");
      const separator = source.slice(0, end).endsWith("\n") ? "" : "\n";
      source = source.replace("[[shebang:end]]", separator + "[[shebang:end]]// shifted 中😀\n/* second 😀 */\n");
    } else {
      source = source.replace("[[file:start]]", "[[file:start]]// shifted 中😀\n/* second 😀 */\n");
    }
  }
  return crlf ? source.replaceAll("\n", "\r\n") : source;
}
function document(source, crlf = false, shifted = false) {
  return parseMarkers(transform(source, crlf, shifted));
}
function lexical(doc, descriptor, descriptors) {
  const bytes = Buffer.from(doc.text), raw = doc.markers[descriptor.marker];
  let start = raw.start.byte, end = raw.end.byte;
  const space = byte => [9, 10, 13, 32].includes(byte);
  if (descriptor.kind === "LineComment" && bytes.subarray(end, end + 2).equals(Buffer.from("\r\n"))) end++;
  if (descriptor.kind === "Whitespace") {
    if (bytes.subarray(start, start + 2).equals(Buffer.from("\r\n")) && descriptors.some(t =>
      t.kind === "LineComment" && doc.markers[t.marker].end.byte === start)) start++;
    else while (start && space(bytes[start - 1])) start--;
    while (space(bytes[end])) end++;
  }
  return { start, end, text: bytes.subarray(start, end).toString("utf8") };
}
module.exports = { ...selection, transform, document, lexical };
