// .eng decoders for Lords of the Realm II.  Three unrelated formats share the extension.
//   node eng.js l2      <L2.eng>            index+blob string table
//   node eng.js battles <BATTLES.ENG>       plain text, CR/LF -> NUL, name/name/description triples
//   node eng.js troops  <TROOPS*.ENG>       plain text numeric table
//   node eng.js validate <dir>              run every check over an install
// Documented in docs/formats/eng.md.  Read-only.
"use strict";
const fs = require("fs"), path = require("path");

/* ---------- L2.eng : "Textfile" + 24-bit offset table + NUL-terminated blob ---------- */
function l2Parse(buf) {
  const magic = buf.toString("latin1", 0, 8);
  const TABLE = 8;
  const ent = i => buf[TABLE + i*4] | (buf[TABLE + i*4 + 1] << 8) | (buf[TABLE + i*4 + 2] << 16);
  const first = ent(1);                       // group 0 is always 0 / unused
  const slots = (first - TABLE) / 4;
  const off = [];
  for (let i = 0; i < slots; i++) off.push(ent(i));
  const groups = [];
  for (let g = 0; g < slots; g++) {
    if (g === 0) { groups.push(null); continue; }
    const s = off[g], e = (g + 1 < slots) ? off[g + 1] : buf.length;
    const strs = []; let i = s;
    while (i < e) { let j = i; while (j < e && buf[j] !== 0) j++; if (j >= e) break; strs.push(buf.toString("latin1", i, j)); i = j + 1; }
    groups.push(strs);
  }
  return { magic, slots, off, groups, hiBytes: Array.from({length: slots}, (_, i) => buf[TABLE + i*4 + 3]) };
}
function l2Report(p, buf) {
  const r = l2Parse(buf);
  let fail = 0; const ok = (c, m) => { if (!c) fail++; console.log((c ? "  OK   " : "  FAIL ") + m); };
  console.log("file: " + p + "  size " + buf.length);
  ok(r.magic === "Textfile", 'magic is "Textfile"');
  ok(Number.isInteger(r.slots), "table length closes: (entry[1]-8)/4 = " + r.slots + " slots (group ids 0.." + (r.slots-1) + ")");
  ok(r.off[0] === 0, "group 0 offset is 0 (unused)");
  ok(r.hiBytes.every(b => b === 0), "4th byte of every table slot is 0 (offsets are 24-bit)");
  let mono = true; for (let i = 2; i < r.slots; i++) if (r.off[i] < r.off[i-1]) mono = false;
  ok(mono, "offsets are non-decreasing from group 1");
  const ctl = {}; for (let i = r.off[1]; i < buf.length; i++) if (buf[i] < 0x20) ctl[buf[i]] = (ctl[buf[i]]||0)+1;
  ok(Object.keys(ctl).every(k => k === "0"), "blob contains no control byte other than NUL");
  let n = 0, bad = 0;
  for (let g = 1; g < r.slots; g++) {
    const s = r.off[g], e = (g + 1 < r.slots) ? r.off[g + 1] : buf.length;
    let i = s; while (i < e) { let j = i; while (j < e && buf[j] !== 0) j++; if (j >= e) { bad++; break; } n++; i = j + 1; }
  }
  ok(bad === 0, "every group region decomposes exactly into NUL-terminated strings");
  console.log("  info  " + n + " strings in " + (r.slots - 1) + " groups; " +
              r.groups.filter(g => g && g.length === 0).length + " groups empty");
  return { fail, r };
}

/* ---------- BATTLES.ENG : plain text; the game NULs every byte < 0x20 ---------- */
function battlesParse(buf) {
  const t = Buffer.from(buf);
  for (let i = 0; i < t.length; i++) if (t[i] < 0x20) t[i] = 0;
  const fields = []; let i = 0;
  while (i < t.length) { let j = i; while (j < t.length && t[j] !== 0) j++; fields.push(t.toString("latin1", i, j)); i = j + 1; }
  const nonEmpty = fields.filter(s => s.length);
  const recs = [];
  for (let k = 0; k + 2 < nonEmpty.length; k += 3)
    recs.push({ index: recs.length, shortName: nonEmpty[k], fullName: nonEmpty[k+1], description: nonEmpty[k+2] });
  return { fields: fields.length, nonEmpty: nonEmpty.length, recs };
}

