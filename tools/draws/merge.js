#!/usr/bin/env node
// Fold the per-agent audit files into the one inventory.
//
//   node tools/draws/merge.js tools/audit/draws-*.json
//
// # Why a script and not a hand merge
//
// `docs/agents.md`'s longest section is about nine merge defects that were all
// the same mistake: **a file that looks like data is usually a claim, and claims
// cannot be merged positionally.** The one that nearly cost the most was a
// *counting* file — `docs/arms.json`, 42 records across 23 addresses — which a
// keyed merge would have silently deduplicated down to 23, leaving a smaller
// file that was internally consistent, valid JSON, and wrong about the only
// thing it existed to say.
//
// So this merges **by `screen`**, refuses a duplicate rather than picking a
// side, and prints the count before and after. `docs/agents.md`: *count before
// and after any edit to a shared JSON file, and say both numbers.*
//
// # The flow, and the file that must not survive the merge
//
// An agent auditing new screens writes `tools/draws/pending-<its own name>.json`,
// this folds it into `screens.json`, and **the pending file is then deleted.**
// That last step is the important one: after the merge, every field of the
// pending file is in the inventory, so keeping both leaves two copies of a
// counting claim maintained by nobody. `docs/agents.md` is explicit that two
// artefacts which must agree are worth having only when *different work*
// maintains each; two copies of the same fold are the version that lies.
//
// Six per-agent files were merged and deleted this way on the first pass, after
// checking field by field that nothing was lost.
//
// # A field this merge deliberately keeps, which looks like a duplicate
//
// A record's `ours` is the **auditor's own count**, made by hand while reading
// the screen. `screendraws.js` ignores it and recomputes ours from the source
// every run. They are not redundant: they are two enumerations from different
// directions, and where they disagree the disagreement is the finding — on the
// first pass they agreed on 7 screens of 24 and were within ±2 on fifteen more,
// which is how the tool's two scanner bugs were found.
'use strict';
const fs = require('fs');
const path = require('path');

const INV = path.join(__dirname, 'screens.json');

// Which module each screen id is drawn by in our tree, and which functions
// paint it in the original. **This is the part a script cannot derive**: the
// agent that read the painter knows its helpers, and `also_drawn_by` is prose.
// So the mapping is written here, checked by `screendraws.js --check` (every
// name must resolve in the corpus; every module must exist), and wrong entries
// are loud rather than silent.
// `roots` is *which functions paint this screen* — the painter plus the helpers
// that are only ever reached from it, including the ones `Screen_DrawWidgets`
// calls every frame and the one the **application frame loop** calls that no
// dispatch table mentions at all (`FUN_0040C725`, the open drop-down).
//
// `module` is which of our screens answers for it, and it is deliberately
// `null` where nothing does — a screen we have not built scores `ours: 0` and
// must not borrow a neighbour's module, which would credit it with somebody
// else's drawing. `0x2F` is the sharp case: its listing lives in `ratings.rs`
// because that is where a reader will look for it, and it still gets `null`,
// because every mark in that file belongs to `0x2E`.
const KNOWN = {
  '0x0A': { module: 'armoury.rs', roots: ['Screen_Armoury', 'Armoury_DrawTorches', 'Armoury_DrawWalker', 'Armoury_RestoreWalkerStrip'] },
  '0x0D': { module: 'armoury.rs', roots: ['Armoury_LoadScreen', 'Armoury_DrawRacks', 'Armoury_DrawTorches', 'Armoury_DrawWalker', 'Armoury_RestoreWalkerStrip', 'FUN_00418e2d'] },
  '0x17': { module: 'army.rs', roots: ['Screen_RaiseArmy'] },
  '0x11': { module: 'divide.rs', roots: ['Screen_ArmyDivision', 'Screen_SplitArmyRows'] },
  '0x08': { module: 'merchant.rs', roots: ['Screen_Merchant', 'Merchant_HoverPlaque'] },
  '0x0C': { module: 'merchant.rs', roots: ['Screen_TradeGoods', 'Trade_DrawPanel'] },
  '0x1B': { module: 'castle.rs', roots: ['Screen_CastleBuild', 'Screen_CastleBuildPanel'] },
  '0x31': { module: 'options.rs', roots: ['Screen_HelpOptions'] },
  '0x39': { module: 'options.rs', roots: ['Screen_AdvancedOptions'] },
  '0x42': { module: 'options.rs', roots: ['Screen_SoundOptions'] },
  '0x43': { module: 'options.rs', roots: ['Screen_DisplayOptions'] },
  '0x1E': { module: null, roots: ['Screen_ConfirmBox'] },
  '0x21': { module: null, roots: ['Screen_SliderBox'] },
  '0x35/0x36': { module: 'saveload.rs', roots: ['Screen_SaveLoad', 'SaveLoad_DrawStatus'] },
  '0x1C': { module: 'conquest.rs', roots: ['Screen_DrawConquest'] },
  '0x20': { module: null, roots: ['Screen_GreatestNoble'] },
  '0x32': { module: 'menubar.rs', roots: ['Menu_RestoreBackdrop', 'FUN_0040c725'] },
  '0x44': { module: null, roots: ['FUN_00425a6a'] },
  '0x45': { module: null, roots: ['Screen_LordsOfMagicAd'] },
  '0x12': { module: 'battle.rs', roots: ['Screen_BattlePrompt'] },
  '0x13': { module: 'battle.rs', roots: ['Screen_BattleResult'] },
  '0x1D': { module: 'siege.rs', roots: ['Screen_SiegePrep'] },
  '0x2B': { module: null, roots: ['Screen_BattleOutcome'] },
  '0x2F': { module: null, roots: ['Screen_BattleMasterRank'] },
  '0x14': { module: 'county.rs', roots: ['Panel_Population'] },
  '0x15': { module: 'county.rs', roots: ['Panel_Tax'] },
  '0x16': { module: 'county.rs', roots: ['Panel_Happiness'] },
  '0x19': { module: 'county.rs', roots: ['Panel_Ration', 'Panel_RationSlider'] },
  '0x0F': { module: 'job.rs', roots: ['Panel_JobDetail', 'FUN_00413526'] },
  '0x02': { module: 'village.rs', roots: ['Village_Draw', 'Village_Animate'] },
  '0x1A': { module: 'diplomacy.rs', roots: ['Screen_DiploDialog', 'FUN_00417bd3'] },
  'ours/menu': { module: 'menu.rs', roots: [], painter: null },
};

