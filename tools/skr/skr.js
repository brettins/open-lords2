// .skr (Lords of the Realm II battle/skirmish scenario) decoder + validator.
//   node skr.js validate <file.skr>
//   node skr.js render   <file.skr> <mapIndex>
//   node skr.js dump     <file.skr>            (JSON of armies + text)
// Layout is documented in docs/formats/skr.md.  Read-only; never writes the input.
"use strict";
const fs = require("fs");

const MAPS      = 20;
const ARMY_SZ   = 44;                       // 11 x u32
const ARMY_OFF  = 0;
const TEXT_OFF  = ARMY_OFF + MAPS * 2 * ARMY_SZ;   // 1760
const TEXT_SZ   = 183;                             // 13 + 29 + 141
const GRID_BASE = TEXT_OFF + MAPS * TEXT_SZ;       // 5420
const GRID_PAD  = 328;                             // unused leading slack
const W = 80, H = 80, LAYER = W * H;               // 6400
const FILE_SZ   = GRID_BASE + GRID_PAD + MAPS * LAYER;   // 133748

const TROOPS = ["peasants","crossbowmen","macemen","swordsmen","pikemen",
                "archers","knights","catapults","siegeTowers","batteringRams","oil"];

// file byte -> { editorMask (mapl2.exe), battleId (Lords2.exe), label }
const TERRAIN = {
  0x00: { mask: null,  id: 1,  label: "open ground" },
  0x02: { mask: 0x002, id: 4,  label: "blocking terrain (11 gfx variants)" },
  0x04: { mask: 0x100, id: 20, label: "deployment marker A" },
  0x09: { mask: 0x001, id: 11, label: "water (49 gfx variants)" },
  0x0a: { mask: 0x020, id: 12, label: "woodland (17 gfx variants)" },
  0x0f: { mask: 0x080, id: 30, label: "deployment marker B" },
  0x10: { mask: 0x004, id: 7,  label: "structure part" },
  0x12: { mask: 0x005, id: 8,  label: "structure head" },
  0x14: { mask: 0x004, id: 9,  label: "structure part (legacy; editor never writes it)" },
  0x15: { mask: 0x040, id: 13, label: "woodland bank, second range" },
  0x20: { mask: 0x008, id: 3,  label: "gfx (rand&7)+0x20" },
  0x50: { mask: 0x010, id: 6,  label: "gfx (rand&7)+0x7c" },
};

function cstr(b, off, len) {
  let e = off; const end = off + len;
  while (e < end && b[e] !== 0) e++;
  return { text: b.toString("latin1", off, e), terminated: e < end, used: e - off };
}

function parse(buf) {
  const armies = [], texts = [], layers = [];
  for (let m = 0; m < MAPS; m++) {
    for (let side = 0; side < 2; side++) {
      const o = ARMY_OFF + (m * 2 + side) * ARMY_SZ, rec = {};
      TROOPS.forEach((t, i) => { rec[t] = buf.readUInt32LE(o + i * 4); });
      armies.push(rec);
    }
    const t = TEXT_OFF + m * TEXT_SZ;
    texts.push({
      shortName:   cstr(buf, t,       13),
      fullName:    cstr(buf, t + 13,  29),
      description: cstr(buf, t + 42, 141),
    });
    const g = GRID_BASE + GRID_PAD + m * LAYER;
    layers.push(buf.subarray(g, g + LAYER));
  }
  return { armies, texts, layers };
}

