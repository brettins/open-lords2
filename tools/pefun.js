const fs=require('fs');
const b=fs.readFileSync(process.argv[2]);
const pe=b.readUInt32LE(0x3c);
const nsec=b.readUInt16LE(pe+6), optSize=b.readUInt16LE(pe+20);
const opt=pe+24, ddOff=opt+96, secOff=opt+optSize;
const secs=[];
for(let i=0;i<nsec;i++){const o=secOff+i*40;secs.push({va:b.readUInt32LE(o+12),vs:b.readUInt32LE(o+8),raw:b.readUInt32LE(o+20),rs:b.readUInt32LE(o+16)});}
const r2o=r=>{for(const s of secs){if(r>=s.va&&r<s.va+Math.max(s.vs,s.rs))return s.raw+(r-s.va);}return -1;};
const cstr=o=>{let e=o;while(e<b.length&&b[e])e++;return b.toString('ascii',o,e);};
const rva=b.readUInt32LE(ddOff+8);
let o=r2o(rva);
for(let i=0;;i++){
  const rec=o+i*20;
  const nameRva=b.readUInt32LE(rec+12);
  if(!nameRva) break;
  const dll=cstr(r2o(nameRva));
  const oft=b.readUInt32LE(rec), ft=b.readUInt32LE(rec+16);
  const thunk=r2o(oft||ft);
  const fns=[];
  for(let j=0;;j++){
    const v=b.readUInt32LE(thunk+j*4);
    if(!v) break;
    if(v&0x80000000) fns.push('#ordinal'+(v&0xffff));
    else { const no=r2o(v); fns.push(cstr(no+2)); }
  }
  console.log(`\n${dll}  (${fns.length} imports)`);
  if(/dplay|smack|dsound|ddraw/i.test(dll)) console.log('   '+fns.join('\n   '));
}
