// RIFF/WAVE (.wav) header reader and structural validator.
//
//   node tools/media/wavinfo.js <dir-or-file> [--json] [--csv] [--recurse]
//
// Walks the chunk list with seeks only, so it costs the same on a 168 MB file as
// on a 700-byte one. Reports every distinct chunk id it sees, so a non-standard
// file cannot hide behind an average.
//
// Checks, per file:
//   * 'RIFF' ... 'WAVE' container
//   * RIFF size field == filesize - 8
//   * chunk walk lands exactly on EOF (with RIFF's even-byte padding)
//   * fmt: blockAlign == channels * bitsPerSample / 8, byteRate == rate * blockAlign
//   * data size is a whole number of blocks

const fs = require('fs');
const path = require('path');

const FORMAT_TAGS = {
  0x0001: 'PCM',
  0x0002: 'ADPCM',
  0x0003: 'IEEE_FLOAT',
  0x0006: 'ALAW',
  0x0007: 'MULAW',
  0x0011: 'IMA_ADPCM',
  0x0031: 'GSM610',
  0x0055: 'MPEGLAYER3',
  0xfffe: 'EXTENSIBLE',
};

function parse(file) {
  const r = { file: path.basename(file), path: file, ok: false, errors: [], chunks: [] };
  const fd = fs.openSync(file, 'r');
  try {
    const size = fs.fstatSync(fd).size;
    r.bytes = size;
    const hdr = Buffer.alloc(12);
    if (fs.readSync(fd, hdr, 0, 12, 0) < 12) { r.errors.push('shorter than a RIFF header'); return r; }
    r.riff = hdr.toString('ascii', 0, 4);
    r.riffSize = hdr.readUInt32LE(4);
    r.form = hdr.toString('ascii', 8, 12);
    if (r.riff !== 'RIFF') { r.errors.push('not RIFF (got ' + JSON.stringify(r.riff) + ')'); return r; }
    if (r.form !== 'WAVE') { r.errors.push('not WAVE (got ' + JSON.stringify(r.form) + ')'); return r; }
    if (r.riffSize !== size - 8) r.errors.push('RIFF size ' + r.riffSize + ' != filesize-8 ' + (size - 8));

    const ch = Buffer.alloc(8);
    let pos = 12;
    while (pos + 8 <= size) {
      if (fs.readSync(fd, ch, 0, 8, pos) < 8) { r.errors.push('truncated chunk header at ' + pos); break; }
      const id = ch.toString('ascii', 0, 4);
      const len = ch.readUInt32LE(4);
      r.chunks.push({ id, len, at: pos });
      const body = pos + 8;
      if (body + len > size) { r.errors.push("chunk '" + id + "' at " + pos + ' length ' + len + ' runs past EOF'); break; }
      if (id === 'fmt ') {
        const f = Buffer.alloc(Math.min(len, 40));
        fs.readSync(fd, f, 0, f.length, body);
        r.fmtLen = len;
        r.formatTag = f.readUInt16LE(0);
        r.format = FORMAT_TAGS[r.formatTag] || ('0x' + r.formatTag.toString(16));
        r.channels = f.readUInt16LE(2);
        r.sampleRate = f.readUInt32LE(4);
        r.byteRate = f.readUInt32LE(8);
        r.blockAlign = f.readUInt16LE(12);
        r.bits = len >= 16 ? f.readUInt16LE(14) : 0;
        if (len >= 18) r.cbSize = f.readUInt16LE(16);
      } else if (id === 'data') {
        r.dataLen = len;
        r.dataAt = body;
      }
      pos = body + len + (len & 1); // RIFF pads odd chunks to an even boundary
    }
    r.chunkIds = r.chunks.map((c) => c.id);
    if (pos !== size) r.errors.push('chunk walk ended at ' + pos + ', file is ' + size);

    if (r.formatTag === undefined) r.errors.push("no 'fmt ' chunk");
    if (r.dataLen === undefined) r.errors.push("no 'data' chunk");
    if (r.formatTag === 1) {
      const expAlign = r.channels * Math.ceil(r.bits / 8);
      if (r.blockAlign !== expAlign) r.errors.push('blockAlign ' + r.blockAlign + ' != ' + expAlign);
      if (r.byteRate !== r.sampleRate * expAlign) r.errors.push('byteRate ' + r.byteRate + ' != ' + r.sampleRate * expAlign);
      if (r.dataLen !== undefined && r.blockAlign && r.dataLen % r.blockAlign) {
        r.errors.push('data length ' + r.dataLen + ' is not a whole number of ' + r.blockAlign + '-byte blocks');
      }
    }
    if (r.dataLen !== undefined && r.byteRate) {
      r.frames = r.blockAlign ? r.dataLen / r.blockAlign : 0;
      r.durationSec = r.dataLen / r.byteRate;
    }
    r.ok = r.errors.length === 0;
    return r;
  } finally {
    fs.closeSync(fd);
  }
}

