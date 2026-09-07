// Smacker (.smk) header reader and structural validator.
//
//   node tools/media/smkinfo.js <dir-or-file> [--json] [--csv]
//
// Reads only the fixed header, the frame-size table and the frame-flag table.
// No frame payload is decoded, so this is a *container* check: it proves the
// declared sizes account for the file exactly, which is the same class of
// self-verifying invariant the PL8 work relies on.
//
// Field layout is the publicly documented Smacker container (multimedia wiki /
// FFmpeg's demuxer description). Nothing here is copied from any implementation.

const fs = require('fs');
const path = require('path');

// Header flag bits. Bit 0 is agreed. Bits 1 and 2 both mean "display at double height",
// but libsmacker and FFmpeg name them oppositely (libsmacker: 2 = doubled, 4 = interlaced;
// FFmpeg: the reverse), so this reports the raw bit rather than picking a side.
// See docs/formats/smk.md.
const FLAG_RING_FRAME = 0x01;
const FLAG_Y_SCALE_B1 = 0x02;
const FLAG_Y_SCALE_B2 = 0x04;

// High byte of each audio-rate dword.
const AUD_PACKED = 0x80000000; // Huffman-compressed audio
const AUD_16BITS = 0x20000000;
const AUD_STEREO = 0x10000000;
const AUD_BINKAUD = 0x08000000;
const AUD_USEDCT = 0x04000000;

function parse(file) {
  const b = fs.readFileSync(file);
  const r = { file: path.basename(file), bytes: b.length, ok: false, errors: [] };
  if (b.length < 104) {
    r.errors.push('shorter than a Smacker header');
    return r;
  }
  r.magic = b.toString('ascii', 0, 4);
  if (r.magic !== 'SMK2' && r.magic !== 'SMK4') {
    r.errors.push('bad magic ' + JSON.stringify(r.magic));
    return r;
  }
  r.width = b.readUInt32LE(4);
  r.height = b.readUInt32LE(8);
  r.declaredFrames = b.readUInt32LE(12);
  r.ptsInc = b.readInt32LE(16);
  r.flags = b.readUInt32LE(20);
  r.audioSize = [];
  for (let i = 0; i < 7; i++) r.audioSize.push(b.readUInt32LE(24 + i * 4));
  r.treesSize = b.readUInt32LE(52);
  r.mmapSize = b.readUInt32LE(56);
  r.mclrSize = b.readUInt32LE(60);
  r.fullSize = b.readUInt32LE(64);
  r.typeSize = b.readUInt32LE(68);
  r.audio = [];
  for (let i = 0; i < 7; i++) {
    const v = b.readUInt32LE(72 + i * 4);
    if (v === 0) { r.audio.push(null); continue; }
    r.audio.push({
      track: i,
      raw: v >>> 0,
      rate: v & 0xffffff,
      bits: (v & AUD_16BITS) ? 16 : 8,
      channels: (v & AUD_STEREO) ? 2 : 1,
      packed: !!(v & AUD_PACKED),
      binkaud: !!(v & AUD_BINKAUD),
      usedct: !!(v & AUD_USEDCT),
      maxUnpackedBytes: r.audioSize[i],
    });
  }
  r.dummy = b.readUInt32LE(100);

  // Frame rate. ptsInc > 0 is milliseconds per frame; < 0 is units of 10 us.
  if (r.ptsInc > 0) r.fps = 1000 / r.ptsInc;
  else if (r.ptsInc < 0) r.fps = 100000 / -r.ptsInc;
  else r.fps = 10;

  r.ringFrame = !!(r.flags & FLAG_RING_FRAME);
  r.yScaleBit1 = !!(r.flags & FLAG_Y_SCALE_B1);
  r.yScaleBit2 = !!(r.flags & FLAG_Y_SCALE_B2);
  r.displayHeight = (r.yScaleBit1 || r.yScaleBit2) ? r.height * 2 : r.height;
  // A ring frame is an extra stored frame used for seamless looping.
  const stored = r.declaredFrames + (r.ringFrame ? 1 : 0);
  r.storedFrames = stored;

  const tableEnd = 104 + stored * 4 + stored;
  if (tableEnd + r.treesSize > b.length) {
    r.errors.push('frame tables + trees run past EOF');
    return r;
  }
  let sum = 0;
  let keyframes = 0;
  const paletteChanges = [];
  const audioFrames = [0, 0, 0, 0, 0, 0, 0];
  for (let i = 0; i < stored; i++) {
    const v = b.readUInt32LE(104 + i * 4);
    if (v & 1) keyframes++;
    sum += v & 0xfffffffc;
  }
  for (let i = 0; i < stored; i++) {
    const f = b[104 + stored * 4 + i];
    if (f & 1) paletteChanges.push(i);
    for (let t = 0; t < 7; t++) if (f & (2 << t)) audioFrames[t]++;
  }
  r.keyframeBits = keyframes;
  r.paletteChangeFrames = paletteChanges.length;
  r.audioFramesPerTrack = audioFrames;
  r.frameDataBytes = sum;
  r.headerBytes = tableEnd + r.treesSize;
  r.accounted = r.headerBytes + sum;
  r.slack = b.length - r.accounted;
  r.ok = r.slack === 0;
  if (!r.ok) r.errors.push('size mismatch: accounted ' + r.accounted + ' vs file ' + b.length + ' (slack ' + r.slack + ')');
  r.durationSec = r.declaredFrames / r.fps;
  return r;
}

