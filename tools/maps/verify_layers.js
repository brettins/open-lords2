// Verify the plane-0/1/2/3/4 and lattice claims in docs/formats/maps-layers.md
// over ALL used slots of L2_maps.dat. Reads the game files in place; writes nothing.
//   node verify_layers.js ["F:/games/Lords of the Realm II"]
const fs = require('fs');
const D = (process.argv[2] || 'F:/games/Lords of the Realm II').replace(/[\\/]+$/, '') + '/';
const b = fs.readFileSync(D + 'L2_maps.dat');
const REC = 32961, PL = 4096, LW = 65, LH = 129, N = b.length / REC;
const P = (r, p) => b.slice(r * REC + p * PL, r * REC + (p + 1) * PL);
const T = r => b.slice(r * REC + 6 * PL, (r + 1) * REC);
const used = [];
for (let r = 0; r < N; r++) {
  let nc = false;
  for (let p = 0; p < 6 && !nc; p++) { const d = P(r, p); for (let i = 1; i < PL; i++) if (d[i] !== d[0]) { nc = true; break; } }
  if (nc) used.push(r);
}
const pass = [], fail = [];
const chk = (name, ok, detail) => { (ok ? pass : fail).push(name); console.log((ok ? '  OK   ' : '  FAIL ') + name + (detail ? '   ' + detail : '')); };
console.log('file:', D + 'L2_maps.dat', 'slots:', N, 'used:', used.length);

// ---------- 1. bank -> tile set ----------
console.log('\n[1] plane 1 = tile-set (bank) selector');
const SETS = { 0: 'Base1a.pl8', 4: 'Mtns1a.pl8', 8: 'Roads1a.pl8', 12: 'Town1a.pl8', 16: 'Castle1a.pl8' };
const nf = {}; let haveSets = true;
for (const k in SETS) { try { nf[k] = fs.readFileSync(D + SETS[k]).readUInt16LE(2); } catch (e) { haveSets = false; } }
if (!haveSets) console.log('       (tile-set PL8 files not present in this install - skipping the bank checks)');
const seen = {};
for (const r of used) { const p1 = P(r, 1), p2 = P(r, 2); for (let i = 0; i < PL; i++) (seen[p1[i]] = seen[p1[i]] || new Set()).add(p2[i]); }
let allIn = true;
if (haveSets) {
for (const k of Object.keys(seen).map(Number).sort((a, c) => a - c)) {
  const a = [...seen[k]].sort((x, y) => x - y); const inRange = a[a.length - 1] < nf[k]; allIn = allIn && inRange;
  console.log('       bank 0x' + k.toString(16).padStart(2, '0') + ' -> ' + SETS[k].padEnd(13) + String(nf[k]).padStart(3)
    + ' frames; plane2 = ' + a[0] + '..' + a[a.length - 1] + ' (' + a.length + ' distinct)'
    + (a.length === nf[k] ? '  SATURATED (uses every frame)' : ''));
}
}
if (haveSets) chk('every plane-2 index is a valid frame of the bank tile set', allIn);
if (haveSets) chk("bank 0x04 uses exactly all 25 Mtns frames", !!seen[4] && seen[4].size === 25 && nf[4] === 25);
chk('bank 0x10 (Castle) never appears on disk', seen[16] === undefined);

