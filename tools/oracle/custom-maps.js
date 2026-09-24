// Writes tools/oracle/scenarios/custom-map-NN.jsonl: a custom game on each of the 44 maps
// the page-7 list offers (FUN_0046A101 0x0046A101 counts the MAPnn.PL8 on disk into DAT_00554018).
// node tools/oracle/custom-maps.js [count]
'use strict';
const fs = require('fs');
const path = require('path');

const COUNT = Number(process.argv[2] || 44);
const dir = path.join(__dirname, 'scenarios');
const j = (o) => JSON.stringify(o);

function scenario(m) {
  const down = Math.min(m, COUNT - 5), row = m - down;
  const s = [
    `// A custom game on list entry ${m} of page 7, through turn one and the AI lords' first moves (custom-maps.js).`,
    j({ op: 'films', mode: 'skip' }),
    j({ op: 'wait-page', page: 1 }), j({ op: 'wait-idle', frames: 5 }),
    '// Page 1 item 0 at y 0x5B (screens-county.md 1.2): page 2.',
    j({ op: 'click', x: 320, y: 103 }),
    j({ op: 'wait-page', page: 2 }), j({ op: 'wait-idle', frames: 5 }),
    '// Page 2 item 3, "Custom game": FUN_00432CC8 hotspot 3, page 4.',
    j({ op: 'click', x: 320, y: 211 }),
    j({ op: 'wait-page', page: 4 }), j({ op: 'advance', frames: 20 }),
    '// Page 4 "Continue", FUN_00433155 hotspot 2: page 7.',
    j({ op: 'click', x: 432, y: 227 }),
    j({ op: 'wait-page', page: 7 }), j({ op: 'advance', frames: 20 }),
  ];
  if (down) s.push(`// FUN_004337B4 hotspot 1, the list's down arrow: rect 603..624 x 209..229 (.data 0x4DCD90), ${down} times.`);
  for (let i = 0; i < down; i++) s.push(j({ op: 'click', x: 613, y: 219 }));
  if (row) {
    s.push(`// FUN_00433905 hotspot ${row}: list row ${row}, rect 496..601 x ${141 + 16 * row}.. (.data 0x4DCDA8 + 24 n).`);
    s.push(j({ op: 'click', x: 548, y: 149 + 16 * row }));
  }
  s.push(j({ op: 'advance', frames: 10 }), j({ op: 'peek', addr: '0x0053F034' }), j({ op: 'anchor', label: 'custom-page' }), j({ op: 'snap' }),
    '// "Start", CUSTOM_BUTTONS 1 at x 0xF3, caption y 0xC6.',
    j({ op: 'click', x: 281, y: 206 }),
    j({ op: 'wait-screen-not', id: '0x1F' }), j({ op: 'advance', frames: 300 }),
    j({ op: 'dismiss' }), j({ op: 'anchor', label: 'map' }), j({ op: 'record', frames: 2 }), j({ op: 'snap' }),
    '// End turn strip, Misc_cty frame 59 at (478, 460) 162 x 20 (screens.md 4.3)',
    j({ op: 'click', x: 559, y: 470 }), j({ op: 'advance', frames: 30 }), j({ op: 'dismiss' }),
    j({ op: 'anchor', label: 'end-turn-clicked' }), j({ op: 'snap' }),
    j({ op: 'advance', frames: 2000 }), j({ op: 'dismiss' }),
    j({ op: 'anchor', label: 'turn-2' }), j({ op: 'record', frames: 2 }), j({ op: 'snap' }));
  return s.join('\n') + '\n';
}

for (let m = 0; m < COUNT; m++) {
  fs.writeFileSync(path.join(dir, `custom-map-${String(m).padStart(2, '0')}.jsonl`), scenario(m));
}
console.log(`${COUNT} scenarios`);
