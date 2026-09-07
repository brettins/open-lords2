const fs=require('fs'),path=require('path');
const D=process.argv[2]; const P=f=>path.join(D,f);
const rd=f=>fs.readFileSync(P(f));
const hdr=b=>({fam:b[0],zoom:b[1],n:b.readUInt16LE(2),h4:b.readUInt16LE(4),h6:b[6],h7:b[7]});
const recs=b=>{const n=b.readUInt16LE(2);const r=[];for(let i=0;i<n;i++){const o=8+i*16;r.push({w:b.readUInt16LE(o),h:b.readUInt16LE(o+2),off:b.readUInt32LE(o+4),x:b.readInt16LE(o+8),y:b.readInt16LE(o+10),sh:b[o+12],rows:b[o+13]});}return r;};

console.log('-- files by fam:zoom --');
const all=fs.readdirSync(D).filter(f=>/\.pl8$/i.test(f)).sort();
const g={};for(const f of all){const b=rd(f);const k=b[0]+':'+b[1];(g[k]=g[k]||[]).push(f);}
for(const k of Object.keys(g).sort())console.log(k, g[k].length, g[k].slice(0,40).join(' '));

console.log('\n-- Base2a vs Base2b --');
const a=rd('Base2a.pl8'), bb=rd('Base2b.pl8');
console.log('sizes',a.length,bb.length,'frames',a.readUInt16LE(2),bb.readUInt16LE(2));
const tab=8+a.readUInt16LE(2)*16; console.log('header+table bytes',tab);
let dh=[],dp=0; for(let i=0;i<a.length;i++){ if(a[i]!==bb[i]){ if(i<tab) dh.push(i); else dp++; } }
console.log('differing bytes in header+table:',dh,'differing pixel bytes:',dp,'pixel bytes total',a.length-tab);

console.log('\n-- Font_c2 vs Fntl2_9 --');
const c2=rd('Font_c2.pl8'), f9=rd('Fntl2_9.pl8');
console.log('hdr c2',hdr(c2),'hdr f9',hdr(f9));
const rc=recs(c2), rf=recs(f9);
let same=0, diff=[]; for(let i=0;i<Math.min(rc.length,rf.length);i++){ if(rc[i].w===rf[i].w&&rc[i].h===rf[i].h&&rc[i].rows===rf[i].rows) same++; else diff.push(i); }
console.log('frames',rc.length,rf.length,'records matching w/h/rows:',same,'differing idx:',diff);

console.log('\n-- frame counts of the five banks --');
for(const f of ['Base1a.pl8','Mtns1a.pl8','Roads1a.pl8','Town1a.pl8','Castle1a.pl8','Base01.pl8','Batlfix2.pl8'])
  { try{ const b=rd(f); console.log(f, hdr(b)); }catch(e){ console.log(f,'MISSING'); } }

console.log('\n-- region grids --');
for(const f of ['Arm_grid.pl8','Mercgrid.pl8','Villgrid.pl8','Vill_gd8.pl8']){const b=rd(f);const r=recs(b)[0];
  console.log(f,`${r.w}x${r.h}`,'span',b.length-r.off,'(w/8)*(h/8)',(r.w>>3)*(r.h>>3),'frames',b.readUInt16LE(2),'distinct',new Set(b.slice(r.off)).size, [...new Set(b.slice(r.off))].sort((x,y)=>x-y).join(','));}

console.log('\n-- overhang residuals (largest stored block per file) --');
for(const f of ['Fntl2_14.pl8','Font_10.pl8','T16_bat1.pl8','T32_bat.pl8']){const b=rd(f);const r=recs(b);let mx=0;
  for(let i=0;i<r.length;i++){const nx=i+1<r.length?r[i+1].off:b.length;const k=nx-r[i].off-r[i].w*r[i].h;if(k>mx)mx=k;}
  console.log(f,'max extra bytes',mx);}

console.log('\n-- type 4 apex stats --');
let rec4=0, nz=new Array(64).fill(0);
for(const f of all){const b=rd(f);const r=recs(b);
 for(let i=0;i<r.length;i++){ if(r[i].sh!==4) continue; const base=r[i].off+r[i].h*r[i].h;
   for(let k=0;k<r[i].rows;k++){ rec4++; for(let j=0;j<r[i].h;j++) if(b[base+k*r[i].h+j]!==0) nz[j]++; } }}
console.log('type-4 overhang records:',rec4,'byte0 nz',nz[0],'byte1 nz',nz[1],'byte28 nz',nz[28],'byte29 nz',nz[29]);
