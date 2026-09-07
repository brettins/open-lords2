const fs=require('fs');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961, N=b.length/REC;
console.log('records:',N);
// per-record profile
for(let r=0;r<N;r++){
  const o=r*REC;const s=new Set();let z=0;const hist={};
  for(let i=o;i<o+REC;i++){s.add(b[i]);if(b[i]===0)z++;hist[b[i]]=(hist[b[i]]||0)+1;}
  const top=Object.entries(hist).sort((a,c)=>c[1]-a[1]).slice(0,3).map(([v,c])=>`${(+v).toString(16)}:${(100*c/REC).toFixed(0)}%`).join(' ');
  console.log(String(r).padStart(2),'@0x'+o.toString(16).padStart(6,'0'),'distinct',String(s.size).padStart(3),'zero%',String((100*z/REC).toFixed(0)).padStart(3),' top:',top);
}
