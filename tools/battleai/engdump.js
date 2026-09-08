#!/usr/bin/env node
// Dump every string of an L2.eng-format text file as "group index  text".
// Used here only as an outside check on names read out of the binary.
//   node tools/battleai/engdump.js "F:/games/Lords of the Realm II/L2.eng"
'use strict';
const fs = require('fs');
const b = fs.readFileSync(process.argv[2]);
const slots = (b.readUInt32LE(12) - 8) / 4;
const off = g => b.readUInt32LE(8 + g * 4) & 0xffffff;
for (let g = 1; g <= slots; g++) {
  const start = off(g), end = g < slots ? off(g + 1) : b.length;
  if (!(end > start)) continue;
  let i = start, idx = 0;
  while (i < end) {
    let j = i; while (j < end && b[j] !== 0) j++;
    const s = b.toString('latin1', i, j);
    if (s.length) console.log(`${g}\t${idx}\t${s}`);
    idx++; i = j + 1;
  }
}