function validate(path) {
  const b = fs.readFileSync(path);
  let fail = 0;
  const ok = (cond, msg) => { if (!cond) fail++; console.log((cond ? "  OK   " : "  FAIL ") + msg); };
  console.log("file: " + path);
  console.log("size: " + b.length + "  expected " + FILE_SZ +
              " = 20*2*44 + 20*183 + 328 + 20*80*80");
  ok(b.length === FILE_SZ, "file size is exactly " + FILE_SZ);
  if (b.length !== FILE_SZ) return 1;

  const p = parse(b);

  // leading pad
  let padNz = 0;
  for (let i = 0; i < GRID_PAD; i++) if (b[GRID_BASE + i]) padNz++;
  ok(padNz === 0, "328-byte pad at " + GRID_BASE + " is all zero (nonzero=" + padNz + ")");

  // text records
  let unterm = 0, resid = 0;
  p.texts.forEach(t => {
    for (const f of [t.shortName, t.fullName, t.description]) {
      if (!f.terminated) unterm++;
    }
  });
  ok(unterm === 0, "all 60 text fields NUL-terminated inside their slot");
  // residue after the terminator (the editor does not clear slots)
  for (let m = 0; m < MAPS; m++) {
    const base = TEXT_OFF + m * TEXT_SZ;
    for (const [o, len, f] of [[0,13,p.texts[m].shortName],[13,29,p.texts[m].fullName],[42,141,p.texts[m].description]]) {
      for (let i = base + o + f.used + 1; i < base + o + len; i++) if (b[i]) resid++;
    }
  }
  console.log("  note  " + resid + " non-zero bytes after a terminator (stale residue is normal)");

  // terrain alphabet
  const hist = {};
  for (let m = 0; m < MAPS; m++) for (const v of p.layers[m]) hist[v] = (hist[v] || 0) + 1;
  const unknown = Object.keys(hist).map(Number).filter(v => !(v in TERRAIN));
  ok(unknown.length === 0, "terrain alphabet within the 12 values both binaries decode" +
     (unknown.length ? " (unknown: " + unknown.map(v => "0x" + v.toString(16)) + ")" : ""));
  console.log("  hist  " + Object.entries(hist)
      .sort((a, b2) => a[0] - b2[0])
      .map(([v, c]) => "0x" + (+v).toString(16).padStart(2, "0") + ":" + c).join("  "));

  // markers: exactly one of each per map?
  for (let m = 0; m < MAPS; m++) {
    const a = [], d = [];
    p.layers[m].forEach((v, i) => { if (v === 0x04) a.push(i); if (v === 0x0f) d.push(i); });
    if (a.length !== 1 || d.length !== 1) {
      console.log("  FAIL map " + m + " markers: 0x04 x" + a.length + ", 0x0f x" + d.length);
      fail++;
    }
  }
  ok(true, "every map carries exactly one 0x04 and one 0x0f marker");

  // blank-template maps: identical to what mapl2.exe FUN_0041a411 writes
  const blank = Buffer.alloc(LAYER);
  blank[20 * W + 40] = 0x04;
  blank[60 * W + 40] = 0x0f;
  const blanks = [];
  for (let m = 0; m < MAPS; m++) if (p.layers[m].equals(blank)) blanks.push(m);
  console.log("  note  maps byte-identical to the editor's blank template (" +
              blanks.length + "): " + blanks.join(","));

  console.log(fail === 0 ? "RESULT: all checks passed" : "RESULT: " + fail + " check(s) failed");
  return fail;
}

const GLYPH = { 0x00:".", 0x02:"#", 0x04:"A", 0x09:"~", 0x0a:"T", 0x0f:"D",
                0x10:"p", 0x12:"P", 0x14:"q", 0x15:"=", 0x20:"+", 0x50:"o" };

function render(path, idx) {
  const b = fs.readFileSync(path), p = parse(b), L = p.layers[idx];
  const t = p.texts[idx];
  console.log("map " + idx + ": " + JSON.stringify(t.shortName.text) + " / " +
              JSON.stringify(t.fullName.text));
  console.log("    " + Array.from({length: W}, (_, i) => i % 10).join(""));
  for (let y = 0; y < H; y++) {
    let s = "";
    for (let x = 0; x < W; x++) { const v = L[y * W + x]; s += (GLYPH[v] !== undefined ? GLYPH[v] : "?"); }
    console.log(String(y).padStart(3) + " " + s);
  }
  console.log("legend: " + Object.entries(GLYPH)
      .map(([v, g]) => g + "=0x" + (+v).toString(16)).join(" "));
}

function dump(path) {
  const b = fs.readFileSync(path), p = parse(b);
  const out = [];
  for (let m = 0; m < MAPS; m++) out.push({
    index: m,
    shortName: p.texts[m].shortName.text,
    fullName: p.texts[m].fullName.text,
    description: p.texts[m].description.text,
    attacker: p.armies[m * 2],
    defender: p.armies[m * 2 + 1],
  });
  console.log(JSON.stringify(out, null, 2));
}

const [, , cmd, file, arg] = process.argv;
if (cmd === "validate") process.exit(validate(file) ? 1 : 0);
else if (cmd === "render") render(file, parseInt(arg || "0", 10));
else if (cmd === "dump") dump(file);
else console.log("usage: node skr.js validate|render|dump <file.skr> [mapIndex]");