function collect(target) {
  const st = fs.statSync(target);
  if (st.isFile()) return [target];
  return fs.readdirSync(target)
    .filter((f) => /\.smk$/i.test(f))
    .map((f) => path.join(target, f))
    .sort((a, b) => path.basename(a).toLowerCase().localeCompare(path.basename(b).toLowerCase()));
}

const args = process.argv.slice(2);
const target = args.find((a) => !a.startsWith('--'));
const asJson = args.includes('--json');
const asCsv = args.includes('--csv');
if (!target) {
  console.error('usage: node smkinfo.js <dir-or-file> [--json] [--csv]');
  process.exit(2);
}

const results = collect(target).map(parse);

if (asJson) {
  console.log(JSON.stringify(results, null, 2));
} else if (asCsv) {
  console.log('file,magic,width,height,frames,stored,fps,ptsInc,flags,trees,bytes,slack,audioTracks,audioRate,audioBits,audioCh,audioPacked');
  for (const r of results) {
    const a = (r.audio || []).find(Boolean);
    console.log([
      r.file, r.magic, r.width, r.height, r.declaredFrames, r.storedFrames,
      r.fps && r.fps.toFixed(3), r.ptsInc, r.flags, r.treesSize, r.bytes, r.slack,
      (r.audio || []).filter(Boolean).length,
      a ? a.rate : '', a ? a.bits : '', a ? a.channels : '', a ? (a.packed ? 1 : 0) : '',
    ].join(','));
  }
} else {
  let bad = 0, totalBytes = 0, totalFrames = 0;
  const geom = new Map(), rates = new Map(), magics = new Map(), audioCfg = new Map();
  for (const r of results) {
    totalBytes += r.bytes;
    if (!r.ok) { bad++; console.log('FAIL ' + r.file + ': ' + r.errors.join('; ')); continue; }
    totalFrames += r.declaredFrames;
    const g = r.width + 'x' + r.height;
    geom.set(g, (geom.get(g) || 0) + 1);
    const f = (r.fps).toFixed(3);
    rates.set(f, (rates.get(f) || 0) + 1);
    magics.set(r.magic, (magics.get(r.magic) || 0) + 1);
    const a = r.audio.filter(Boolean);
    const key = a.length === 0 ? 'none'
      : a.map((x) => `t${x.track}:${x.rate}Hz/${x.bits}bit/${x.channels}ch${x.packed ? '/packed' : ''}${x.binkaud ? '/bink' : ''}${x.usedct ? '/dct' : ''}`).join(' ');
    audioCfg.set(key, (audioCfg.get(key) || 0) + 1);
    console.log(
      r.file.padEnd(14) + r.magic + '  ' + (r.width + 'x' + r.height).padEnd(9) +
      String(r.declaredFrames).padStart(5) + 'f' +
      (r.ringFrame ? '+ring' : '     ') +
      ' ' + f.padStart(7) + 'fps ' +
      r.durationSec.toFixed(1).padStart(6) + 's ' +
      String(r.bytes).padStart(10) + 'B  flags=0x' + r.flags.toString(16) +
      ' trees=' + String(r.treesSize).padStart(6) +
      ' pal=' + String(r.paletteChangeFrames).padStart(4) +
      '  audio=' + key);
  }
  console.log('\n' + results.length + ' files, ' + bad + ' failing the size invariant');
  console.log('total bytes: ' + totalBytes.toLocaleString() + ', total frames: ' + totalFrames.toLocaleString());
  const show = (label, m) => console.log(label + ': ' + [...m.entries()].sort((a, b) => b[1] - a[1]).map(([k, v]) => k + ' x' + v).join(', '));
  show('magic', magics);
  show('geometry', geom);
  show('fps', rates);
  show('audio', audioCfg);
}
