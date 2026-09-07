const fs=require('fs');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961,PL=4096,N=b.length/REC;
const P=(r,p)=>b.slice(r*REC+p*PL,r*REC+(p+1)*PL);
const used=[];for(let r=0;r<N;r++){let nc=false;for(let p=0;p<6&&!nc;p++){const d=P(r,p);for(let i=1;i<PL;i++)if(d[i]!==d[0]){nc=true;break;}}if(nc)used.push(r);}
const BN={0:'base',4:'mtns',8:'roads',12:'bank0c'};
const want=+(process.argv[2]||12);
const t={};
for(const r of used){const p0=P(r,0),p1=P(r,1),p2=P(r,2),p3=P(r,3),p4=P(r,4);
 for(let i=0;i<PL;i++) if(p1[i]===want){
   const k=p2[i]+'|f0x'+p0[i].toString(16)+'|p3='+p3[i]+'|p4='+p4[i];
   t[k]=(t[k]||0)+1;}}
const rows=Object.entries(t).sort((a,c)=>{const A=a[0].split('|'),C=c[0].split('|');return (+A[0])-(+C[0])||a[0].localeCompare(c[0]);});
console.log('bank 0x'+want.toString(16)+' : idx | plane0 | plane3 | plane4 -> count   ('+rows.length+' combos)');
for(const [k,v] of rows) console.log('  ',k.padEnd(34),v);
