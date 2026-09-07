const fs=require('fs');
const W='F:/games/Lords of the Realm II/L2_maps.dat', DOS='F:/games/LORDS2/L2_MAPS.DAT';
const SLOT=6*4096+65*129;
const w=fs.readFileSync(W), d=fs.readFileSync(DOS);
console.log('sizes',w.length,d.length,'slots',w.length/SLOT,d.length/SLOT,'SLOT',SLOT,'0x'+SLOT.toString(16));
console.log('DOS is byte-identical prefix:',Buffer.compare(w.subarray(0,d.length),d)===0);
const slot=i=>w.subarray(i*SLOT,(i+1)*SLOT);
const plane=(s,p)=>s.subarray(p*4096,(p+1)*4096);
const tail=s=>s.subarray(6*4096);
// used census, two definitions
const usedZero=[],usedNonConst=[];
for(let i=0;i<80;i++){const s=slot(i);
  let allz=true;for(let k=0;k<6*4096;k++)if(s[k]){allz=false;break;}
  if(!allz)usedZero.push(i);
  let nonconst=false;for(let p=0;p<6;p++){const pl=plane(s,p);for(let k=1;k<4096;k++)if(pl[k]!==pl[0]){nonconst=true;break;}if(nonconst)break;}
  if(nonconst)usedNonConst.push(i);}
console.log('used (planes not all zero):',usedZero.length,usedZero.join(','));
console.log('used (some plane non-constant):',usedNonConst.length, JSON.stringify(usedNonConst)===JSON.stringify(usedZero)?'(same)':usedNonConst.join(','));
const U=usedZero;
// empty-slot identity
const eq=(a,b)=>Buffer.compare(a,b)===0;
const g1=[],g2=[];for(let i=24;i<40;i++)g1.push(i);for(let i=60;i<80;i++)g2.push(i);
console.log('24-39 all identical:',g1.every(i=>eq(slot(i),slot(24))),'60-79 all identical:',g2.every(i=>eq(slot(i),slot(60))),'groups differ:',!eq(slot(24),slot(60)));
console.log('empty tail alphabet 24:',[...new Set(tail(slot(24)))],'60:',[...new Set(tail(slot(60)))]);
// slot 40 vs 0
const diffPct=(a,b)=>{let n=0;for(let i=0;i<a.length;i++)if(a[i]!==b[i])n++;return 100*n/a.length;};
console.log('slot40 vs slot0 diff %:',diffPct(slot(40),slot(0)).toFixed(2),
  'county plane diffs:',(()=>{let n=0;const A=plane(slot(40),5),B=plane(slot(0),5);for(let i=0;i<4096;i++)if(A[i]!==B[i])n++;return n;})(),
  'tail identical:',eq(tail(slot(40)),tail(slot(0))));
