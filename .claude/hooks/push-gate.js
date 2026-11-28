#!/usr/bin/env node
// Block `git push` until the commits it sends carry the reviews docs/agents.md
// *Before a push* asks for:
//
//   behaviour  crates/*/src, non-test .rs   an oracle-check record in
//                                           docs/oracle-checks/*.json
//   code       any source file              a code-review marker from
//                                           tools/review/mark.js code
//
// A record or marker at commit R covers every commit up to R, and after a
// rebase every commit whose patch-id matches one of them. A commit may instead carry the trailer
// `Oracle-check: skip <reason>` or `Code-review: skip <reason>`, which stays in
// the public history. Exit 2 blocks the call and sends stderr to the model.

'use strict';

const { execFileSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const BEHAVIOUR = (f) =>
  /^crates\/[^/]+\/src\/.+\.rs$/.test(f) &&
  !/(^|\/)tests?(\/|\.rs$)|_tests?\.rs$|tests_part|test_support/.test(f);
const CODE = (f) =>
  /\.(rs|js|mjs|cjs|ts|ps1|py|sh|toml|ya?ml)$/.test(f) && !f.startsWith('docs/');

function git(cwd, ...args) {
  return execFileSync('git', args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
}
function tryGit(cwd, ...args) {
  try { return git(cwd, ...args); } catch { return null; }
}
const lines = (s) => (s ? s.split('\n').filter(Boolean) : []);

const unquote = (s) => s.replace(/^["']|["']$/g, '');

function repoDir(command, at) {
  const cds = [...command.slice(0, at).matchAll(/(?:^|&&|;)\s*cd\s+("[^"]+"|'[^']+'|\S+)/g)];
  const dir = cds.length ? unquote(cds[cds.length - 1][1]) : process.env.CLAUDE_PROJECT_DIR || process.cwd();
  return tryGit(dir, 'rev-parse', '--show-toplevel');
}

// `git push [remote] [src[:dst]]`: the tip sent and the remote ref it lands on.
function range(repo, args) {
  const words = args.trim().split(/\s+/).filter((w) => w && !w.startsWith('-'));
  const [remote, spec] = words;
  let tip = 'HEAD';
  let up = null;
  if (remote && spec) {
    const [src, dst] = spec.replace(/^\+/, '').split(':');
    if (src) tip = src;
    const name = (dst || src || '').replace(/^refs\/heads\//, '');
    up = tryGit(repo, 'rev-parse', '--verify', '-q', `refs/remotes/${remote}/${name}`);
  } else {
    up = tryGit(repo, 'rev-parse', '--verify', '-q', '@{u}');
  }
  up = up || tryGit(repo, 'rev-parse', '--verify', '-q', 'origin/main');
  tip = tryGit(repo, 'rev-parse', '--verify', '-q', `${tip}^{commit}`);
  if (!up || !tip) return null;
  const base = tryGit(repo, 'merge-base', tip, up);
  const commits = lines(base
    ? git(repo, 'rev-list', '--reverse', `${base}..${tip}`)
    : git(repo, 'rev-list', '--reverse', tip, '--not', '--remotes'));
  return { base, tip, commits };
}

const touched = (repo, c) => lines(git(repo, 'diff-tree', '--root', '--no-commit-id', '--name-only', '-r', '-m', '--first-parent', c));

function patchId(repo, c) {
  const diff = execFileSync('git', ['show', '--format=', c], { cwd: repo, encoding: 'utf8', maxBuffer: 1 << 28 });
  const out = execFileSync('git', ['patch-id', '--stable'], { cwd: repo, input: diff, encoding: 'utf8' });
  return out.split(' ')[0] || null;
}
const trailer = (repo, c, key) =>
  new RegExp(`^${key}:\\s*skip\\s+\\S`, 'mi').test(git(repo, 'log', '-1', '--format=%B', c));
const isAncestor = (repo, a, b) => tryGit(repo, 'merge-base', '--is-ancestor', a, b) !== null;

// A review at R counts if R was pushed with these commits (ancestry), or if R
// was rebased away and its own commits' patch-ids survive.
function uncovered(repo, r, reviews, needing) {
  const live = reviews.filter(Boolean);
  const pushed = live.filter((x) => isAncestor(repo, x, r.tip) && !(r.base && isAncestor(repo, x, r.base)));
  const left = needing.filter((c) => !pushed.some((x) => isAncestor(repo, c, x)));
  if (left.length === 0) return left;
  const patches = new Set();
  for (const x of live.filter((y) => !isAncestor(repo, y, r.tip))) {
    const mb = tryGit(repo, 'merge-base', x, r.tip);
    for (const c of lines(tryGit(repo, 'rev-list', mb ? `${mb}..${x}` : x) || '').slice(0, 200)) patches.add(patchId(repo, c));
  }
  return patches.size ? left.filter((c) => !patches.has(patchId(repo, c))) : left;
}

// Committed records only: a record the push does not carry does not count.
function oracleRecords(repo, tip) {
  return lines(tryGit(repo, 'ls-tree', '--name-only', tip, 'docs/oracle-checks/') || '')
    .filter((f) => f.endsWith('.json'))
    .map((f) => {
      try { return tryGit(repo, 'rev-parse', '--verify', '-q', JSON.parse(git(repo, 'show', `${tip}:${f}`)).reviewed + '^{commit}'); }
      catch { return null; }
    });
}

function codeMarkers(repo) {
  const dir = path.join(path.resolve(repo, git(repo, 'rev-parse', '--git-common-dir')), 'l2-reviews');
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir).map((f) => /^code-([0-9a-f]{40})\.json$/.exec(f)).filter(Boolean).map((m) => m[1]);
}

function gate(repo, r, kind, needs, reviews, key) {
  const needing = r.commits.filter((c) => touched(repo, c).some(needs) && !trailer(repo, c, key));
  if (needing.length === 0) return [];
  return uncovered(repo, r, reviews, needing).map((c) => `  ${c.slice(0, 9)} ${git(repo, 'log', '-1', '--format=%s', c)}  (needs ${kind})`);
}

function main(command) {
  const push = /(?:^|[;&|(\n])\s*(?:\w+=\S*\s+)*git\s+(?:-C\s+("[^"]+"|'[^']+'|\S+)\s+)?push\b[^;&|\n]*/.exec(command);
  if (!push || /--dry-run|\s-n(\s|$)|--delete|\s--tags\b/.test(push[0])) return 0;
  const repo = push[1] ? tryGit(unquote(push[1]), 'rev-parse', '--show-toplevel') : repoDir(command, push.index);
  if (!repo) return 0;
  const r = range(repo, push[0].replace(/^[\s\S]*?\bpush\b/, ''));
  if (!r || r.commits.length === 0) return 0;
  const missing = [
    ...gate(repo, r, 'an oracle check', BEHAVIOUR, oracleRecords(repo, r.tip), 'Oracle-check'),
    ...gate(repo, r, 'a code review', CODE, codeMarkers(repo), 'Code-review'),
  ];
  if (missing.length === 0) return 0;
  process.stderr.write(
    'Blocked: this push sends commits nobody has reviewed (docs/agents.md *Before a push*).\n\n' +
      missing.join('\n') + '\n\n' +
      'A rebase keeps reviews whose patch is unchanged; a commit that changed needs reviewing again.\n' +
      'Oracle check (game behaviour): run the oracle-check agent on a branch at the tip with base ' +
      `${r.base ? r.base.slice(0, 9) : "none (new history)"}; it writes docs/oracle-checks/<branch>.json. Fix or file each mismatch, commit the record.\n` +
      'Code review: run /code-review over the same range, fix or file what it finds, then\n' +
      '    node tools/review/mark.js code --findings <n> --summary "<what you fixed or filed>"\n' +
      'A commit that needs neither (a pure move, a rename, a revert) says so in its message:\n' +
      '    Oracle-check: skip <reason>      Code-review: skip <reason>\n'
  );
  return 2;
}

let input = '';
process.stdin.on('data', (c) => (input += c));
process.stdin.on('end', () => {
  let command = '';
  try {
    command = (JSON.parse(input.replace(/^﻿/, '')).tool_input || {}).command || '';
  } catch (e) {
    process.stderr.write(`push-gate: could not read the tool call (${e.message}); allowing.\n`);
    process.exit(0);
  }
  try {
    process.exit(main(command));
  } catch (e) {
    process.stderr.write(`push-gate: ${e.message}; allowing.\n`);
    process.exit(0);
  }
});
