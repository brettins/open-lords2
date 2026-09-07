const fs=require('fs'),path=require('path');
const D=process.argv[2]; const all=fs.readdirSync(D).filter(f=>/\.pl8$/i.test(f)).sort();
const recs=b=>{const n=b.readUInt16LE(2);const r=[];for(let i=0;i<n;i++){const o=8+i*16;r.push({w:b.readUInt16LE(o),h:b.readUInt16LE(o+2),off:b.readUInt32LE(o+4),sh:b[o+12],rows:b[o+13]});}return r;};
function t4(filter,label){
 let rec=0,byRow={},nz0=0,nz1=0,nzA=0,nzB=0,frames=0;
 for(const f of all){const b=fs.readFileSync(path.join(D,f)); if(!filter(b,f))continue; const r=recs(b);
  for(const x of r){ if(x.sh!==4)continue; frames++; const base=x.off+x.h*x.h; byRow[x.h]=(byRow[x.h]||0)+x.rows;
    for(let k=0;k<x.rows;k++){rec++; const p=base+k*x.h;
      if(b[p]!==0)nz0++; if(b[p+1]!==0)nz1++;
      if(x.h>=30){ if(b[p+x.h-2]!==0)nzA++; if(b[p+x.h-1]!==0)nzB++; } } } }
 console.log(label,{frames,rec,byRow,byte0nz:nz0,byte1nz:nz1,lastPairNzA:nzA,lastPairNzB:nzB});
}
t4(()=>true,'ALL 291 files:');
t4(b=>b[0]===2,'mode-2 files only:');
t4(b=>b[0]===2&&b[1]===0,'mode-2 zoom-0 only:');

console.log('\n-- overhang extra-byte distribution --');
for(const f of ['Fntl2_14.pl8','Font_10.pl8','T16_bat1.pl8','T32_bat.pl8']){
 const b=fs.readFileSync(path.join(D,f)); const r=recs(b); const ex=[];
 for(let i=0;i<r.length;i++){const nx=i+1<r.length?r[i+1].off:b.length;const k=nx-r[i].off-r[i].w*r[i].h; if(k)ex.push([i,k]);}
 const vals=ex.map(e=>e[1]);
 console.log(f,'frames with extra:',ex.length,'first:',ex[0],'min:',Math.min(...vals),'max:',Math.max(...vals),'sum:',vals.reduce((a,c)=>a+c,0),'2*rows-only count:',ex.filter((e,i)=>true).length);
 // total residual an old raw-only decoder would see across the file
 console.log('   distinct extras:',[...new Set(vals)].sort((a,b)=>a-b).join(','));
}
