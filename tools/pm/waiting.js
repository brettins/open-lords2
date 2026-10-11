// Renders docs/questions.json (open questions first, in reply-ready form) and the
// ledger's awaiting-user rows into one HTML page: the "Waiting on You" artifact.
//   node tools/pm/waiting.js [out.html]     default: docs/waiting.html
// The lead publishes the file with the Artifact tool to the same URL each time.
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const root = path.resolve(__dirname, '..', '..');
const qs = JSON.parse(fs.readFileSync(path.join(root, 'docs/questions.json'), 'utf8'));
const work = JSON.parse(fs.readFileSync(path.join(root, 'docs/work.json'), 'utf8'));
const head = execFileSync('git', ['rev-parse', '--short', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();
const out = process.argv[2] || path.join(root, 'docs/waiting.html');

const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const num = (q) => Number(q.q.replace(/\D/g, ''));
const open = qs.questions.filter((q) => q.state === 'open').sort((a, b) => num(a) - num(b));
const answered = qs.questions.filter((q) => q.state !== 'open').sort((a, b) => num(b) - num(a));
const asked = new Set(qs.questions.filter((q) => q.state === 'open').map((q) => q.row));
const held = work.items.filter((r) => r.state === 'awaiting-user' && !asked.has(r.id));
const reports = [...qs.reports].sort((a, b) => (a.date < b.date ? 1 : -1)).slice(0, 6);
const today = new Date().toISOString().slice(0, 10);

const card = (q) => `
<article class="q" id="${esc(q.q)}">
  <header><span class="id">${esc(q.q)}</span><h3>${esc(q.title)}</h3></header>
  <p class="inplay">${esc(q.inplay)}</p>
  <p class="todo"><b>What to do.</b> ${esc(q.todo)}</p>
  <div class="reply"><code>${esc(q.ask)}</code><button type="button" class="copy" data-copy="${esc(q.ask.replace(/^Reply:\s*/, ''))}">copy</button></div>
</article>`;

const heldRow = (r) => `<li><span class="row">${esc(r.id)}</span> ${esc(r.title)}<br><span class="muted">${esc(r.next)}</span></li>`;
const ansRow = (q) => `<tr><td class="id">${esc(q.q)}</td><td>${esc(q.title)}<br><span class="muted">${esc(q.answer || '')}</span></td><td class="muted">${esc(q.outcome || '')}</td></tr>`;
const repRow = (r) => `<tr><td class="mono">${esc(r.date)}</td><td>${esc(r.text)}<br><span class="muted">${esc(r.outcome)}</span></td></tr>`;

const html = `<title>Waiting on You</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@400;500;600&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
/* Layout: one column of reply-ready cards, 68ch wide; the ID is the handle, the reply line is the control. */
:root{--bg:#f6f4ef;--panel:#ffffff;--fg:#1f1d1a;--muted:#6b665d;--line:#d9d4c8;--accent:#8a1c1c;--accent-ink:#ffffff;--chip:#efe9dc;
  --sans:"IBM Plex Sans",system-ui,-apple-system,"Segoe UI",sans-serif;--mono:"IBM Plex Mono",ui-monospace,Menlo,Consolas,monospace}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--bg:#171614;--panel:#201e1b;--fg:#ece7dd;--muted:#a39c8f;--line:#3a362f;--accent:#d9534f;--accent-ink:#1a0d0d;--chip:#2b2823;color-scheme:dark}}
:root[data-theme="dark"]{--bg:#171614;--panel:#201e1b;--fg:#ece7dd;--muted:#a39c8f;--line:#3a362f;--accent:#d9534f;--accent-ink:#1a0d0d;--chip:#2b2823;color-scheme:dark}
body{background:var(--bg);color:var(--fg);font-family:var(--sans);font-size:16px;line-height:1.5;padding-block:32px 64px;padding-inline:16px}
main{max-width:68ch;margin:0 auto;display:grid;gap:28px}
h1{font-size:2rem;font-weight:600;margin:0;letter-spacing:-.01em;text-wrap:balance}
h2{font-size:.8rem;font-weight:600;letter-spacing:.08em;text-transform:uppercase;color:var(--muted);margin:0 0 10px}
h3{font-size:1.1rem;font-weight:600;margin:0;text-wrap:balance}
p{margin:0}
.lead{color:var(--muted)}
.count{font-family:var(--mono);font-variant-numeric:tabular-nums}
.q{background:var(--panel);border:1px solid var(--line);border-left:4px solid var(--accent);padding:16px 18px;display:grid;gap:10px}
.q header{display:flex;gap:12px;align-items:baseline}
.id{font-family:var(--mono);font-weight:500;background:var(--chip);padding:1px 7px;border-radius:3px;white-space:nowrap}
.inplay{font-size:.95rem}
.todo{font-size:.95rem}
.reply{display:flex;gap:10px;align-items:center;flex-wrap:wrap;border-top:1px dashed var(--line);padding-top:10px}
.reply code{font-family:var(--mono);font-size:.9rem;min-width:0;overflow-wrap:anywhere}
.copy{font:500 .8rem var(--mono);background:var(--accent);color:var(--accent-ink);border:0;padding:4px 10px;border-radius:3px;cursor:pointer}
.copy:focus-visible,details summary:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
.muted{color:var(--muted);font-size:.9rem}
.mono{font-family:var(--mono);white-space:nowrap}
ul{margin:0;padding-left:0;list-style:none;display:grid;gap:10px}
.row{font-family:var(--mono);font-size:.85rem;color:var(--muted)}
details{border-top:1px solid var(--line);padding-top:12px}
summary{cursor:pointer;font-weight:500}
.tw{overflow-x:auto;margin-top:10px}
table{border-collapse:collapse;width:100%;font-size:.92rem}
td{vertical-align:top;padding:8px 10px 8px 0;border-bottom:1px solid var(--line)}
td.id{font-family:var(--mono);white-space:nowrap}
footer{color:var(--muted);font-size:.85rem;border-top:1px solid var(--line);padding-top:12px}
@media (prefers-reduced-motion:no-preference){.copy{transition:transform .1s}.copy:active{transform:scale(.96)}}
</style>
<main>
<div>
  <h1>Waiting on You</h1>
  <p class="lead"><span class="count">${open.length}</span> open question${open.length === 1 ? '' : 's'}. Answer in chat with the ID first, exactly as the reply line shows. Numbers are never reused.</p>
</div>
<section>
  <h2>Open</h2>
  <div style="display:grid;gap:14px">${open.map(card).join('') || '<p class="muted">Nothing. Everything asked has an answer.</p>'}</div>
</section>
<section>
  <h2>Held, no question yet</h2>
  <ul>${held.map(heldRow).join('') || '<li class="muted">none</li>'}</ul>
</section>
<details>
  <summary>Answered (${answered.length})</summary>
  <div class="tw"><table>${answered.map(ansRow).join('')}</table></div>
</details>
<details>
  <summary>Your last reports and what came of them</summary>
  <div class="tw"><table>${reports.map(repRow).join('')}</table></div>
</details>
<footer>From <code>docs/questions.json</code> and <code>docs/work.json</code> at ${esc(head)}, rendered ${today}. Regenerate: <code>node tools/pm/waiting.js</code>.</footer>
</main>
<script>
document.querySelectorAll('.copy').forEach((b) => b.addEventListener('click', async () => {
  const t = b.dataset.copy;
  try { await navigator.clipboard.writeText(t); b.textContent = 'copied'; }
  catch { const r = document.createRange(); r.selectNodeContents(b.previousElementSibling); const s = getSelection(); s.removeAllRanges(); s.addRange(r); b.textContent = 'select + copy'; }
  setTimeout(() => { b.textContent = 'copy'; }, 1500);
}));
</script>
`;
fs.writeFileSync(out, html);
console.log(`waiting: ${open.length} open, ${held.length} held, ${answered.length} answered -> ${out}`);
