const fs=require('fs');
const b=fs.readFileSync(process.argv[2]);
const pe=b.readUInt32LE(0x3c),opt=pe+24,ddOff=opt+96;
const nsec=b.readUInt16LE(pe+6),secOff=opt+b.readUInt16LE(pe+20);
const base=b.readUInt32LE(opt+28);
const secs=[];for(let i=0;i<nsec;i++){const o=secOff+i*40;secs.push({name:b.toString('ascii',o,o+8).replace(/\0+$/,''),va:b.readUInt32LE(o+12),vs:b.readUInt32LE(o+8),raw:b.readUInt32LE(o+20),rs:b.readUInt32LE(o+16)});}
const r2o=r=>{for(const s of secs){if(r>=s.va&&r<s.va+Math.max(s.vs,s.rs))return s.raw+(r-s.va);}return -1;};
const cstr=o=>{let e=o;while(e<b.length&&b[e])e++;return b.toString('ascii',o,e);};
const text=secs.find(s=>s.name==='.text');
const H=v=>'0x'+(v>>>0).toString(16).padStart(8,'0');

// ---- map IAT slots -> names
const iat={};
let o=r2o(b.readUInt32LE(ddOff+8));
for(let i=0;;i++){const rec=o+i*20;const nr=b.readUInt32LE(rec+12);if(!nr)break;
  const dll=cstr(r2o(nr));const oft=b.readUInt32LE(rec),ft=b.readUInt32LE(rec+16);
  const nameThunk=r2o(oft||ft);
  for(let j=0;;j++){const v=b.readUInt32LE(nameThunk+j*4);if(!v)break;
    const slotVA=base+ft+j*4;
    iat[slotVA]= dll+'!'+((v&0x80000000)?('#'+(v&0xffff)):cstr(r2o(v)+2));}}

// ---- scan .text for FF15/FF25 [abs] and record
const calls={}, thunks={};
for(let p=text.raw;p<text.raw+text.rs-6;p++){
  if(b[p]===0xFF&&(b[p+1]===0x15||b[p+1]===0x25)){
    const tgt=b.readUInt32LE(p+2);
    if(iat[tgt]){
      const va=base+text.va+(p-text.raw);
      if(b[p+1]===0x25) thunks[va]=iat[tgt];
      else (calls[iat[tgt]]=calls[iat[tgt]]||[]).push(va);
    }
  }
}
// ---- resolve E8 rel32 calls into thunks
for(let p=text.raw;p<text.raw+text.rs-5;p++){
  if(b[p]===0xE8){
    const va=base+text.va+(p-text.raw);
    const tgt=(va+5+b.readInt32LE(p+1))>>>0;
    if(thunks[tgt]) (calls[thunks[tgt]]=calls[thunks[tgt]]||[]).push(va);
  }
}
const filter=process.argv[3]||'';
console.log('=== import call sites'+(filter?' matching /'+filter+'/':'')+' ===');
for(const k of Object.keys(calls).sort()){
  if(filter&&!new RegExp(filter,'i').test(k)) continue;
  console.log(`\n${k}  (${calls[k].length} call sites)`);
  console.log('   '+calls[k].sort((a,b)=>a-b).map(H).join('  '));
}
