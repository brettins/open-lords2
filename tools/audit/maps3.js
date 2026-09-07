const fs=require('fs');const SLOT=6*4096+65*129;
const w=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const slot=i=>w.subarray(i*SLOT,(i+1)*SLOT);const plane=(s,p)=>s.subarray(p*4096,(p+1)*4096);
const U=[...Array(24).keys()].concat([...Array(20).keys()].map(i=>i+40));
let blocks=0,strict=0,bankSets={},perCounty={ok:0,bad:0};
let looseAnchors=[];
for(const i of U){const s=slot(i);const P=[0,1,2,3,4,5].map(p=>plane(s,p));const at=(p,x,y)=>P[p][y*64+x];
 for(let y=0;y<63;y++)for(let x=0;x<63;x++){
  const isS=(xx,yy)=>(at(0,xx,yy)&0x80)!==0;
  if(!isS(x,y)||at(3,x,y)!==0)continue;
  if(!(isS(x+1,y)&&isS(x,y+1)&&isS(x+1,y+1)))continue;
  blocks++;
  const p3=[at(3,x+1,y),at(3,x,y+1),at(3,x+1,y+1)];
  if(p3[0]===1&&p3[1]===2&&p3[2]===3){strict++;
    const bs=[at(1,x,y),at(1,x+1,y),at(1,x,y+1),at(1,x+1,y+1)];
    const key=[...new Set(bs)].sort((a,b)=>a-b).join('/');bankSets[key]=(bankSets[key]||0)+1;}
  else looseAnchors.push([i,x,y,at(3,x,y),...p3]);
 }
 // parts 1,2,3 once per county
 const cnt={};for(let k=0;k<4096;k++)if((P[0][k]&0x80)&&P[3][k]>=1&&P[3][k]<=3){const c=P[5][k];cnt[c+':'+P[3][k]]=(cnt[c+':'+P[3][k]]||0)+1;}
 const seen=new Set();for(let k=0;k<4096;k++){const c=P[5][k];if(c>=1&&c<=16)seen.add(c);}
 for(const c of seen)for(const p of[1,2,3]) (cnt[c+':'+p]===1?perCounty.ok++:perCounty.bad++);
}
console.log('settlement 2x2 candidate anchors:',blocks,'strict p3 0/1/2/3:',strict,'non-matching:',looseAnchors);
console.log('bank composition of the strict blocks:',bankSets);
console.log('parts 1/2/3 exactly once per county: ok',perCounty.ok,'bad',perCounty.bad);

// distribution of roads-bank members per strict settlement block
{const dist={};
for(const i of U){const s=slot(i);const P=[0,1,2,3,4,5].map(p=>plane(s,p));const at=(p,x,y)=>P[p][y*64+x];
 for(let y=0;y<63;y++)for(let x=0;x<63;x++){
  const isS=(xx,yy)=>(at(0,xx,yy)&0x80)!==0;
  if(!isS(x,y)||at(3,x,y)!==0)continue;
  if(!(isS(x+1,y)&&isS(x,y+1)&&isS(x+1,y+1)))continue;
  if(!(at(3,x+1,y)===1&&at(3,x,y+1)===2&&at(3,x+1,y+1)===3))continue;
  const n=[[0,0],[1,0],[0,1],[1,1]].filter(([dx,dy])=>at(1,x+dx,y+dy)===8).length;
  dist[n]=(dist[n]||0)+1;}}
console.log('roads-bank members per settlement block:',dist);}
