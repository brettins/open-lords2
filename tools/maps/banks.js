// Plane1 (bank) x plane2 (index) census over all used slots.
const fs=require('fs');
const PATH=process.argv[2]||'F:/games/Lords of the Realm II/L2_maps.dat';
const b=fs.readFileSync(PATH);
const REC=32961,PL=4096,N=b.length/REC;
const P=(r,p)=>b.slice(r*REC+p*PL,r*REC+(p+1)*PL);
const used=[];
for(let r=0;r<N;r++){let nc=false;for(let p=0;p<6&&!nc;p++){const d=P(r,p);for(let i=1;i<PL;i++)if(d[i]!==d[0]){nc=true;break;}}if(nc)used.push(r);}
const banks={};
for(const r of used){const p1=P(r,1),p2=P(r,2);
  for(let i=0;i<PL;i++){const k=p1[i];(banks[k]=banks[k]||{n:0,idx:new Set(),max:-1,min:999})
    ;const e=banks[k];e.n++;e.idx.add(p2[i]);if(p2[i]>e.max)e.max=p2[i];if(p2[i]<e.min)e.min=p2[i];}}
const NAMES={0:'base (140 frames)',4:'mtns (25)',8:'roads (140)',12:'town (61)',16:'castle (100)'};
console.log('used slots:',used.length,'tiles:',used.length*PL);
for(const k of Object.keys(banks).map(Number).sort((a,c)=>a-c)){const e=banks[k];
  const a=[...e.idx].sort((x,y)=>x-y);
  console.log('bank 0x'+k.toString(16).padStart(2,'0'), (NAMES[k]||'?').padEnd(20),
    'tiles='+String(e.n).padStart(7),'distinct idx='+String(a.length).padStart(4),
    'range='+e.min+'..'+e.max);
  // contiguity
  const missing=[];for(let v=e.min;v<=e.max;v++)if(!e.idx.has(v))missing.push(v);
  console.log('    missing in range ('+missing.length+'):',missing.join(',').slice(0,300));
}
