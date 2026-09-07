// Independent re-derivation of every PL8 count asserted in docs/formats/*.md.
// Written for the audit; does not reuse tools/pl8fail or crates/l2-formats.
const fs = require('fs'), path = require('path');

function rleRows(buf, p, w, rows) {           // returns end offset or null
  for (let y = 0; y < rows; y++) {
    let x = 0;
    while (x < w) {
      if (p >= buf.length) return null;
      const op = buf[p++];
      if (op === 0) { if (p >= buf.length) return null; const s = buf[p++]; if (s === 0) return null; x += s; }
      else { p += op; x += op; }
    }
    if (x !== w) return null;
  }
  return p <= buf.length ? p : null;
}

function audit(dir) {
  const files = fs.readdirSync(dir).filter(f => /\.pl8$/i.test(f)).sort();
  const R = { dir, files: files.length, frames: 0, ok: 0, bad: [], rule: {}, fam: {}, zoomByFam: {},
              shape: {}, hdr4: new Set(), hdr6: {}, hdr7: new Set(), rec0E: 0, rec0F: 0,
              isoOK: 0, isoGeomOK: 0, mode2files: 0, mode2frames: 0, mode2iso: 0, mode2raw: 0, mode2grid: 0,
              grids: [], overhangFiles: {}, shape1RowsNonZero: 0, shape0RowsNonZero: {}, firstOffOK: 0, perFile: {} };
  const bump = (o,k)=>o[k]=(o[k]||0)+1;
  for (const f of files) {
    const buf = fs.readFileSync(path.join(dir, f));
    const fam = buf[0], zoom = buf[1], n = buf.readUInt16LE(2);
    bump(R.fam, fam); bump(R.zoomByFam, fam + ':' + zoom);
    R.hdr4.add(buf.readUInt16LE(4)); bump(R.hdr6, buf[6]); R.hdr7.add(buf[7]);
    const recs = [];
    for (let i = 0; i < n; i++) {
      const o = 8 + i * 16;
      recs.push({ w: buf.readUInt16LE(o), h: buf.readUInt16LE(o+2), off: buf.readUInt32LE(o+4),
                  x: buf.readInt16LE(o+8), y: buf.readInt16LE(o+10),
                  shape: buf[o+12], rows: buf[o+13], b0E: buf[o+14], b0F: buf[o+15] });
    }
    R.frames += n;
    if (n && recs[0].off === 8 + n * 16) R.firstOffOK++;
    // family-1 files whose every frame spans exactly w*h are really raw (Font_c2)
    const spans = recs.map((r,i)=> (i+1<n ? recs[i+1].off : buf.length) - r.off);
    const allWH = n>0 && recs.every((r,i)=> spans[i] === r.w*r.h);
    const isRle = fam === 1 && !allWH;
    const isMode2 = fam === 2;
    if (isMode2) { R.mode2files++; R.mode2frames += n; }
    let fileOK = true; const pf = {rule:{}};
    for (let i = 0; i < n; i++) {
      const r = recs[i], span = spans[i];
      if (r.b0E !== 0) R.rec0E++;
      if (r.b0F !== 0) R.rec0F++;
      bump(R.shape, r.shape);
      if (r.shape === 1 && r.rows !== 0) R.shape1RowsNonZero++;
      if (r.shape === 0 && r.rows !== 0) bump(R.shape0RowsNonZero, f);
      let rule = null;
      if (r.shape >= 1 && r.shape <= 4) {
        let need = r.h * r.h;
        if (r.shape === 2) need += r.rows * r.w;
        if (r.shape === 3 || r.shape === 4) need += r.rows * r.h;
        if (span === need) { rule = 'iso' + r.shape; R.isoOK++;
          if (r.h % 2 === 0 && r.w === 2*r.h - 2) R.isoGeomOK++;
          if (isMode2) R.mode2iso++; }
      } else if (r.shape === 0) {
        if (isRle) { const e = rleRows(buf, r.off, r.w, r.h); if (e !== null && e - r.off === span) rule = 'rle'; }
        else if (span === r.w * r.h) { rule = 'raw'; if (isMode2) R.mode2raw++; }
        else if (r.w>=8 && r.h>=8 && span === (r.w>>3)*(r.h>>3)) { rule = 'grid'; R.grids.push(f); if (isMode2) R.mode2grid++; }
        else if (span > r.w*r.h && r.rows > 0) {
          const e = rleRows(buf, r.off + r.w*r.h, r.w, r.rows);
          if (e !== null && e - r.off === span) { rule = 'rawOver'; bump(R.overhangFiles, f); }
        }
      }
      if (rule) { bump(R.rule, rule); bump(pf.rule, rule); } else { fileOK = false; R.bad.push(`${f}#${i} shape=${r.shape} rows=${r.rows} ${r.w}x${r.h} span=${span}`); }
    }
    if (fileOK) R.ok++;
    R.perFile[f] = { fam, zoom, n, size: buf.length, ...pf };
  }
  R.hdr4 = [Math.min(...R.hdr4), Math.max(...R.hdr4)];
  R.hdr7 = [Math.min(...R.hdr7), Math.max(...R.hdr7)];
  return R;
}
const dirs = process.argv.slice(2);
for (const d of dirs) {
  const R = audit(d);
  const pf = R.perFile; delete R.perFile;
  const grids = R.grids; R.grids = [...new Set(grids)];
  console.log('=== ' + d + ' ===');
  console.log(JSON.stringify(R, null, 1).slice(0, 4000));
  // per-file detail suppressed: derived from game data, not written to the repo
}