let mn=100,mx=0;for(const a of U.filter(i=>i>=41))for(const b of U.filter(i=>i<24)){const p=diffPct(slot(a),slot(b));if(p<mn)mn=p;if(p>mx)mx=p;}
console.log('slots 41-59 vs 0-23 diff % range:',mn.toFixed(1),'-',mx.toFixed(1));
// unique
let dup=0;for(let i=0;i<U.length;i++)for(let j=i+1;j<U.length;j++)if(eq(slot(U[i]),slot(U[j])))dup++;
console.log('duplicate used slots:',dup);
// alphabets
const alpha=[...Array(6)].map(()=>new Set());
let tiles=0;
for(const i of U){const s=slot(i);for(let p=0;p<6;p++){const pl=plane(s,p);for(let k=0;k<4096;k++)alpha[p].add(pl[k]);}tiles+=4096;}
console.log('tiles:',tiles);
alpha.forEach((a,p)=>console.log(' plane',p,'distinct',a.size,[...a].sort((x,y)=>x-y).map(v=>'0x'+v.toString(16)).join(',')));
// tail alphabet used
const ta=new Set();for(const i of U)for(const b of tail(slot(i)))ta.add(b);
console.log('tail alphabet over used slots:',[...ta].map(v=>'0x'+v.toString(16)));
// invariants
let agree=0, castleTiles=0, blocks=0, leftover=0, per4=true, p3flag=true, p3nz=0;
let bankTiles={0:0,4:0,8:0,12:0,16:0}, p2rangeByBank={};
let classes={};
let bit02=0,bit02adj=0,land=0,landAdj=0,boundaryTiles=0;
let bit10=0, per10ok=true;
let p4set={}, p4castle={}, mapMultisets={};
for(const i of U){const s=slot(i);const P=[0,1,2,3,4,5].map(p=>plane(s,p));
 const at=(p,x,y)=>P[p][y*64+x];
 // counties
 const seen=new Set();for(let k=0;k<4096;k++){const c=P[5][k];if(c>=1&&c<=16)seen.add(c);}
 const nc=seen.size;
 let ct=0;const marked=new Uint8Array(4096);let bl=0;
 for(let y=0;y<64;y++)for(let x=0;x<64;x++){
   const f=at(0,x,y),c=at(5,x,y),b=at(1,x,y),g=at(2,x,y),o=at(3,x,y),m=at(4,x,y);
   if(((f&4)!==0)===(c===0))agree++;
   if(f&0x40){ct++;
     if(!marked[y*64+x]){ if(x+1<64&&y+1<64&&(at(0,x+1,y)&0x40)&&(at(0,x,y+1)&0x40)&&(at(0,x+1,y+1)&0x40)){bl++;marked[y*64+x]=marked[y*64+x+1]=marked[(y+1)*64+x]=marked[(y+1)*64+x+1]=1;} else leftover++; }}
   if(o!==0){p3nz++; if(!(f&0x08))p3flag=false;}
   bankTiles[b]=(bankTiles[b]||0)+1;
   const key=b; (p2rangeByBank[key]=p2rangeByBank[key]||new Set()).add(g);
   const cls=('0x'+f.toString(16))+'|'+b; classes[cls]=classes[cls]||{n:0,idx:new Set()}; classes[cls].n++; classes[cls].idx.add(g);
   if(f&0x02)bit02++;
   if(c!==0){land++;
     let adj=false;for(const[dx,dy]of[[1,0],[-1,0],[0,1],[0,-1]]){const nx=x+dx,ny=y+dy;if(nx<0||ny<0||nx>63||ny>63)continue;const c2=at(5,nx,ny);if(c2!==0&&c2!==c)adj=true;}
     if(adj){landAdj++;boundaryTiles++;}
     if((f&0x02)&&adj)bit02adj++;}
   if(f&0x10)bit10++;
   if(m!==0){ if(f&0x40){p4castle[m]=(p4castle[m]||0)+1;} else if(f&0x80){p4set[m]=(p4set[m]||0)+1;} }
 }
 blocks+=bl; castleTiles+=ct; if(ct!==4*nc)per4=false;
 // 0x10 per county
 const cnt10={};for(let k=0;k<4096;k++)if(P[0][k]&0x10)cnt10[P[5][k]]=(cnt10[P[5][k]]||0)+1;
 for(const c of seen)if(cnt10[c]!==4)per10ok=false;
 // settlement multiset
 const ms=[];for(let k=0;k<4096;k++)if(P[4][k]!==0&&(P[0][k]&0x80)&&!(P[0][k]&0x40))ms.push(P[4][k]);
 ms.sort((a,b)=>a-b);const key=ms.join(',');mapMultisets[key]=(mapMultisets[key]||0)+1;
}
console.log('invariant (p0&4)<=>(p5==0):',agree+'/'+tiles,(100*agree/tiles).toFixed(4)+'%');
console.log('castle 0x40 tiles:',castleTiles,'2x2 blocks:',blocks,'leftover:',leftover,'per-map 4*counties:',per4);
console.log('plane3 nonzero:',p3nz,'all have p0&0x08:',p3flag);
console.log('bank tile counts:',bankTiles);
for(const k in p2rangeByBank){const a=[...p2rangeByBank[k]].sort((x,y)=>x-y);console.log('  bank',k,'p2 min',a[0],'max',a[a.length-1],'distinct',a.length);}
console.log('bit 0x02 tiles:',bit02,'of which boundary-adjacent:',bit02adj);
console.log('land tiles:',land,'boundary-adjacent:',boundaryTiles,(100*boundaryTiles/land).toFixed(1)+'%');
console.log('bit 0x10 tiles:',bit10,'exactly 4 per county everywhere:',per10ok);
console.log('plane4 on settlement tiles:',p4set);
console.log('plane4 on castle tiles:',p4castle);
console.log('settlement multisets:',mapMultisets);
console.log('\nplane0|bank classes:');
const cl=Object.entries(classes).sort((a,b)=>b[1].n-a[1].n);
for(const[k,v]of cl){const a=[...v.idx].sort((x,y)=>x-y);console.log('  ',k,v.n,'idx',a[0]+'-'+a[a.length-1],'('+a.length+' distinct)');}
