// Locate a string in L2.eng and report its (group, index).
// usage: node engfind.js <L2.eng> <substring> [...]
const fs = require('fs');
const buf = fs.readFileSync(process.argv[2]);
const off = g => buf[8+g*4] | (buf[9+g*4]<<8) | (buf[10+g*4]<<16);
const n = (off(1) - 8) / 4;
const groups = [];
for (let g = 1; g < n; g++) {
  const start = off(g);
  const end = (g + 1 < n) ? off(g+1) : buf.length;
  const strs = [];
  let p = start;
  while (p < end) {
    let q = p; while (q < end && buf[q] !== 0) q++;
    strs.push(buf.slice(p, q).toString('latin1'));
    p = q + 1;
  }
  groups.push({ g, strs });
}
for (const needle of process.argv.slice(3)) {
  for (const {g, strs} of groups)
    strs.forEach((s, i) => { if (s.includes(needle)) console.log(`group ${g} index ${i}: ${JSON.stringify(s)}`); });
}