// ---------- 2. plane 0 ----------
console.log('\n[2] plane 0 = per-tile flag bits');
let a4 = 0, t4 = 0, b2 = 0, b2tot = 0, n10 = 0, bad10 = 0;
for (const r of used) {
  const p0 = P(r, 0), p5 = P(r, 5);
  const cs = new Set(); let c10 = 0;
  for (let i = 0; i < PL; i++) {
    t4++; if (((p0[i] & 4) !== 0) === (p5[i] === 0)) a4++;
    if (p5[i] > 0 && p5[i] <= 16) cs.add(p5[i]);
    if (p0[i] & 0x10) c10++;
    if (p0[i] & 2) {
      b2tot++; const x = i % 64, y = (i / 64) | 0, c = p5[i]; let bd = false;
      for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
        const nx = x + dx, ny = y + dy; if (nx < 0 || ny < 0 || nx > 63 || ny > 63) continue;
        const ncy = p5[ny * 64 + nx]; if (ncy !== c && ncy !== 0 && c !== 0) { bd = true; break; }
      }
      if (bd) b2++;
    }
  }
  n10 += c10; if (c10 !== 4 * cs.size) bad10++;
}
chk('(plane0 & 0x04) <=> county == 0', a4 === t4, a4 + '/' + t4);
chk('every 0x02 tile is 4-adjacent to a different county (boundary)', b2 === b2tot, b2 + '/' + b2tot);
chk('every map has exactly 4 tiles with bit 0x10 per county', bad10 === 0, n10 + ' tiles total, ' + bad10 + ' maps disagree');
const cls = {};
for (const r of used) {
  const p0 = P(r, 0), p1 = P(r, 1), p2 = P(r, 2);
  for (let i = 0; i < PL; i++) {
    const k = '0x' + p0[i].toString(16).padStart(2, '0') + ' bank0x' + p1[i].toString(16).padStart(2, '0');
    const e = cls[k] = cls[k] || { n: 0, s: new Set() }; e.n++; e.s.add(p2[i]);
  }
}
const rng = s => {
  const a = [...s].sort((x, y) => x - y), o = []; let st = a[0], pv = a[0];
  for (let i = 1; i < a.length; i++) { if (a[i] === pv + 1) { pv = a[i]; continue; } o.push(st === pv ? '' + st : st + '-' + pv); st = pv = a[i]; }
  o.push(st === pv ? '' + st : st + '-' + pv); return o.join(',');
};
console.log('       plane0 value x bank -> plane2 index ranges (exact, disjoint):');
for (const k of Object.keys(cls).sort()) console.log('         ' + k.padEnd(18) + String(cls[k].n).padStart(7) + '  ' + rng(cls[k].s));

// ---------- 3. plane 3 ----------
console.log('\n[3] plane 3 = part index inside a multi-tile object');
function screenOrder(W, H) {
  const a = []; for (let dy = 0; dy < H; dy++) for (let dx = 0; dx < W; dx++) a.push([dx, dy]);
  a.sort((p, q) => (p[0] + p[1]) - (q[0] + q[1]) || p[0] - q[0]); return a;
}
const GROUPS = [{ bank: 4, base: 0, W: 2, H: 2 }, { bank: 4, base: 4, W: 2, H: 2 }, { bank: 4, base: 8, W: 2, H: 2 },
{ bank: 4, base: 12, W: 2, H: 2 }, { bank: 4, base: 16, W: 3, H: 3 }, { bank: 12, base: 0, W: 2, H: 2 }];
let gOK = 0, gTot = 0;
for (const r of used) {
  const p1 = P(r, 1), p2 = P(r, 2), p3 = P(r, 3);
  for (const g of GROUPS) {
    const o = screenOrder(g.W, g.H);
    for (let i = 0; i < PL; i++) {
      if (p1[i] !== g.bank) continue; const q = p2[i] - g.base;
      if (q < 0 || q >= g.W * g.H) continue; gTot++;
      const dx = o[q][0], dy = o[q][1]; if (p3[i] === dx + g.W * dy) gOK++;
    }
  }
}
chk('plane3 == dx + W*dy for every mtns 2x2/3x3 and town castle-site tile', gOK === gTot, gOK + '/' + gTot);
let cOK = 0, cTot = 0;
for (const r of used) {
  const p0 = P(r, 0), p1 = P(r, 1), p2 = P(r, 2), p3 = P(r, 3);
  for (let i = 0; i < PL; i++) {
    if (!(p0[i] & 0x40) || p1[i] !== 12 || p2[i] !== 0) continue;
    const x = i % 64, y = (i / 64) | 0; cTot++;
    let ok = x < 63 && y < 63;
    if (ok) for (const q of [[1, 0, 2, 1], [0, 1, 1, 2], [1, 1, 3, 3]]) {
      const j = (y + q[1]) * 64 + x + q[0];
      if (!(p0[j] & 0x40) || p1[j] !== 12 || p2[j] !== q[2] || p3[j] !== q[3]) { ok = false; break; }
    }
    if (ok) cOK++;
  }
}
chk('every castle (0x40) is a 2x2 of Town frames 0/2/1/3 with plane3 0/1/2/3', cOK === cTot, cOK + '/' + cTot);
let sOK = 0; const sPart = { 0: 0, 1: 0, 2: 0, 3: 0 };
for (const r of used) {
  const p0 = P(r, 0), p3 = P(r, 3);
  for (let i = 0; i < PL; i++) {
    if (!(p0[i] & 0x80)) continue; sPart[p3[i]] = (sPart[p3[i]] || 0) + 1;
    if (p3[i] !== 0) continue; const x = i % 64, y = (i / 64) | 0;
    let ok = x < 63 && y < 63;
    if (ok) for (const q of [[1, 0, 1], [0, 1, 2], [1, 1, 3]]) {
      const j = (y + q[1]) * 64 + x + q[0];
      if (!(p0[j] & 0x80) || p3[j] !== q[2]) { ok = false; break; }
    }
    if (ok) sOK++;
  }
}
const NCTY = cTot;   // one castle == one county; cTot counted them
chk('settlement parts 1,2,3 each occur once per county (' + NCTY + ')',
  sPart[1] === NCTY && sPart[2] === NCTY && sPart[3] === NCTY, JSON.stringify(sPart));
