const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');

function lookup(key) {
  const env = process.env[key];
  if (env) return env.replace(/[\\/]+$/, '');
  let text = '';
  try { text = fs.readFileSync(path.join(ROOT, 'paths.local'), 'utf8'); } catch { return null; }
  for (const line of text.split(/\r?\n/)) {
    const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*(.+?)\s*$/);
    if (m && m[1] === key && fs.existsSync(m[2])) return m[2].replace(/[\\/]+$/, '');
  }
  return null;
}

function need(key) {
  const v = lookup(key);
  if (v) return v;
  console.error(`${key} is not set: export it, or add it to paths.local (see paths.local.example).`);
  process.exit(2);
}

module.exports = {
  ROOT,
  lookup,
  need,
  game: (...rest) => path.join(need('LORDS2_DIR'), ...rest),
  dos: (...rest) => path.join(need('LORDS2_DOS_DIR'), ...rest),
};

if (require.main === module) console.log(need(process.argv[2]));
