const fs=require('fs'),path=require('path');
const dir=process.argv[2];
const files=fs.readdirSync(dir).filter(f=>/\.pl8$/i.test(f)).sort();
const s1={}, f0F={}, zoomIso={}, isoSizes={};
for(const f of files){
  const b=fs.readFileSync(path.join(dir,f)); const fam=b[0],zoom=b[1],n=b.readUInt16LE(2);
  for(let i=0;i<n;i++){const o=8+i*16;const sh=b[o+12],rows=b[o+13],w=b.readUInt16LE(o),h=b.readUInt16LE(o+2);
    if(sh===1&&rows!==0){s1[f]=(s1[f]||0)+1;}
    if(b[o+15]!==0){f0F[f]=(f0F[f]||0)+1;}
    if(sh>=1&&sh<=4){ (zoomIso[fam+':'+zoom]=zoomIso[fam+':'+zoom]||new Set()).add(w+'x'+h); }
  }
}
console.log('shape1 with rows!=0 per file:',s1);
console.log('rec byte 0x0F non-zero per file:',f0F, 'families:', Object.keys(f0F).map(f=>f+'=fam'+fs.readFileSync(path.join(dir,f))[0]));
const z={};for(const k in zoomIso)z[k]=[...zoomIso[k]];
console.log('iso tile sizes by fam:zoom:',z);
