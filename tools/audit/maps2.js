const fs=require('fs');const SLOT=6*4096+65*129;
const w=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const slot=i=>w.subarray(i*SLOT,(i+1)*SLOT);const plane=(s,p)=>s.subarray(p*4096,(p+1)*4096);const tail=s=>s.subarray(6*4096);
const U=[...Array(24).keys()].concat([...Array(20).keys()].map(i=>i+40));
// 1. plane3 nonzero without p0&0x08
let bad={},tot=0;
for(const i of U){const s=slot(i);const P0=plane(s,0),P3=plane(s,3);
 for(let k=0;k<4096;k++)if(P3[k]!==0){tot++;if(!(P0[k]&0x08))bad['0x'+P0[k].toString(16)]=(bad['0x'+P0[k].toString(16)]||0)+1;}}
console.log('plane3 nonzero total',tot,'without p0&0x08, by p0 value:',bad, 'sum', Object.values(bad).reduce((a,c)=>a+c,0));
// 2. plane3 histograms for slots 0,1,3,4 (maps.md)
for(const i of [0,1,3,4]){const h={};for(const b of plane(slot(i),3))h[b]=(h[b]||0)+1;console.log('slot',i,'plane3 hist',h);}
// 3. per-new-map minimum diff vs DOS maps
const diffPct=(a,b)=>{let n=0;for(let i=0;i<a.length;i++)if(a[i]!==b[i])n++;return 100*n/a.length;};
const mins=[];for(let a=40;a<60;a++){let m=100;for(let b=0;b<24;b++)m=Math.min(m,diffPct(slot(a),slot(b)));mins.push([a,+m.toFixed(1)]);}
console.log('per new slot, min diff% vs any DOS map:',JSON.stringify(mins));
// 4. lattice mapping row=x+y+1 col=(x-y+64)>>1
let cells=new Set(),okAll=true;
for(let y=0;y<64;y++)for(let x=0;x<64;x++){const r=x+y+1,c=(x-y+64)>>1;if(r<0||r>128||c<0||c>64)okAll=false;cells.add(r*65+c);}
console.log('lattice mapping distinct cells:',cells.size,'all in range:',okAll,'total cells',65*129);
// 5. tail stats
const cnt06=i=>{let n=0;for(const b of tail(slot(i)))if(b===6)n++;return n;};
console.log('slots with zero 0x06 cells:',U.filter(i=>cnt06(i)===0));
console.log('slot 23 0x06 count:',cnt06(23),'blank template 24:',cnt06(24),'blank template 60:',cnt06(60));
// covered cells for slot 0
{const t=tail(slot(0));let a=0,b=0;for(let y=0;y<64;y++)for(let x=0;x<64;x++){const r=x+y+1,c=(x-y+64)>>1;const v=t[r*65+c];if(v===6)a++;else if(v===0x16)b++;}
 console.log('slot0 covered cells: 0x06=',a,'0x16=',b);}
// 6. castle/settlement structure
let castleOK=0,castleTot=0,setOK=0,setTot=0,mixedBank=0,partOK=0,partTot=0;
for(const i of U){const s=slot(i);const P=[0,1,2,3,4,5].map(p=>plane(s,p));const at=(p,x,y)=>P[p][y*64+x];
 const seen=new Uint8Array(4096);
 for(let y=0;y<63;y++)for(let x=0;x<63;x++){
  if((at(0,x,y)&0x40)&&!seen[y*64+x]){castleTot++;
   const f=[[0,0],[1,0],[0,1],[1,1]].map(([dx,dy])=>({b:at(1,x+dx,y+dy),g:at(2,x+dx,y+dy),p3:at(3,x+dx,y+dy)}));
   f.forEach((v,j)=>seen[(y+(j>1?1:0))*64+x+(j%2)]=1);
   const p3ok=f[0].p3===0&&f[1].p3===1&&f[2].p3===2&&f[3].p3===3;
   const gok=f[0].g===0&&f[1].g===2&&f[2].g===1&&f[3].g===3;
   const bok=f.every(v=>v.b===12);
   if(p3ok&&gok&&bok)castleOK++;}}
 // settlement anchors: p0&0x80, p3==0, forming 2x2 with p3 0/1/2/3
 for(let y=0;y<63;y++)for(let x=0;x<63;x++){
  if((at(0,x,y)&0x80)&&at(3,x,y)===0&&(at(0,x+1,y)&0x80)&&(at(0,x,y+1)&0x80)&&(at(0,x+1,y+1)&0x80)){
   setTot++;const p3=[at(3,x,y),at(3,x+1,y),at(3,x,y+1),at(3,x+1,y+1)];
   if(p3[0]===0&&p3[1]===1&&p3[2]===2&&p3[3]===3)setOK++;
   const banks=new Set([at(1,x,y),at(1,x+1,y),at(1,x,y+1),at(1,x+1,y+1)]);
   if(banks.size>1)mixedBank++;}}
}
console.log('castle blocks',castleTot,'matching Town frames 0/2/1/3 + p3 0/1/2/3:',castleOK);
console.log('settlement 2x2 anchors',setTot,'with p3 0/1/2/3:',setOK,'with mixed banks:',mixedBank);