function collect(target, recurse) {
  const st = fs.statSync(target);
  if (st.isFile()) return [target];
  const out = [];
  for (const e of fs.readdirSync(target, { withFileTypes: true })) {
    const p = path.join(target, e.name);
    if (e.isDirectory()) { if (recurse) out.push(...collect(p, true)); continue; }
    if (/\.wav$/i.test(e.name)) out.push(p);
  }
  return out.sort((a, b) => path.basename(a).toLowerCase().localeCompare(path.basename(b).toLowerCase()));
}

const args = process.argv.slice(2);
const target = args.find((a) => !a.startsWith('--'));
if (!target) { console.error('usage: node wavinfo.js <dir-or-file> [--json] [--csv] [--recurse]'); process.exit(2); }
const results = collect(target, args.includes('--recurse')).map(parse);

if (args.includes('--json')) {
  console.log(JSON.stringify(results, null, 2));
} else if (args.includes('--csv')) {
  console.log('file,bytes,format,tag,channels,rate,bits,blockAlign,byteRate,dataLen,seconds,chunks,ok,errors');
  for (const r of results) {
    console.log([r.file, r.bytes, r.format, r.formatTag, r.channels, r.sampleRate, r.bits,
      r.blockAlign, r.byteRate, r.dataLen, r.durationSec && r.durationSec.toFixed(3),
      '"' + (r.chunkIds || []).join(' ') + '"', r.ok ? 'ok' : 'FAIL',
      '"' + r.errors.join('; ') + '"'].join(','));
  }
} else {
  const cfg = new Map(), chunkSets = new Map(), chunkIds = new Map();
  let bad = 0, bytes = 0, seconds = 0;
  for (const r of results) {
    bytes += r.bytes;
    if (r.durationSec) seconds += r.durationSec;
    const key = `${r.format} ${r.sampleRate}Hz ${r.bits}-bit ${r.channels}ch`;
    cfg.set(key, (cfg.get(key) || 0) + 1);
    const cs = (r.chunkIds || []).join(' ');
    chunkSets.set(cs, (chunkSets.get(cs) || 0) + 1);
    for (const c of r.chunkIds || []) chunkIds.set(c, (chunkIds.get(c) || 0) + 1);
    if (!r.ok) { bad++; console.log('FAIL ' + r.file.padEnd(14) + ' ' + r.errors.join('; ')); }
  }
  const show = (label, m) => {
    console.log('\n' + label + ':');
    for (const [k, v] of [...m.entries()].sort((a, b) => b[1] - a[1])) console.log('  ' + String(v).padStart(5) + '  ' + JSON.stringify(k));
  };
  console.log('\n' + results.length + ' files, ' + bad + ' with problems, ' +
    bytes.toLocaleString() + ' bytes, ' + (seconds / 60).toFixed(1) + ' minutes of audio');
  show('format configurations', cfg);
  show('chunk layouts', chunkSets);
  show('chunk ids seen', chunkIds);
}
