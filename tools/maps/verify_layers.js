// Verify plane-1/2/3/4 claims over ALL used slots of L2_maps.dat, file-only.
const fs=require('fs');
const PATH=process.argv[2]||'F:/games/Lords of the Realm II/L2_maps.dat';
const D='F:/games/Lords of the Realm II/';
const b=fs.readFileSync(PATH);
const REC=32961,PL=4096,N=b.length/REC;
const P=(r,p)=>b.slice(r*REC+p*PL,r*REC+(p+1)*PL);
const used=[];for(let r=0;r<N;r++){let nc=false;for(let p=0;p<6&&!nc;p++){const d=P(r,p);for(let i=1;i<PL;i++)if(d[i]!==d[0]){nc=true;break;}}if(nc)used.push(r);}
console.log('used slots:',used.length);

// ---- 1. bank -> tile set: frame counts must bound the observed index range
const SETS=[['base','Base1a.pl8'],['mtns','Mtns1a.pl8'],['roads','Roads1a.pl8'],['town','Town1a.pl8'],['castle','Castle1a.pl8']];
const nframes={};
for(const [n,f] of SETS){const x=fs.readFileSync(D+f);nframes[n]=x.readUInt16LE(2);}
const BANK={0:'base',4:'mtns',8:'roads',12:'town',16:'castle'};
const seen={};
for(const r of used){const p1=P(r,1),p2=P(r,2);
 for(let i=0;i<PL;i++){const k=p1[i];(seen[k]=seen[k]||new Set()).add(p2[i]);}}
console.log('\n== 1. bank -> tile set ==');
for(const k of Object.keys(seen).map(Number).sort((a,c)=>a-c)){
  const s=seen[k],a=[...s].sort((x,y)=>x-y),nm=BANK[k],nf=nframes[nm];
  console.log(`  bank 0x${k.toString(16).padStart(2,'0')} = ${nm.padEnd(6)} frames=${String(nf).padStart(3)}  indices used ${a[0]}..${a[a.length-1]} (${a.length} distinct)  in range: ${a[a.length-1]<nf?'YES':'NO'}${a.length===nf?'   *saturated: uses every frame*':''}`);
}

// ---- 2. multi-tile objects: plane3 = dx + W*dy, frames in iso screen order
// screen order of an object's WxH tile block: sort (dx,dy) by (dx+dy), then dx
function screenOrder(W,H){const a=[];for(let dy=0;dy<H;dy++)for(let dx=0;dx<W;dx++)a.push([dx,dy]);
  a.sort((p,q)=>(p[0]+p[1])-(q[0]+q[1])||p[0]-q[0]);return a;}
// group definitions per bank, derived from the PL8 frame X/Y layout
const GROUPS=[
  {bank:4,  base:0,  W:2,H:2},{bank:4,base:4,W:2,H:2},{bank:4,base:8,W:2,H:2},{bank:4,base:12,W:2,H:2},
  {bank:4,  base:16, W:3,H:3},
  {bank:12, base:0,  W:2,H:2},                 // castle site placeholder, town1a[0..3]
];
let gOK=0,gBAD=0,gTot=0;const badEx=[];
for(const r of used){const p1=P(r,1),p2=P(r,2),p3=P(r,3);
 for(const g of GROUPS){const order=screenOrder(g.W,g.H);
  for(let i=0;i<PL;i++){
    if(p1[i]!==g.bank) continue;
    const q=p2[i]-g.base; if(q<0||q>=g.W*g.H) continue;
    // only accept if this index really belongs to this group (indices are unique per bank)
    gTot++;
    const [dx,dy]=order[q];
    const want=dx+g.W*dy;
    if(p3[i]===want) gOK++; else {gBAD++; if(badEx.length<5)badEx.push({slot:r,i,bank:g.bank,idx:p2[i],p3:p3[i],want});}
  }}}
console.log('\n== 2. plane3 == dx + W*dy, with PL8 frame order = iso screen order ==');
console.log(`  mtns 2x2/3x3 groups + town castle-site: ${gOK}/${gTot} tiles agree, ${gBAD} disagree`, badEx.length?JSON.stringify(badEx):'');

