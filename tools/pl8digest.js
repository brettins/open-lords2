// pl8digest.js - emit a deterministic digest of every PL8 frame in a directory.
// Usage: node pl8digest.js <dir>
//
// The reference half of the cross-implementation differential test; the Rust
// half is crates/l2-formats/examples/pl8digest.rs and tools/pl8diff.ps1 compares
// them. Written from docs/formats/pl8.md rather than transliterated from the
// Rust, so that agreement is evidence of two readings converging.
//
// Line format (files sorted by name, frames in index order):
//   <name> hdr mode=<m> sub=<s> frames=<n> verdict=<ok|err:...>
//   <name> <frameIndex> <w>x<h> <fnv1a64 hex | err:...>
const fs = require('fs'), path = require('path');

// FNV-1a 64 over 16-bit limbs. Node's crypto would be easier, but the Rust side
// must stay dependency-free, so both sides hand-roll the same trivial hash.
// BigInt would be the obvious way to get 64 bits here and is far too slow for
// the ~10^8 bytes this walks.
function Fnv() { this.h0 = 0x2325; this.h1 = 0x8422; this.h2 = 0x9ce4; this.h3 = 0xcbf2; }
Fnv.prototype.push = function (b) {
  const h0 = this.h0 ^ b, h1 = this.h1, h2 = this.h2, h3 = this.h3;
  // multiply by 0x0000_0100_0000_01b3, whose 16-bit limbs are
  // p0 = 0x01b3, p1 = 0, p2 = 0x0100 (the prime's 2^40 term), p3 = 0
  const r0 = h0 * 0x1b3;
  const t1 = h1 * 0x1b3 + (r0 >>> 16);
  const t2 = h2 * 0x1b3 + h0 * 0x100 + (t1 >>> 16);
  const t3 = h3 * 0x1b3 + h1 * 0x100 + (t2 >>> 16);
  this.h0 = r0 & 0xffff; this.h1 = t1 & 0xffff;
  this.h2 = t2 & 0xffff; this.h3 = t3 & 0xffff;
};
Fnv.prototype.hex = function () {
  const p = v => v.toString(16).padStart(4, '0');
  return p(this.h3) + p(this.h2) + p(this.h1) + p(this.h0);
};

// Errors are thrown as normalised tokens from a vocabulary shared with the Rust
// side, so the two decoders are compared on their failure verdicts too.
const fail = tok => { const e = new Error(tok); e.token = tok; throw e; };
const tokenOf = e => e.token || ('err:internal:' + e.message);

function parse(b) {
  if (b.length < 8) fail('err:truncated');
  const storage = b[0], sub = b[1], count = b.readUInt16LE(2);
  const frames = [];
  for (let i = 0; i < count; i++) {
    const rec = 8 + i * 16;
    if (rec + 16 > b.length) fail('err:truncated');
    frames.push({ w: b.readUInt16LE(rec), h: b.readUInt16LE(rec + 2), off: b.readUInt32LE(rec + 4) });
  }
  return { b, storage, sub, frames };
}

// Returns { indices, opaque, end }. `end` is where the frame's data stopped,
// which is what makes the container self-verifying.
function decode(f, i) {
  const b = f.b, info = f.frames[i];
  if (!info) fail('err:framerange:' + i);
  const w = info.w, h = info.h, pixels = w * h;
  const indices = new Uint8Array(pixels), opaque = new Uint8Array(pixels);
  let end;

  if (f.storage === 0) {
    end = info.off + pixels;
    if (end > b.length) fail('err:truncated');
    indices.set(b.subarray(info.off, end));
    // Palette index 0 is transparent: the game blits only non-zero bytes
    // (verified in the original at 0x004B43B1).
    for (let k = 0; k < pixels; k++) opaque[k] = indices[k] !== 0 ? 1 : 0;
  } else if (f.storage === 1) {
    let p = info.off;
    const next = () => { if (p >= b.length) fail('err:truncated'); return b[p++]; };
    for (let y = 0; y < h; y++) {
      let x = 0;
      while (x < w) {
        const n = next();
        if (n === 0) {
          const skip = next();
          // A zero-length skip advances nothing and would loop forever.
          if (skip === 0) fail(`err:zerorun:${i}:${y}`);
          x += skip;
        } else {
          for (let k = 0; k < n; k++) {
            const v = next(), xi = x + k;
            // A run may declare more pixels than the row has left; the row-width
            // check below rejects the frame, so nothing spills into row y + 1.
            if (xi < w) { indices[y * w + xi] = v; opaque[y * w + xi] = v !== 0 ? 1 : 0; }
          }
          x += n;
        }
      }
      if (x !== w) fail(`err:rowoverrun:${i}:${y}:${x}:${w}`);
    }
    end = p;
  } else {
    fail('err:unsupported:' + f.storage);
  }
  return { w, h, indices, opaque, end };
}

const boundary = (f, i) => (i + 1 < f.frames.length ? f.frames[i + 1].off : f.b.length);

// Every frame decodes and ends exactly where the next begins.
function validate(f) {
  for (let i = 0; i < f.frames.length; i++) {
    const { end } = decode(f, i);
    if (end !== boundary(f, i)) fail('err:framesize:' + i);
  }
}

const dir = process.argv[2];
if (!dir) { console.error('usage: pl8digest <dir>'); process.exit(2); }

const names = fs.readdirSync(dir).filter(n => /\.pl8$/i.test(n)).sort();
const out = [];
for (const name of names) {
  const b = fs.readFileSync(path.join(dir, name));
  let f;
  try { f = parse(b); } catch (e) { out.push(`${name} hdr ${tokenOf(e)}`); continue; }

  let verdict = 'ok';
  try { validate(f); } catch (e) { verdict = tokenOf(e); }
  out.push(`${name} hdr mode=${f.storage} sub=${f.sub} frames=${f.frames.length} verdict=${verdict}`);

  // Digest frames even when the file failed validation: most failures are a
  // trailing-byte discrepancy, and the pixels are still worth comparing.
  for (let i = 0; i < f.frames.length; i++) {
    const info = f.frames[i];
    let d;
    try { d = decode(f, i); }
    catch (e) { out.push(`${name} ${i} ${info.w}x${info.h} ${tokenOf(e)}`); continue; }
    const hash = new Fnv();
    for (let k = 0; k < d.indices.length; k++) { hash.push(d.indices[k]); hash.push(d.opaque[k]); }
    out.push(`${name} ${i} ${d.w}x${d.h} ${hash.hex()}`);
  }
}
process.stdout.write(out.join('\n') + '\n');
