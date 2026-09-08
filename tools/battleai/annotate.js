#!/usr/bin/env node
// Rewrite Ghidra DAT_ symbols that fall inside the three big battle arrays into
// "<array>_<offset>" tokens, so decompiled order handlers can be read as field
// accesses instead of absolute addresses. Purely cosmetic; no interpretation.
//
//   node tools/battleai/annotate.js out/handlers.c > out/handlers.ann.c
'use strict';
const fs = require('fs');

// Known field names, from docs/battle.md 1 and 2 (debug-panel labels where [V]).
const UNIT = {
  0x00: 'owner', 0x01: 'human', 0x02: 'figCount', 0x03: 'side', 0x04: 'firstFig',
  0x06: 'lastFig', 0x08: 'cat', 0x0f: 'firing', 0x14: 'reTarg', 0x1a: 'orders',
  0x1e: 'x', 0x20: 'y', 0x22: 'tgx', 0x24: 'tgy', 0x2c: 'ordTarget', 0x30: 'tgCell',
};
const MAN = {
  0x12: 'type', 0x13: 'human', 0x18: 'dirc', 0x1c: 'cell', 0x20: 'x', 0x22: 'y',
  0x24: 'tgx', 0x26: 'tgy', 0x2c: 'owner', 0x31: 'state', 0x172: 'armour',
  0x174: 'target', 0x175: 'targeted', 0x178: 'unit', 0x17a: 'side', 0x17e: 'oppo',
  0x197: 'band', 0x19a: 'hits', 0x19c: 'melee', 0x1a0: 'men',
  0x164: 'onRoute', 0x165: 'holdIt', 0x166: 'routed', 0x16a: 'wclass', 0x170: 'range',
  0x176: 'barred', 0x189: 'moveDelay', 0x18b: 'recovery', 0x194: 'isEngine',
  0x2f: 'dlyState', 0x30: 'delay', 0x32: 'walking', 0x36: 'pathLen', 0x1a4: 'movStraff',
};
const ARRAYS = [
  { base: 0x00566520, size: 0x34,  tag: 'U', names: UNIT },
  { base: 0x00554480, size: 0x1b0, tag: 'M', names: MAN  },
  { base: 0x005440e0, size: 0xc800,  tag: 'CELL', names: {} },
];

function label(addr) {
  for (const a of ARRAYS) {
    if (addr >= a.base && addr < a.base + a.size) {
      const off = addr - a.base;
      const nm = a.names[off];
      return `${a.tag}_${off.toString(16).padStart(2, '0')}${nm ? '_' + nm : ''}`;
    }
  }
  return null;
}

const src = fs.readFileSync(process.argv[2], 'utf8');
process.stdout.write(src.replace(/\b(?:DAT|UNK|PTR_DAT)_00([0-9a-f]{6})\b/g, (m, h) => {
  const l = label(parseInt(h, 16));
  return l === null ? m : l;
}).replace(/\bg_battleUnits\b/g, 'U_00_owner').replace(/\bg_battleMen\b/g, 'M_00')
  .replace(/\bg_battlefield\b/g, 'CELL_00'));
