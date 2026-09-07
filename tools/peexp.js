const fs=require('fs');
const b=fs.readFileSync(process.argv[2]);
const pe=b.readUInt32LE(0x3c);
if(b.toString('ascii',pe,pe+4)!=='PE\0\0'){console.log('NOT A PE FILE');process.exit(0);}
const nsec=b.readUInt16LE(pe+6),optSize=b.readUInt16LE(pe+20),opt=pe+24;
const magic=b.readUInt16LE(opt), ddOff=opt+(magic===0x10b?96:112), secOff=opt+optSize;
const secs=[];for(let i=0;i<nsec;i++){const o=secOff+i*40;secs.push({name:b.toString('ascii',o,o+8).replace(/\0+$/,''),va:b.readUInt32LE(o+12),vs:b.readUInt32LE(o+8),raw:b.readUInt32LE(o+20),rs:b.readUInt32LE(o+16)});}
const r2o=r=>{for(const s of secs){if(r>=s.va&&r<s.va+Math.max(s.vs,s.rs))return s.raw+(r-s.va);}return -1;};
const cstr=o=>{let e=o;while(e<b.length&&b[e])e++;return b.toString('ascii',o,e);};
console.log('machine=0x'+b.readUInt16LE(pe+4).toString(16),'size='+b.length,'sections='+secs.map(s=>s.name).join(','));
const erva=b.readUInt32LE(ddOff);
if(!erva){console.log('NO EXPORT TABLE');process.exit(0);}
const e=r2o(erva);
const ordBase=b.readUInt32LE(e+16), nFun=b.readUInt32LE(e+20), nName=b.readUInt32LE(e+24);
const aFun=r2o(b.readUInt32LE(e+28)), aName=r2o(b.readUInt32LE(e+32)), aOrd=r2o(b.readUInt32LE(e+36));
console.log(`ordinalBase=${ordBase} functions=${nFun} names=${nName}`);
const byOrd={};
for(let i=0;i<nName;i++){
  const nameRva=b.readUInt32LE(aName+i*4);
  const ord=b.readUInt16LE(aOrd+i*2);
  byOrd[ord+ordBase]=cstr(r2o(nameRva));
}
for(let i=0;i<nFun;i++){
  const fr=b.readUInt32LE(aFun+i*4);
  if(!fr) continue;
  console.log(`  #${i+ordBase}  ${byOrd[i+ordBase]||'(no name / by-ordinal only)'}`);
}
