"use strict";

// DOM observations only. Coordinates and source bytes come from reviewed markers,
// never from Monaco's model, a token provider or a language client.
async function tokenGeometry(editor, source) {
  return editor.evaluate((element, { text, markers }) => {
    const lines = text.split(/\r?\n/),
      viewport = element.getBoundingClientRect();
    const normalize = (text) => text.replaceAll("\u00a0", " ");
    const rendered = [
      ...element.querySelectorAll(".view-lines > .view-line"),
    ].map((line) => {
      const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT),
        nodes = [];
      let node;
      while ((node = walker.nextNode())) {
        if (!node.parentElement?.closest('[class*="dyn-rule-"]'))
          nodes.push(node);
      }
      return {
        line,
        nodes,
        text: normalize(nodes.map((node) => node.textContent).join("")),
      };
    });
    const glyphs = {};
    for (const [name, marker] of Object.entries(markers)) {
      const expectedLine = lines[marker.start.line];
      const rows = rendered.filter((row) => row.text === expectedLine);
      if (rows.length !== 1) {
        glyphs[name] = { visible: false, rows: rows.length };
        continue;
      }
      const row = rows[0],
        range = document.createRange(),
        styles = [];
      let offset = 0,
        started = false,
        ended = false;
      for (const node of row.nodes) {
        const next = offset + node.textContent.length;
        if (next > marker.start.character && offset < marker.end.character) {
          const css = getComputedStyle(node.parentElement);
          styles.push({
            color: css.color,
            fontStyle: css.fontStyle,
            fontWeight: css.fontWeight,
            textDecorationLine: css.textDecorationLine,
          });
        }
        if (
          !started &&
          marker.start.character >= offset &&
          marker.start.character < next
        ) {
          range.setStart(node, marker.start.character - offset);
          started = true;
        }
        if (
          started &&
          marker.end.character > offset &&
          marker.end.character <= next
        ) {
          range.setEnd(node, marker.end.character - offset);
          ended = true;
          break;
        }
        offset = next;
      }
      if (!started || !ended) {
        glyphs[name] = { visible: false, incomplete: true };
        continue;
      }
      const rect = range.getBoundingClientRect(),
        lineRect = row.line.getBoundingClientRect();
      const style = styles.every(
        (item) => JSON.stringify(item) === JSON.stringify(styles[0]),
      )
        ? styles[0]
        : { mixed: styles };
      glyphs[name] = {
        text: normalize(range.toString()),
        line: marker.start.line,
        start: marker.start.character,
        end: marker.end.character,
        style,
        visible:
          rect.width > 0 &&
          rect.height > 0 &&
          rect.top >= viewport.top &&
          rect.bottom <= viewport.bottom &&
          rect.left >= viewport.left &&
          rect.right <= viewport.right,
        aligned:
          rect.top >= lineRect.top &&
          rect.bottom <= lineRect.bottom &&
          marker.start.line === marker.end.line,
        rect: rect.toJSON(),
      };
    }
    return {
      glyphs,
      renderedLines: rendered.map((row) => row.text),
      viewport: viewport.toJSON(),
    };
  }, source);
}
module.exports = { tokenGeometry };