/* ---------- TROOPS*.ENG : plain text, mirrors Lords2.exe FUN_0042ac0c ---------- */
const TROOP_COLS = ["Pe","Xb","Ma","Sw","Pi","Ar","Kn","Ca","To","Ra","Oi"];
function troopsParse(buf) {
  const s = buf.toString("latin1");
  let i = s.indexOf("*");
  if (i < 0) return null;
  const adv = [], tab = [];
  let row = 0, group = 0, side = 0, type = 0, pending = false, count = 0;
  while (i < s.length) {
    const c = s.charCodeAt(i);
    if (c < 0x30 || c > 0x39) { i++; continue; }
    let j = i; while (j < s.length && s.charCodeAt(j) >= 0x30 && s.charCodeAt(j) <= 0x39) j++;
    const v = parseInt(s.slice(i, j), 10); count++;
    if (type || side || group || pending) {
      tab[row] = tab[row] || []; tab[row][group] = tab[row][group] || [];
      tab[row][group][side] = tab[row][group][side] || [];
      tab[row][group][side][type] = v;
      type++; if (type > 10) { type = 0; side++; if (side > 1) { side = 0; group++; if (group > 4) { group = 0; row++; } } }
      pending = false;
    } else { adv[row] = Math.max(0, Math.min(10, v)); pending = true; }
    if (row > 0x22) break;
    i = j;
  }
  return { count, rows: row, adv, tab };
}

function usage() { console.log("usage: node eng.js l2|battles|troops|validate <path>"); }
const [, , cmd, target] = process.argv;
if (cmd === "l2") { l2Report(target, fs.readFileSync(target)); }
else if (cmd === "battles") {
  const r = battlesParse(fs.readFileSync(target));
  console.log("fields=" + r.fields + " non-empty=" + r.nonEmpty + " records=" + r.recs.length);
  for (const x of r.recs) console.log(x.index + "  " + JSON.stringify(x.shortName) + " | " + JSON.stringify(x.fullName) + " | " + JSON.stringify(x.description.slice(0, 60)));
} else if (cmd === "troops") {
  const r = troopsParse(fs.readFileSync(target));
  console.log("numbers=" + r.count + " (expected 35*(1+5*2*11)=" + 35*111 + ") rows=" + r.rows);
  console.log("cols: " + TROOP_COLS.join(" "));
  for (let row = 0; row < 35; row++)
    console.log("row " + String(row).padStart(2) + " adv=" + r.adv[row] +
                "  grp2 att: " + r.tab[row][2][0].join(" ") + "   def: " + r.tab[row][2][1].join(" "));
} else if (cmd === "validate") {
  const dir = target;
  const find = n => { const f = fs.readdirSync(dir).find(x => x.toLowerCase() === n); return f && path.join(dir, f); };
  let fail = 0;
  for (const n of ["l2.eng"]) { const f = find(n); if (f) fail += l2Report(f, fs.readFileSync(f)).fail; }
  const bf = find("battles.eng");
  if (bf) { const r = battlesParse(fs.readFileSync(bf));
    console.log("file: " + bf); 
    const c = r.nonEmpty % 3 === 0;
    console.log((c ? "  OK   " : "  FAIL ") + "non-empty field count " + r.nonEmpty + " is a multiple of 3 -> " + r.recs.length + " records");
    if (!c) fail++; }
  for (const n of ["troops.eng", "troops2.eng", "troops3.eng"]) {
    const f = find(n); if (!f) continue;
    const r = troopsParse(fs.readFileSync(f));
    const c = r.count === 3885 && r.rows === 35;
    console.log("file: " + f); console.log((c ? "  OK   " : "  FAIL ") + "3885 numbers = 35 rows x (1 + 5 groups x 2 sides x 11 types)");
    if (!c) fail++;
    const over = [];
    for (let row = 0; row < 35; row++) for (let g = 0; g < 5; g++) for (let sd = 0; sd < 2; sd++)
      for (let t = 7; t < 11; t++) if ((r.tab[row][g][sd][t] || 0) > 9) over.push([row,g,sd,t]);
    console.log((over.length === 0 ? "  OK   " : "  FAIL ") + "siege columns Ca/To/Ra/Oi all <= 9 (the game clamps them to 9)");
    if (over.length) fail++;
  }
  console.log(fail === 0 ? "RESULT: all checks passed" : "RESULT: " + fail + " check(s) failed");
  process.exit(fail ? 1 : 0);
} else usage();
