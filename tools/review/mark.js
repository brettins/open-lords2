#!/usr/bin/env node
// Record that a review ran, for .claude/hooks/push-gate.js and copy-gate.js.
//
//   node tools/review/mark.js code [<commit>] --findings <n> --summary "<text>"
//   node tools/review/mark.js copy <file> --summary "<text>"
//
// `code` marks a commit (default HEAD) as code-reviewed up to and including
// itself. `copy` marks the exact bytes of a body file as copy-reviewed; edit the
// file afterwards and the mark no longer matches. Markers live in
// <git-common-dir>/l2-reviews, shared by every worktree and never committed.

'use strict';

const { execFileSync } = require('child_process');
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const git = (...a) => execFileSync('git', a, { encoding: 'utf8' }).trim();

function flag(args, name) {
  const i = args.indexOf(name);
  return i >= 0 ? args.splice(i, 2)[1] : undefined;
}

function usage(why) {
  process.stderr.write(`mark.js: ${why}\n` +
    '  node tools/review/mark.js code [<commit>] --findings <n> --summary "<text>"\n' +
    '  node tools/review/mark.js copy <file> --summary "<text>"\n');
  process.exit(1);
}

const args = process.argv.slice(2);
const kind = args.shift();
const summary = flag(args, '--summary');
const findings = flag(args, '--findings');
if (!summary || summary.trim().length < 10) usage('--summary says what the review found and what you did about it');

const dir = path.join(path.resolve(git('rev-parse', '--git-common-dir')), 'l2-reviews');
fs.mkdirSync(dir, { recursive: true });
const date = new Date().toISOString();

if (kind === 'code') {
  if (findings === undefined || !/^\d+$/.test(findings)) usage('--findings is the number of findings, 0 if none');
  const commit = git('rev-parse', '--verify', `${args[0] || 'HEAD'}^{commit}`);
  const file = path.join(dir, `code-${commit}.json`);
  fs.writeFileSync(file, JSON.stringify({ commit, date, findings: Number(findings), summary }, null, 1) + '\n');
  console.log(`code review marked at ${commit.slice(0, 9)}: ${file}`);
} else if (kind === 'copy') {
  if (!args[0] || !fs.existsSync(args[0])) usage('copy needs the body file you will post');
  const sha = crypto.createHash('sha256').update(fs.readFileSync(args[0])).digest('hex');
  const file = path.join(dir, `copy-${sha}.json`);
  fs.writeFileSync(file, JSON.stringify({ source: path.resolve(args[0]), sha256: sha, date, summary }, null, 1) + '\n');
  console.log(`copy review marked for ${args[0]} (${sha.slice(0, 12)})`);
} else {
  usage('the first argument is code or copy');
}
