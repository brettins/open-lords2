// node tools/pm/hub.js [out.html]  -- the player's hub page: progress, questions, activity.
// Reads docs/work.json, docs/questions.json and `work.js --status`; writes one HTML file
// (default docs/hub.html) that the lead publishes as an artifact. Rows are never typed here.
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const root = path.join(__dirname, '..', '..');
const work = JSON.parse(fs.readFileSync(path.join(root, 'docs/work.json'), 'utf8'));
const qs = JSON.parse(fs.readFileSync(path.join(root, 'docs/questions.json'), 'utf8'));
const feats = JSON.parse(fs.readFileSync(path.join(root, 'docs/features.json'), 'utf8'));
const status = execFileSync('node', [path.join(__dirname, 'work.js'), '--status'], { encoding: 'utf8' });
const head = (execFileSync('git', ['rev-parse', '--short', 'HEAD'], { cwd: root, encoding: 'utf8' })).trim();
const merged = execFileSync('git', ['log', '--merges', '--since=36 hours', '--format=%h %s'], { cwd: root, encoding: 'utf8' })
  .trim().split('\n').filter(Boolean).map((l) => ({ sha: l.slice(0, 8), text: l.slice(9).replace(/^Merge branch '([^']+)'.*$/, '$1') }));

const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const fig = (re) => { const m = status.match(re); return m ? { n: +m[1], of: +m[2] } : null; };
const figures = [
  ['End Turn differential', fig(/End Turn differential: (\d+) of (\d+)/), 'fields the original moved in one End Turn, agreeing'],
  ['Input arms', fig(/Input arms: (\d+) of (\d+)/), 'controls a screen answers, reproduced'],
  ['Sound triggers', fig(/Sound triggers: (\d+) of (\d+)/), 'trigger sites that fire'],
  ['Stored fields', fig(/Stored fields: (\d+) of (\d+)/), 'save fields carried by a load'],
].filter((f) => f[1]);
const gated = (status.match(/Install-gated tests: (\d+)/) || [])[1];

const byState = (s) => work.items.filter((i) => i.state === s);
const rows = (list) => list.map((r) => `<tr><td>${esc(r.id)}</td><td>${esc(r.title)}</td><td>${esc(r.track)}/${esc(r.system)}</td></tr>`).join('\n');
const open = qs.questions.filter((q) => q.state === 'open');
const answered = qs.questions.filter((q) => q.state !== 'open');

const qOpen = open.map((q) => `
  <section class="q"><div class="id">${q.q}<small>${esc(q.row)}</small></div><div>
    <h3>${esc(q.title)}</h3>
    ${q.inplay ? `<p><b>In play.</b> ${esc(q.inplay)}</p>` : ''}
    ${q.doubt ? `<p><b>The doubt.</b> ${esc(q.doubt)}</p>` : ''}
    ${q.todo ? `<p><b>What to do.</b> ${esc(q.todo)}</p>` : ''}
    ${q.ask ? `<p class="ask">${esc(q.ask)}</p>` : ''}
  </div></section>`).join('\n');
const qDone = answered.map((q) => `<tr><td>${q.q}</td><td>${esc(q.title)}</td><td>${esc(q.answer || '')}</td><td>${esc(q.outcome || '')}</td></tr>`).join('\n');
const reports = (qs.reports || []).map((r) => `<tr><td>${esc(r.date)}</td><td>${esc(r.text)}</td><td>${esc(r.outcome || '')}</td></tr>`).join('\n');

const MARK = { done: ['&#10003;', 'done'], partial: ['&#9684;', 'partial'], missing: ['&#10007;', 'missing'], 'not-assessed': ['&#9675;', 'not assessed'], 'out-of-scope': ['&#8212;', 'out of scope'] };
const byStatus = (list) => Object.keys(MARK).map((k) => [k, list.filter((f) => f.status === k).length]).filter((e) => e[1]);
const featureTabs = Object.entries(feats.areas).map(([key, name]) => {
  const list = feats.features.filter((f) => f.area === key);
  const counts = byStatus(list).map(([k, n]) => `<span class="fc ${k}">${n} ${MARK[k][1]}</span>`).join(' ');
  const items = list.map((f) => `<li class="f ${esc(f.status)}"><span class="mark" title="${MARK[f.status][1]}">${MARK[f.status][0]}</span><span class="fname">${esc(f.name)}</span>${f.gap ? `<span class="gap">${esc(f.gap)}</span>` : ''}${f.rows && f.rows.length ? `<span class="frows">${f.rows.map(esc).join(', ')}</span>` : ''}</li>`).join('\n');
  return `<section class="area"><h2>${esc(name)} <span class="counts">${counts}</span></h2><ul class="feats">${items}</ul></section>`;
}).join('\n');
const featTotals = byStatus(feats.features).map(([k, n]) => `<div><span>${MARK[k][1]}</span><b>${n}</b></div>`).join('');

const html = `<title>Lords2 Hub</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Newsreader:opsz,wght@6..72,500;6..72,600&family=Source+Sans+3:wght@400;600&family=JetBrains+Mono:wght@400&display=swap">
<style>
:root{--paper:#EEF0EA;--ink:#1E2621;--muted:#5C665F;--rule:#C9CFC6;--card:#F7F8F5;--azure:#24468C;--azure-soft:#DCE4F5;--ochre:#8A6116;--ochre-soft:#F1E7CC;--mono:'JetBrains Mono',ui-monospace,Consolas,monospace}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--paper:#171B18;--ink:#E6EAE3;--muted:#9AA59C;--rule:#2E3630;--card:#1F2521;--azure:#9DB6EE;--azure-soft:#22304F;--ochre:#E0B25C;--ochre-soft:#3A3018}}
:root[data-theme="dark"]{--paper:#171B18;--ink:#E6EAE3;--muted:#9AA59C;--rule:#2E3630;--card:#1F2521;--azure:#9DB6EE;--azure-soft:#22304F;--ochre:#E0B25C;--ochre-soft:#3A3018}
body{margin:0;background:var(--paper);color:var(--ink);font:16px/1.5 'Source Sans 3',system-ui,sans-serif;padding-block:28px 64px;padding-inline:clamp(16px,4vw,48px)}
main{max-width:900px;margin:0 auto}
h1{font:600 36px/1.05 'Newsreader',Georgia,serif;margin:0 0 4px}
.stamp{color:var(--muted);font-size:14px;margin:0 0 20px}.stamp code{font-family:var(--mono);font-size:13px}
nav{display:flex;gap:4px;border-bottom:1px solid var(--rule);margin-bottom:24px;overflow-x:auto}
nav button{background:none;border:0;border-bottom:3px solid transparent;color:var(--muted);font:600 14px/1 'Source Sans 3',sans-serif;letter-spacing:.06em;text-transform:uppercase;padding:12px 14px;cursor:pointer}
nav button[aria-selected="true"]{color:var(--azure);border-bottom-color:var(--azure)}
nav button:focus-visible{outline:2px solid var(--azure);outline-offset:-2px}
h2{font:500 13px/1 'Source Sans 3',sans-serif;letter-spacing:.1em;text-transform:uppercase;color:var(--muted);margin:32px 0 12px;padding-bottom:8px;border-bottom:1px solid var(--rule)}
.figs{display:grid;grid-template-columns:repeat(auto-fit,minmax(190px,1fr));gap:1px;background:var(--rule);border:1px solid var(--rule)}
.figs div{background:var(--card);padding:14px 16px}.figs b{display:block;font:600 28px/1.1 'Newsreader',Georgia,serif;font-variant-numeric:tabular-nums}
.figs span{font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:var(--muted)}.figs small{display:block;color:var(--muted);font-size:13px;margin-top:4px}
.bar{height:4px;background:var(--azure-soft);margin-top:10px}.bar i{display:block;height:100%;background:var(--azure)}
.q{display:grid;grid-template-columns:64px 1fr;gap:0 20px;padding:20px 0;border-bottom:1px solid var(--rule)}
.id{font:600 26px/1 'Newsreader',Georgia,serif;color:var(--azure);padding-top:4px}.id small{display:block;font:11px/1.4 var(--mono);color:var(--muted);margin-top:6px}
.q h3{font:600 21px/1.25 'Newsreader',Georgia,serif;margin:0 0 8px;text-wrap:balance}.q p{margin:0 0 10px;max-width:66ch}
.ask{background:var(--ochre-soft);color:var(--ochre);padding:8px 12px;font-weight:600;border-left:3px solid var(--ochre)}
.tw{overflow-x:auto}table{border-collapse:collapse;width:100%;font-size:14px}
td,th{text-align:left;padding:6px 8px;border-bottom:1px solid var(--rule);vertical-align:top}
th{font-weight:600;font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:var(--muted)}
td:first-child{font-family:var(--mono);font-size:13px;white-space:nowrap}
.muted{color:var(--muted);font-size:14px}
.feats{list-style:none;margin:0;padding:0}.f{display:grid;grid-template-columns:22px 1fr;gap:2px 10px;padding:6px 0;border-bottom:1px solid var(--rule);align-items:baseline}
.mark{font-size:15px;text-align:center}.f.done .mark{color:#2E7D4F}.f.partial .mark{color:var(--ochre)}.f.missing .mark{color:#A03A3A}.f.not-assessed .mark,.f.out-of-scope .mark{color:var(--muted)}
.fname{font-weight:600}.gap,.frows{grid-column:2;font-size:13px;color:var(--muted)}.frows{font-family:var(--mono);font-size:12px}
.counts{font-weight:400;letter-spacing:0;text-transform:none;margin-left:8px}.fc{margin-right:8px}.fc.done{color:#2E7D4F}.fc.partial{color:var(--ochre)}.fc.missing{color:#A03A3A}
.area h2{display:flex;flex-wrap:wrap;align-items:baseline}
@media (max-width:520px){.q{grid-template-columns:1fr}}
</style>
<main>
<h1>Lords2 Hub</h1>
<p class="stamp">main <code>${head}</code>, ${new Date().toISOString().slice(0, 16).replace('T', ' ')} UTC. Answer questions in chat by number, e.g. <code>Q5</code>.</p>
<nav role="tablist">
<button role="tab" id="t-progress" aria-selected="true" aria-controls="p-progress">Progress</button>
<button role="tab" id="t-features" aria-selected="false" aria-controls="p-features">Features</button>
<button role="tab" id="t-questions" aria-selected="false" aria-controls="p-questions">Questions${open.length ? ` (${open.length})` : ''}</button>
<button role="tab" id="t-activity" aria-selected="false" aria-controls="p-activity">Activity</button>
</nav>

<section id="p-progress" role="tabpanel" aria-labelledby="t-progress">
<div class="figs">
${figures.map(([name, f, what]) => `<div><span>${esc(name)}</span><b>${f.n} <span style="font-size:14px;letter-spacing:0;text-transform:none">of ${f.of}</span></b><small>${esc(what)}</small><div class="bar"><i style="width:${Math.round(100 * f.n / f.of)}%"></i></div></div>`).join('\n')}
</div>
<p class="muted">Each figure is measured by a check in the tree, not typed. ${gated ? `${gated} tests need the game installed to run.` : ''}</p>
<h2>What is left, by state</h2>
<div class="tw"><table><tr><th>State</th><th>Rows</th><th>Meaning</th></tr>
${Object.entries(work.states).map(([s, m]) => `<tr><td>${esc(s)}</td><td>${byState(s).length}</td><td>${esc(m)}</td></tr>`).join('\n')}
</table></div>
</section>

<section id="p-features" role="tabpanel" aria-labelledby="t-features" hidden>
<div class="figs">${featTotals}</div>
<p class="muted">${feats.features.length} features of the original in ${Object.keys(feats.areas).length} areas, graded in docs/features.json against evidence in the tree. A partial one names its gap and the ledger rows that cover it.</p>
${featureTabs}
</section>

<section id="p-questions" role="tabpanel" aria-labelledby="t-questions" hidden>
${open.length ? qOpen : '<p class="muted">Nothing waits on you.</p>'}
<h2>Answered</h2>
<div class="tw"><table><tr><th>Q</th><th>Question</th><th>Answer</th><th>What happened</th></tr>
${qDone}
</table></div>
<h2>Your reports</h2>
<div class="tw"><table><tr><th>Date</th><th>Report</th><th>What happened</th></tr>
${reports}
</table></div>
</section>

<section id="p-activity" role="tabpanel" aria-labelledby="t-activity" hidden>
<h2>In flight</h2>
<div class="tw"><table><tr><th>Row</th><th>What</th><th>Area</th></tr>${rows(byState('in-flight'))}</table></div>
<h2>Delivered, awaiting merge</h2>
<div class="tw"><table><tr><th>Row</th><th>What</th><th>Area</th></tr>${rows(byState('queued-merge')) || '<tr><td colspan="3" class="muted">none</td></tr>'}</table></div>
<h2>Merged in the last 36 hours</h2>
<div class="tw"><table><tr><th>Commit</th><th>Branch</th></tr>${merged.map((m) => `<tr><td>${m.sha}</td><td>${esc(m.text)}</td></tr>`).join('\n') || '<tr><td colspan="2" class="muted">none</td></tr>'}</table></div>
<h2>Waiting on you</h2>
<div class="tw"><table><tr><th>Row</th><th>What</th><th>Area</th></tr>${rows(byState('awaiting-user')) || '<tr><td colspan="3" class="muted">none</td></tr>'}</table></div>
</section>
</main>
<script>
(function(){
  var tabs=[].slice.call(document.querySelectorAll('[role=tab]'));
  function show(id){tabs.forEach(function(t){var on=t.id===id;t.setAttribute('aria-selected',on);document.getElementById(t.getAttribute('aria-controls')).hidden=!on;});try{localStorage.setItem('hub-tab',id);}catch(e){}}
  tabs.forEach(function(t){t.addEventListener('click',function(){show(t.id);});});
  var saved=null;try{saved=localStorage.getItem('hub-tab');}catch(e){}
  if(saved&&document.getElementById(saved))show(saved);
})();
</script>
`;
const out = process.argv[2] || path.join(root, 'docs/hub.html');
fs.writeFileSync(out, html);
console.log(`hub: ${out}, ${open.length} open question(s), ${byState('in-flight').length} in flight`);
