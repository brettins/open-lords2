// Dump L2.eng group 101 (campaign map names) next to the L2_maps.dat slot census.
const fs=require('fs');
const D=(process.argv[2]||'F:/games/Lords of the Realm II').replace(/[\/]+$/,'')+'/';
const e=fs.readFileSync(D+'L2.eng');
const off=g=>e[8+g*4]|(e[9+g*4]<<8)|(e[10+g*4]<<16);
const N=(off(1)-8)/4;
const g=+(process.argv[3]||101);
const start=off(g),end=off(g+1);
const out=[];let i=start;
while(i<end){let j=i;while(j<end&&e[j]!==0)j++;out.push(e.toString('latin1',i,j));i=j+1;}
// slot census from L2_maps.dat
const b=fs.readFileSync(D+'L2_maps.dat');const REC=32961,PL=4096,NS=b.length/REC;
const P=(r,p)=>b.slice(r*REC+p*PL,r*REC+(p+1)*PL);
const usedSet=new Set();
for(let r=0;r<NS;r++){let nc=false;for(let p=0;p<6&&!nc;p++){const d=P(r,p);for(let k=1;k<PL;k++)if(d[k]!==d[0]){nc=true;break;}}if(nc)usedSet.add(r);}
console.log('L2.eng groups 1..'+(N-1)+'; group '+g+' has '+out.length+' strings; L2_maps.dat has '+NS+' slots, '+usedSet.size+' used');
out.forEach((s,k)=>{
  const cty=usedSet.has(k)?(()=>{const p5=P(k,5);const c=new Set();for(const v of p5)if(v>0&&v<=16)c.add(v);return c.size;})():'-';
  console.log('  slot '+String(k).padStart(2)+'  '+(usedSet.has(k)?'USED ':'empty')+'  counties='+String(cty).padStart(2)+'  '+JSON.stringify(s));});
