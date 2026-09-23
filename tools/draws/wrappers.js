'use strict';
// Our one-draw helpers, counted per call site as the original's leaves are.
// `job/common.rs` say/count/zero/delta each stand for one original leaf
// (Eng_DrawString, Ui_DrawCount, Ui_DrawNumber, the +/- number); counted by
// their bodies they read as 5 draws however often a panel calls them.
//
// A helper is a `fn` in the module whose body makes exactly one draw, or one
// named in MULTI: several of our calls standing for one original draw.
const MULTI = new Map(Object.entries({
  delta: 'a signed number: space, then Ui_DrawNumber with its lead',
  report_value: 'Ui_DrawNumber, zero or signed',
  bevel_rect: 'Ui_DrawBevelRect / FUN_00403EE4',
}));

const FN = /^(\s*)(?:pub(?:\([a-z]+\))?\s+)?fn\s+([a-z_][a-z_0-9]*)\s*[<(]/;

/**
 * `sources`: [{file, lines}]; `drawsOn(line)`: the draw calls on a line, as
 * [{name}]. Returns the helpers: name -> {file, from, to, draws}.
 */
function helpers(sources, drawsOn) {
  const out = new Map();
  for (const { file, lines } of sources) {
    for (let i = 0; i < lines.length; i++) {
      const m = FN.exec(lines[i]);
      if (!m) continue;
      const close = new RegExp(`^${m[1]}}\\s*$`);
      let j = i + 1;
      while (j < lines.length && !close.test(lines[j])) j++;
      let draws = 0;
      for (let k = i; k <= j && k < lines.length; k++) draws += drawsOn(lines[k]).length;
      if (draws === 1 || (draws > 0 && MULTI.has(m[2]))) {
        out.set(m[2], { file, from: i, to: j, draws });
      }
      i = j;
    }
  }
  return out;
}

/** Call sites of each helper, outside its own body: [{file, line, name, text}]. */
function helperCalls(sources, hs) {
  const calls = [];
  if (!hs.size) return calls;
  const names = [...hs.keys()].join('|');
  const re = new RegExp(`(^|[^.\\w])(?:[a-z_]+::)*(${names})\\s*\\(`, 'g');
  for (const { file, lines } of sources) {
    lines.forEach((l, i) => {
      if (/^\s*(\/\/|\/\*|\*)/.test(l) || FN.test(l)) return;
      let m; re.lastIndex = 0;
      while ((m = re.exec(l))) calls.push({ file, line: i + 1, name: m[2], text: l.trim() });
    });
  }
  return calls;
}

/** Is line `i` of `file` inside a helper's body? */
function inHelper(hs, file, i) {
  for (const h of hs.values()) if (h.file === file && i >= h.from && i <= h.to) return true;
  return false;
}

module.exports = { helpers, helperCalls, inHelper, MULTI };
