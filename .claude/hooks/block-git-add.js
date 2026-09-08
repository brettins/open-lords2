#!/usr/bin/env node
// Block `git add -A`, `git add --all` and `git add .`.
//
// This rule has been written down three times and broken three times:
//
//   C9  - `git add -A` swept fifteen rendered PNGs of game data into an
//         unrelated commit, straight past the first rule of the project.
//   C18 - the replacement rule ("stage explicit paths") was necessary and not
//         sufficient: `git commit` commits the whole index, so one agent still
//         took another's staged renames.
//   and again after that, when the same sweep put 35 MB of build output into
//   the history via a crate-local `target/` the gitignore never matched.
//
// Documentation did not stop it, so this does. A rule that depends on everyone
// remembering it is a rule that fails the first time somebody is in a hurry.
//
// stdin is the tool call as JSON; `.tool_input.command` is the command line.
// Exit 2 blocks the call and sends stderr back to the model, so it learns why
// and can retry with explicit paths.

'use strict';

let input = '';
process.stdin.on('data', (chunk) => (input += chunk));
process.stdin.on('end', () => {
  let command = '';
  try {
    // Strip a UTF-8 BOM. PowerShell prepends one when piping to a native
    // command, and JSON.parse rejects it - which silently turned this whole
    // guard off the first time it was tested. A guard that fails open and says
    // nothing is the same bug as a test that cannot fail (C12).
    command = (JSON.parse(input.replace(/^﻿/, '')).tool_input || {}).command || '';
  } catch (e) {
    // Still fail open - a broken hook must not block real work - but say so,
    // because a silent one is indistinguishable from a working one.
    process.stderr.write(`block-git-add: could not read the tool call (${e.message}); allowing.\n`);
    process.exit(0);
  }

  // `git add` followed by -A, --all, or a bare dot. The trailing guard stops
  // `git add -Almighty` or `git add ./crates/l2-sim` matching, which are fine.
  const sweeping = /\bgit\s+add\b[^\n;|&]*?(\s(-A|--all|\.)(\s|$))/;

  if (sweeping.test(command)) {
    process.stderr.write(
      'Blocked: `git add -A`, `git add --all` and `git add .` are not allowed in this repo.\n' +
        '\n' +
        'This has gone wrong three times here (docs/decisions.md C9, C18): it has committed\n' +
        "the game's copyrighted art, another agent's half-finished work, and 35 MB of build\n" +
        'output, each time under an unrelated commit message.\n' +
        '\n' +
        'Stage explicit paths instead, and name them on the *commit* as well, because\n' +
        '`git commit` takes the whole index and not just what you added:\n' +
        '\n' +
        '    git commit -F msg.txt -- crates/l2-sim docs/battle.md\n' +
        '\n' +
        'Three cases, per docs/agents.md:\n' +
        '  - editing files          ->  git commit -F msg -- <paths>\n' +
        '  - a brand-new file       ->  git add <path>, then the -- form\n' +
        '  - a removal (git rm)     ->  stage it, check `git status`, plain `git commit`\n' +
        '                               (only safe when no other agent is mid-edit)\n'
    );
    process.exit(2);
  }

  process.exit(0);
});