// ---- 2b. geometric closure: every part sits at the right (x,y) offset from its anchor
let cOK=0,cBAD=0,cTot=0;const cEx=[];
for(const r of used){const p1=P(r,1),p2=P(r,2),p3=P(r,3);
 for(const g of GROUPS){const order=screenOrder(g.W,g.H);
  for(let i=0;i<PL;i++){
   if(p1[i]!==g.bank) continue;
   const q=p2[i]-g.base; if(q!==0) continue;         // anchor frame only
   if(p3[i]!==0) continue;
   cTot++;
   const x=i%64,y=(i/64)|0; let ok=true;
   for(let k=1;k<g.W*g.H;k++){const [dx,dy]=order[k];
     const j=(y+dy)*64+(x+dx);
     if(x+dx>63||y+dy>63){ok=false;break;}
     if(p1[j]!==g.bank||p2[j]!==g.base+k||p3[j]!==dx+g.W*dy){ok=false;break;}}
   if(ok)cOK++;else{cBAD++;if(cEx.length<5)cEx.push({slot:r,x,y,bank:g.bank,base:g.base});}
  }}}
console.log(`  anchors whose whole ${'WxH'} block is present and correctly indexed: ${cOK}/${cTot}`, cEx.length?JSON.stringify(cEx):'');

// ---- 3. settlement 2x2 blocks (plane0 & 0x80)
let sOK=0,sTot=0,sBad=[];
for(const r of used){const p0=P(r,0),p3=P(r,3);
 const set=new Set();for(let i=0;i<PL;i++)if(p0[i]&0x80)set.add(i);
 const done=new Set();
 for(const i of [...set].sort((a,c)=>a-c)){if(done.has(i))continue;
   if(p3[i]!==0)continue;
   const x=i%64,y=(i/64)|0;
   const q=[[1,0,1],[0,1,2],[1,1,3]];
   let ok=x<63&&y<63;
   if(ok)for(const [dx,dy,pv] of q){const j=(y+dy)*64+x+dx; if(!set.has(j)||p3[j]!==pv){ok=false;break;}}
   sTot++; if(ok){sOK++;[i,i+1,i+64,i+65].forEach(k=>done.add(k));} else if(sBad.length<5)sBad.push({slot:r,x,y});}
 }
console.log('\n== 3. settlement (plane0 & 0x80) 2x2 blocks with plane3 = 0,1,2,3 ==');
console.log(`  ${sOK}/${sTot} anchors form a complete 2x2`, sBad.length?JSON.stringify(sBad):'');

// ---- 4. plane 4
console.log('\n== 4. plane 4 ==');
const cls={};
for(const r of used){const p0=P(r,0),p4=P(r,4);
 for(let i=0;i<PL;i++){if(!p4[i])continue;
  const k=(p0[i]&0x40)?'castle 0x40':(p0[i]&0x80)?'settlement 0x80':'OTHER flags 0x'+p0[i].toString(16);
  (cls[k]=cls[k]||{})[p4[i]]=((cls[k]||{})[p4[i]]||0)+1;}}
for(const k of Object.keys(cls).sort())console.log('  ',k.padEnd(20),JSON.stringify(cls[k]));
// settlement start table per map
console.log('  per-map settlement start table (plane4 on 0x80 tiles):');
const hist={};
for(const r of used){const p0=P(r,0),p4=P(r,4),p5=P(r,5);const v=[];
 for(let i=0;i<PL;i++) if((p0[i]&0x80)&&p4[i]) v.push(p4[i]+':c'+p5[i]);
 v.sort();const key=v.map(s=>s.split(':')[0]).join(',');hist[key]=(hist[key]||0)+1;
 if(r<4||r===40) console.log('    slot',r,v.join(' '));}
console.log('   value-multiset histogram:',JSON.stringify(hist));
// castle plane4 -> six lists
console.log('  castle plane4: six 16-entry county lists (row n-1 gets county for each castle tile with plane4=n)');
const rowsizes={};
for(const r of used){const p0=P(r,0),p4=P(r,4),p5=P(r,5);
 const rows=[[],[],[],[],[],[]];
 for(let i=0;i<PL;i++) if((p0[i]&0x40)&&p4[i]) rows[p4[i]-1].push(p5[i]);
 const sizes=rows.map(x=>x.length);
 const key=sizes.join(',');rowsizes[key]=(rowsizes[key]||0)+1;
 if(r<6||r===40) console.log('    slot',String(r).padStart(2),'sizes',key,' rows:',rows.map(x=>'['+[...new Set(x)].sort((a,c)=>a-c).join(' ')+']').join(''));
 }
console.log('   row-size-vector histogram:'); Object.entries(rowsizes).sort((a,c)=>c[1]-a[1]).forEach(([k,v])=>console.log('     ',k,'x'+v));
