const fs=require('fs');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961,PL=4096,LW=65,LH=129,N=b.length/REC;
const P=(r,p)=>b.slice(r*REC+p*PL,r*REC+(p+1)*PL);
const T=r=>b.slice(r*REC+6*PL,(r+1)*REC);
const used=[];for(let r=0;r<N;r++){let nc=false;for(let p=0;p<6&&!nc;p++){const d=P(r,p);for(let i=1;i<PL;i++)if(d[i]!==d[0]){nc=true;break;}}if(nc)used.push(r);}
console.log('slot  land(p5!=0)  n06   n06/land   base-idx-of-tail-water?');
let sl=0,s6=0;
for(const r of used){const p5=P(r,5),t=T(r);
 let land=0;for(let i=0;i<PL;i++)if(p5[i])land++;
 let n6=0;for(const v of t)if(v===6)n6++;
 sl+=land;s6+=n6;
 console.log(String(r).padStart(4),String(land).padStart(8),String(n6).padStart(6),(n6/land).toFixed(3));}
console.log('TOTAL land',sl,'n06',s6, (s6/sl).toFixed(3));