chk('every settlement anchor completes a 2x2 with plane3 0/1/2/3', sOK === NCTY, sOK + '/' + NCTY);

// ---------- 4. plane 4 ----------
console.log('\n[4] plane 4');
let p4other = 0; const p4c = {}, p4s = {};
for (const r of used) {
  const p0 = P(r, 0), p4 = P(r, 4);
  for (let i = 0; i < PL; i++) {
    if (!p4[i]) continue;
    if (p0[i] & 0x40) p4c[p4[i]] = (p4c[p4[i]] || 0) + 1;
    else if (p0[i] & 0x80) p4s[p4[i]] = (p4s[p4[i]] || 0) + 1;
    else p4other++;
  }
}
chk('plane4 is non-zero only where plane0 has bit 0x40 or 0x80', p4other === 0, p4other + ' others');
console.log('       on castle tiles     :', JSON.stringify(p4c));
console.log('       on settlement tiles :', JSON.stringify(p4s));
let qTop = 0, qTopNZ = 0, qR = 0, qRZ = 0;
for (const r of used) {
  const p0 = P(r, 0), p3 = P(r, 3), p4 = P(r, 4);
  for (let i = 0; i < PL; i++) {
    if (!(p0[i] & 0x40)) continue;
    if (p3[i] === 0) { qTop++; if (p4[i]) qTopNZ++; }
    if (p3[i] === 1) { qR++; if (!p4[i]) qRZ++; }
  }
}
chk('castle top quadrant (plane3=0) always has plane4 == 0', qTopNZ === 0, qTop + ' tiles');
chk('castle right quadrant (plane3=1) always has plane4 != 0', qRZ === 0, qR + ' tiles');
const h = {};
for (const r of used) {
  const p0 = P(r, 0), p4 = P(r, 4); const v = [];
  for (let i = 0; i < PL; i++) if ((p0[i] & 0x80) && p4[i]) v.push(p4[i]);
  v.sort(); h[v.join(',')] = (h[v.join(',')] || 0) + 1;
}
console.log('       settlement plane4 multiset per map:', JSON.stringify(h));

// ---------- 5. lattice ----------
console.log('\n[5] 65x129 lattice');
const cov = new Set();
for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) cov.add((x + y + 1) * LW + ((x - y + 64) >> 1));
chk('row=x+y+1, col=(x-y+64)>>1 is injective and lands inside 65x129', cov.size === 4096, cov.size + ' distinct cells of ' + LW * LH);
let rmin = 1e9, rmax = -1, cmin = 1e9, cmax = -1;
for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) {
  const rr = x + y + 1, cc = (x - y + 64) >> 1;
  rmin = Math.min(rmin, rr); rmax = Math.max(rmax, rr); cmin = Math.min(cmin, cc); cmax = Math.max(cmax, cc);
}
console.log('       covered rows ' + rmin + '..' + rmax + ' of 0..128, cols ' + cmin + '..' + cmax + ' of 0..64');
const alpha = new Set(); for (const r of used) for (const v of T(r)) alpha.add(v);
chk('lattice alphabet is {0x06, 0x16} over used slots', alpha.size === 2 && alpha.has(6) && alpha.has(0x16), [...alpha].join(','));

console.log('\n' + pass.length + ' checks passed, ' + fail.length + ' failed' + (fail.length ? ': ' + fail.join('; ') : ''));