function load(file) {
  const j = JSON.parse(fs.readFileSync(file, 'utf8'));
  if (!Array.isArray(j)) throw new Error(`${file} is not an array`);
  return j;
}

function main() {
  const files = process.argv.slice(2).filter(a => !a.startsWith('--'));
  if (!files.length) {
    console.error('usage: node tools/draws/merge.js <per-agent json> ...');
    process.exit(2);
  }
  const inv = JSON.parse(fs.readFileSync(INV, 'utf8'));
  const before = inv.length;
  const by = new Map(inv.map(r => [r.screen, r]));

  let added = 0, updated = 0;
  const clashes = [];
  for (const file of files) {
    for (const rec of load(file)) {
      if (!rec.screen) { clashes.push(`${file}: a record with no "screen"`); continue; }
      const existing = by.get(rec.screen);
      if (!existing) {
        by.set(rec.screen, { ...rec, _from: path.basename(file) });
        added++;
        continue;
      }
      if (existing._from && existing._from !== path.basename(file)) {
        // Two agents claiming one screen is a **content** collision, and
        // `docs/agents.md` is explicit that no tool resolves one. Refuse.
        clashes.push(
          `${rec.screen} is claimed by both ${existing._from} and ${path.basename(file)}`);
        continue;
      }
      // The pilot's six are already here and are the authority on themselves;
      // an agent record only fills fields the pilot's does not have.
      for (const [k, v] of Object.entries(rec)) {
        if (existing[k] === undefined) existing[k] = v;
      }
      updated++;
    }
  }

  if (clashes.length) {
    console.error('draws/merge: refusing to merge —\n  ' + clashes.join('\n  '));
    process.exit(1);
  }

  const out = [...by.values()];
  for (const r of out) {
    if (KNOWN[r.screen]) Object.assign(r, KNOWN[r.screen]);
    delete r._from;
  }
  fs.writeFileSync(INV, JSON.stringify(out, null, 2) + '\n');
  console.log(`draws/merge: ${before} records before, ${out.length} after ` +
    `(${added} added, ${updated} filled in from ${files.length} files).`);
  console.log('  now run: node tools/draws/screendraws.js --write   # recount from the binary');
  console.log('           node tools/draws/screendraws.js --check   # and prove it');
}

main();
