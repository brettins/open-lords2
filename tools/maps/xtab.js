const fs=require('fs');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961,PL=4096,N=b.length/REC;
const P=(r,p)=>b.slice(r*REC+p*PL,r*REC+(p+1)*PL);
const used=[];for(let r=0;r<N;r++){let nc=false;for(let p=0;p<6&&!nc;p++){const d=P(r,p);for(let i=1;i<PL;i++)if(d[i]!==d[0]){nc=true;break;}}if(nc)used.push(r);}
const BN={0:'base',4:'mtns',8:'roads',12:'town'};
// plane0 bit x bank
const bits=[0x01,0x02,0x04,0x08,0x10,0x20,0x40,0x80];
const tab={};
for(const r of used){const p0=P(r,0),p1=P(r,1),p2=P(r,2),p3=P(r,3),p4=P(r,4);
 for(let i=0;i<PL;i++){
  for(const bt of bits){ if(p0[i]&bt){const k='bit0x'+bt.toString(16)+'|'+BN[p1[i]];tab[k]=(tab[k]||0)+1;} }
  const k2='p3='+p3[i]+'|'+BN[p1[i]]; tab[k2]=(tab[k2]||0)+1;
 }}
console.log('--- plane0 bit x bank ---');
for(const bt of bits){const row=Object.keys(BN).map(k=>('bit0x'+bt.toString(16)+'|'+BN[k]));
  console.log('0x'+bt.toString(16).padStart(2,'0'), row.map((k,j)=>Object.values(BN)[j]+'='+String(tab[k]||0).padStart(6)).join('  '));}
console.log('--- plane3 value x bank ---');
for(let v=0;v<=8;v++){const row=Object.keys(BN).map(k=>('p3='+v+'|'+BN[k]));
  console.log('p3='+v, row.map((k,j)=>Object.values(BN)[j]+'='+String(tab[k]||0).padStart(6)).join('  '));}
// plane3 index sets per value
console.log('--- for plane3 != 0: bank/index combos ---');
const combo={};
for(const r of used){const p1=P(r,1),p2=P(r,2),p3=P(r,3);
 for(let i=0;i<PL;i++) if(p3[i]) { const k=p3[i]+' -> '+BN[p1[i]]+'['+p2[i]+']'; combo[k]=(combo[k]||0)+1; }}
for(const k of Object.keys(combo).sort()) console.log('  ',k, combo[k]);
