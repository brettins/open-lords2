#!/usr/bin/env node
// Block posting text to GitHub until that exact text has had a copy review
// (docs/agents.md *Before a push*). Covers gh issue/pr create, comment, edit,
// review and close, and gh release create/edit. The body must come from a file,
// and tools/review/mark.js copy <file> must have marked those bytes. Exit 2
// blocks the call and sends stderr to the model.

'use strict';

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const POSTS = /(?:^|[;&|(\n])\s*gh\s+(?:(?:issue|pr)\s+(?:create|comment|edit|review|close)|release\s+(?:create|edit))\b[^;&|\n]*/;
const FILE = /(?:--body-file|--notes-file|(?:^|\s)-F)[=\s]+("[^"]+"|'[^']+'|\S+)/;
const INLINE = /(?:--body|--comment|--notes|(?:^|\s)-[bcn])(?:[=\s]+|$)/;
const API_BODY = /(?:^|[;&|(\n])\s*gh\s+api\b[^;&|\n]*\s-[fF]\s+body=/;

function block(why) {
  process.stderr.write(`Blocked: ${why}\n\n` +
    'Text posted to GitHub gets a copy review first (docs/agents.md *Before a push*):\n' +
    '  1. Write the body to a file.\n' +
    '  2. Have it read cold: plain words, every claim true against the code and the issue,\n' +
    '     CLAUDE.md rule 7 (findings, numbers, what was left alone).\n' +
    '  3. node tools/review/mark.js copy <file> --summary "<what the review changed>"\n' +
    '  4. Post with --body-file <file>. Editing the file afterwards needs a new mark.\n');
  process.exit(2);
}

function markers(dir) {
  try {
    const common = execFileSync('git', ['rev-parse', '--git-common-dir'], { cwd: dir, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
    return path.join(path.resolve(dir, common), 'l2-reviews');
  } catch {
    return null;
  }
}

function main(command) {
  if (API_BODY.test(command)) block('`gh api` with a body skips the copy review; use gh issue/pr with --body-file.');
  const post = POSTS.exec(command);
  if (!post) return;
  const seg = post[0];
  if (/\s--fill(?:-first|-verbose)?\b/.test(seg)) block('--fill posts commit text as the body without a review.');
  const file = FILE.exec(seg);
  if (!file) {
    if (INLINE.test(seg)) block('the body is inline, so it cannot be matched to a review.');
    return; // labels, titles, state: no body posted
  }
  const name = file[1].replace(/^["']|["']$/g, '');
  if (name === '-') block('the body comes from stdin, so it cannot be matched to a review.');
  const cd = /(?:^|&&|;)\s*cd\s+("[^"]+"|'[^']+'|\S+)/.exec(command);
  const dir = cd ? cd[1].replace(/^["']|["']$/g, '') : process.env.CLAUDE_PROJECT_DIR || process.cwd();
  const full = path.resolve(dir, name);
  if (!fs.existsSync(full)) return; // gh will refuse it itself
  const sha = crypto.createHash('sha256').update(fs.readFileSync(full)).digest('hex');
  const store = markers(dir);
  if (!store || !fs.existsSync(path.join(store, `copy-${sha}.json`))) {
    block(`${name} has no copy-review mark for its current text.`);
  }
}

let input = '';
process.stdin.on('data', (c) => (input += c));
process.stdin.on('end', () => {
  let command = '';
  try {
    command = (JSON.parse(input.replace(/^﻿/, '')).tool_input || {}).command || '';
  } catch (e) {
    process.stderr.write(`copy-gate: could not read the tool call (${e.message}); allowing.\n`);
    process.exit(0);
  }
  main(command);
  process.exit(0);
});
