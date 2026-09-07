// Minimal PE VA<->file-offset helper for Lords2.exe (no external deps).
const fs=require('fs');
function load(path){
  const b=fs.readFileSync(path);
  const pe=b.readUInt32LE(0x3c), opt=pe+24;
  const base=b.readUInt32LE(opt+28);
  const nsec=b.readUInt16LE(pe+6), secOff=opt+b.readUInt16LE(pe+20);
  const secs=[];
  for(let i=0;i<nsec;i++){const o=secOff+i*40;
    secs.push({name:b.toString('ascii',o,o+8).replace(/\0+$/,''),va:b.readUInt32LE(o+12),
               vsize:b.readUInt32LE(o+8),raw:b.readUInt32LE(o+20),rsize:b.readUInt32LE(o+16)});}
  const off=va=>{const r=va-base;for(const s of secs){if(r>=s.va&&r<s.va+Math.max(s.vsize,s.rsize))return s.raw+(r-s.va);}return -1;};
  return {b,base,secs,off,
    read:(va,n)=>{const o=off(va);return o<0?null:b.slice(o,o+n);},
    u32:va=>{const o=off(va);return o<0?null:b.readUInt32LE(o);}};
}
module.exports={load};
