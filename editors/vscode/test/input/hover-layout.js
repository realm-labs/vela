"use strict";

// Reviewed source line plus its two authored type/parameter hints. Inlay hints
// shift the pointer target after the provider response but before the next paint.
// Wait for those actual labels before measuring; never retry accepted input.
function hoverLineReady(lines) {
  // The pinned SDK uses width-only padding, not an extra DOM space.
  const reviewed = "/* 中😀 */ let value: i64 = combine(left:1);";
  return lines.filter(line => line.replaceAll("\u00a0", " ").trim() === reviewed).length === 1;
}
module.exports = { hoverLineReady };
