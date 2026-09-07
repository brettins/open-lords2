// Dump the DIALOG resources of a PE32 image.
//
//   node tools/net/dlgdump.js "F:/games/Lords of the Realm II/Lords2.exe" [id]
//
// Lords2's entire multiplayer setup UI is standard Win32 dialogs called through
// DialogBoxParamA - not the game's own DirectDraw UI. That is what makes the
// network path drivable at all: a dialog is a real window with real controls
// that accepts posted messages, whereas the fullscreen DirectDraw surface does
// not (docs/decisions.md D8). This script recovers the control IDs needed to
// drive them.
//
// Handles both the classic DLGTEMPLATE and the extended DLGTEMPLATEEX layout.
const fs = require('fs');
const file = process.argv[2];
const only = process.argv[3] !== undefined ? parseInt(process.argv[3], 10) : null;
const b = fs.readFileSync(file);
const pe = b.readUInt32LE(0x3c);
const nsec = b.readUInt16LE(pe + 6), optSize = b.readUInt16LE(pe + 20), opt = pe + 24;
const imageBase = b.readUInt32LE(opt + 28);
const secOff = opt + optSize;
const secs = [];
for (let i = 0; i < nsec; i++) {
  const o = secOff + i * 40;
  secs.push({
    name: b.toString('ascii', o, o + 8).replace(/\0+$/, ''),
    va: b.readUInt32LE(o + 12), vs: b.readUInt32LE(o + 8),
    raw: b.readUInt32LE(o + 20), rs: b.readUInt32LE(o + 16),
  });
}
const r2o = r => { for (const s of secs) if (r >= s.va && r < s.va + Math.max(s.vs, s.rs)) return s.raw + (r - s.va); return -1; };
const rsrcRva = b.readUInt32LE(opt + 96 + 2 * 8);
const rsrcOff = r2o(rsrcRva);

function entries(dirOff) {
  const nName = b.readUInt16LE(dirOff + 12), nId = b.readUInt16LE(dirOff + 14);
  const out = [];
  for (let i = 0; i < nName + nId; i++) {
    const e = dirOff + 16 + i * 8;
    const nameField = b.readUInt32LE(e), offField = b.readUInt32LE(e + 4);
    let name;
    if (nameField & 0x80000000) {
      const no = rsrcOff + (nameField & 0x7fffffff);
      const len = b.readUInt16LE(no);
      name = b.toString('utf16le', no + 2, no + 2 + len * 2);
    } else name = nameField;
    out.push({ name, off: rsrcOff + (offField & 0x7fffffff), isDir: !!(offField & 0x80000000) });
  }
  return out;
}

// Type 5 = RT_DIALOG
const root = entries(rsrcOff);
const dlgType = root.find(e => e.name === 5);
if (!dlgType) { console.log('no RT_DIALOG resources'); process.exit(0); }

// Classic dialog control classes are encoded as 0xFFFF followed by an atom.
const ATOM = { 0x80: 'BUTTON', 0x81: 'EDIT', 0x82: 'STATIC', 0x83: 'LISTBOX', 0x84: 'SCROLLBAR', 0x85: 'COMBOBOX' };

function readSz(o) {   // returns [text, nextOffset]; handles 0xFFFF ordinal form
  if (b.readUInt16LE(o) === 0xffff) return ['#' + b.readUInt16LE(o + 2), o + 4];
  let e = o;
  while (b.readUInt16LE(e) !== 0) e += 2;
  return [b.toString('utf16le', o, e), e + 2];
}
const align4 = o => (o + 3) & ~3;

function dumpDialog(id, off, size) {
  const sig = b.readUInt16LE(off + 2);
  const ex = (b.readUInt16LE(off) === 1 && sig === 0xffff);
  let o, style, nItems, cx, cy, x, y;
  if (ex) {
    style = b.readUInt32LE(off + 12);
    nItems = b.readUInt16LE(off + 16);
    x = b.readInt16LE(off + 18); y = b.readInt16LE(off + 20);
    cx = b.readInt16LE(off + 22); cy = b.readInt16LE(off + 24);
    o = off + 26;
  } else {
    style = b.readUInt32LE(off);
    nItems = b.readUInt16LE(off + 8);
    x = b.readInt16LE(off + 10); y = b.readInt16LE(off + 12);
    cx = b.readInt16LE(off + 14); cy = b.readInt16LE(off + 16);
    o = off + 18;
  }
  let menu, cls, title;
  [menu, o] = readSz(o);
  [cls, o] = readSz(o);
  [title, o] = readSz(o);
  if (style & 0x40) o += ex ? 6 : 2;                 // DS_SETFONT: point size (+ more in EX)
  if (style & 0x40) { const r = readSz(o); o = r[1]; }  // face name
  console.log(`\n=== DIALOG ${id} ${ex ? '(EX)' : ''} "${title}" ===`);
  console.log(`  style 0x${style.toString(16)}  ${cx}x${cy} at ${x},${y}  ${nItems} control(s)`);
  for (let i = 0; i < nItems; i++) {
    o = align4(o);
    let cstyle, cid, cxx, cyy, cw, ch;
    if (ex) {
      cstyle = b.readUInt32LE(o + 8);
      cxx = b.readInt16LE(o + 16); cyy = b.readInt16LE(o + 18);
      cw = b.readInt16LE(o + 20); ch = b.readInt16LE(o + 22);
      cid = b.readUInt32LE(o + 24);
      o += 28;
    } else {
      cstyle = b.readUInt32LE(o);
      cxx = b.readInt16LE(o + 8); cyy = b.readInt16LE(o + 10);
      cw = b.readInt16LE(o + 12); ch = b.readInt16LE(o + 14);
      cid = b.readUInt16LE(o + 16);
      o += 18;
    }
    let ccls, ctext;
    if (b.readUInt16LE(o) === 0xffff) { ccls = ATOM[b.readUInt16LE(o + 2)] || ('atom ' + b.readUInt16LE(o + 2)); o += 4; }
    else [ccls, o] = readSz(o);
    [ctext, o] = readSz(o);
    const extra = b.readUInt16LE(o); o += 2 + extra;
    console.log(`    id ${String(cid).padStart(5)}  ${ccls.padEnd(9)} style 0x${cstyle.toString(16).padStart(8, '0')}  ${String(cw).padStart(4)}x${String(ch).padStart(3)} at ${cxx},${cyy}  "${ctext}"`);
  }
}

for (const d of entries(dlgType.off)) {
  if (only !== null && d.name !== only) continue;
  const langs = entries(d.off);
  for (const l of langs) {
    const dataRva = b.readUInt32LE(l.off);
    const size = b.readUInt32LE(l.off + 4);
    try { dumpDialog(d.name, r2o(dataRva), size); }
    catch (e) { console.log(`\n=== DIALOG ${d.name} === parse failed: ${e.message}`); }
  }
}
