const fs=require('fs');
const b=fs.readFileSync(process.argv[2]);
const pe=b.readUInt32LE(0x3c);
if(b.toString('ascii',pe,pe+4)!=='PE\0\0'){console.log('not PE');process.exit(1);}
const nsec=b.readUInt16LE(pe+6), optSize=b.readUInt16LE(pe+20);
const opt=pe+24, magic=b.readUInt16LE(opt);
console.log('magic', magic.toString(16), magic===0x10b?'(PE32)':'(PE32+)');
const ddOff = opt + (magic===0x10b?96:112);
const secOff = opt+optSize;
const secs=[];
for(let i=0;i<nsec;i++){const o=secOff+i*40;secs.push({name:b.toString('ascii',o,o+8).replace(/\0+$/,''),va:b.readUInt32LE(o+12),vs:b.readUInt32LE(o+8),raw:b.readUInt32LE(o+20),rs:b.readUInt32LE(o+16)});}
console.log('sections:',secs.map(s=>`${s.name}@${s.va.toString(16)}`).join(' '));
const r2o=r=>{for(const s of secs){if(r>=s.va&&r<s.va+Math.max(s.vs,s.rs))return s.raw+(r-s.va);}return -1;};
const cstr=o=>{let e=o;while(e<b.length&&b[e])e++;return b.toString('ascii',o,e);};
function walk(dirIdx,label,nameOff,thunkOff){
  const rva=b.readUInt32LE(ddOff+dirIdx*8), size=b.readUInt32LE(ddOff+dirIdx*8+4);
  console.log(`\n=== ${label} (rva=${rva.toString(16)} size=${size}) ===`);
  if(!rva){console.log('  (none)');return;}
  let o=r2o(rva);
  const step = dirIdx===1?20:32;
  for(let i=0;;i++){
    const rec=o+i*step;
    if(rec+step>b.length) break;
    let nr=b.readUInt32LE(rec+nameOff);
    if(nr===0) break;
    // delay imports in some binaries use VAs; normalize
    const nо=r2o(nr);
    if(nо<0||nо>=b.length){console.log('  <unmappable name rva '+nr.toString(16)+'>');break;}
    console.log('  '+cstr(nо));
  }
}
walk(1,'IMPORTS',12,16);
walk(13,'DELAY IMPORTS',4,16);
