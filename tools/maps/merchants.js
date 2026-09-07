// Reproduce the game's plane-4 castle table: the six merchant trade routes.
//
// Mirrors Lords2.exe exactly:
//   Map_LoadPlanes            (0x00467770) scans y-major, x-inner, and for every tile with
//                             plane0 & 0x40 and plane4 != 0 calls
//   Merchant_RouteAppend      (0x00429153) row[plane4-1][first free of 16] = plane5
//   Merchant_PickStartCounties(0x004291b3) chooses one distinct start county per row
//   Merchant_SpawnAll         (0x00427ed0) spawns merchants until a start county is 0
//
// Usage: node tools/maps/merchants.js ["F:/games/Lords of the Realm II"] [slot]
const fs = require('fs');
const DIR = (process.argv[2] || 'F:/games/Lords of the Realm II').replace(/[\/]+$/, '') + '/';
const REC = 32961, PL = 4096;
const buf = fs.readFileSync(DIR + 'L2_maps.dat');
const NSLOT = buf.length / REC;
const plane = (s, p) => buf.subarray(s * REC + p * PL, s * REC + (p + 1) * PL);

// L2.eng group 101 = slot names, group 5 = merchant names
let slotNames = [], merchantNames = [];
try {
  const e = fs.readFileSync(DIR + 'L2.eng');
  const off = g => e[8 + g * 4] | (e[9 + g * 4] << 8) | (e[10 + g * 4] << 16);
  const grp = g => { const a = off(g), b = off(g + 1), o = []; let i = a;
    while (i < b) { let j = i; while (j < b && e[j] !== 0) j++; o.push(e.toString('latin1', i, j)); i = j + 1; } return o; };
  slotNames = grp(101); merchantNames = grp(5);
} catch (x) { /* no L2.eng */ }

function analyse(s) {
  const p0 = plane(s, 0), p3 = plane(s, 3), p4 = plane(s, 4), p5 = plane(s, 5);
  const rows = [[], [], [], [], [], []];       // the 6x16 table at 0x00567970
  const overflow = [0, 0, 0, 0, 0, 0];
  let counties = 0;
  for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) {
    const i = y * 64 + x;
    if (p5[i] < 0x11 && p5[i] > counties) counties = p5[i];   // DAT_0056d5dc
    if (p4[i] === 0) continue;
    if (p0[i] & 0x40) { const r = p4[i] - 1; if (rows[r].length < 16) rows[r].push(p5[i]); else overflow[r]++; }
  }
  // Merchant_PickStartCounties (0x004291b3), byte for byte
  const cell = (r, k) => (k < rows[r].length ? rows[r][k] : 0);
  const chosen = [0, 0, 0, 0, 0, 0];
  const dup = v => chosen.some(c => c === v);
  for (let r = 0; r < 6; r++) {
    let tries = 0, cur = 0, val = cell(r, 0);
    while (dup(val) && ++tries < 6) {
      val = cell(r, 2 + cur); cur += 2;
      if (val === 0) cur = 1;
    }
    chosen[r] = val;
  }
  // Merchant_SpawnAll (0x00427ed0): stops at the first zero start county
  let merchants = 0;
  for (let r = 0; r < 6; r++) { if (chosen[r] === 0) break; merchants++; }
  return { rows, overflow, counties, chosen, merchants, p0, p3, p4, p5 };
}

const used = [];
for (let s = 0; s < NSLOT; s++) {
  let nc = false;
  for (let p = 0; p < 6 && !nc; p++) { const d = plane(s, p); for (let k = 1; k < PL; k++) if (d[k] !== d[0]) { nc = true; break; } }
  if (nc) used.push(s);
}

const one = process.argv[3] !== undefined ? +process.argv[3] : null;
const list = one === null ? used : [one];

let anyDup = 0, anyOver = 0, anyOob = 0, badStart = 0, notOnRoute = 0, mapsWithSix = 0;
let quadDup = 0, quadTotal = 0, p4Total = 0, unusedRoute = 0;
const memberHist = new Map();
const sizeHist = new Map();
console.log('slot  name            cty  merch  route sizes           start counties');
for (const s of list) {
  const a = analyse(s);
  const sizes = a.rows.map(r => r.length);
  // every county on at least one route?
  const onRoute = new Set(); a.rows.forEach(r => r.forEach(c => onRoute.add(c)));
  for (let c = 1; c <= a.counties; c++) if (!onRoute.has(c)) notOnRoute++;
  // duplicates within a route
  a.rows.forEach(r => { if (new Set(r).size !== r.length) anyDup++; });
  // entries out of range (the 0x004280e9 guard)
  a.rows.forEach(r => r.forEach(c => { if (c === 0 || c > a.counties) anyOob++; }));
  a.overflow.forEach(o => { if (o) anyOver++; });
  if (new Set(a.chosen.filter(v => v)).size !== a.chosen.filter(v => v).length) badStart++;
  if (a.merchants === 6) mapsWithSix++;
  a.rows.forEach(r => p4Total += r.length);
  for (let r = a.merchants; r < 6; r++) if (a.rows[r].length) unusedRoute++;
  // the three non-zero quadrants of one castle must carry distinct plane-4 values
  for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) {
    const i = y * 64 + x;
    if ((a.p0[i] & 0x40) && a.p3[i] === 0) {           // castle anchor
      const q = [0, 1, 2, 3].map(k => a.p4[i + (k & 1) + 64 * (k >> 1)]);
      quadTotal++;
      const nz = q.filter(v => v);
      if (new Set(nz).size !== nz.length) quadDup++;
    }
  }
  const per = new Map();
  a.rows.forEach(r => r.forEach(c => per.set(c, (per.get(c) || 0) + 1)));
  for (const v of per.values()) memberHist.set(v, (memberHist.get(v) || 0) + 1);
  sizeHist.set(a.merchants, (sizeHist.get(a.merchants) || 0) + 1);
  console.log(String(s).padStart(4) + '  ' + (slotNames[s] || '').padEnd(15) +
    String(a.counties).padStart(3) + String(a.merchants).padStart(6) + '   ' +
    sizes.join(',').padEnd(20) + '  ' + a.chosen.join(',') +
    (one !== null ? '' : ''));
  if (one !== null) {
    a.rows.forEach((r, i) => console.log('   route ' + (i + 1) + ' (' +
      (merchantNames[i] || '?') + '): ' + (r.length ? r.join(' -> ') : '(empty)')));
  }
}
if (one === null) {
  console.log('\nmaps: ' + list.length);
  console.log('counties missing from every route : ' + notOnRoute);
  console.log('routes with a duplicate county    : ' + anyDup);
  console.log('route entries 0 or > county count : ' + anyOob);
  console.log('routes that overflowed 16 entries : ' + anyOver);
  console.log('maps where two merchants share a start county: ' + badStart);
  console.log('castles whose non-zero quadrants repeat a value: ' + quadDup + ' / ' + quadTotal);
  console.log('total route entries (= plane-4 castle census): ' + p4Total);
  console.log('non-empty routes no merchant ever walks: ' + unusedRoute);
  console.log('counties by number of routes they sit on: ' + [...memberHist.entries()].sort((a,b)=>a[0]-b[0]).map(([k,v])=>k+' route(s): '+v).join(', '));
  console.log('merchant count histogram: ' + [...sizeHist.entries()].sort((a, b) => a[0] - b[0])
    .map(([k, v]) => k + ' merchants: ' + v + ' maps').join(', '));
}
