const fs=require('fs');
const b=fs.readFileSync(process.env.MAPS||'F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961,PL=4096,W=65,H=129;
const T=r=>b.slice(r*REC+6*PL,(r+1)*REC);
const slot=+(process.argv[2]||24);
const t=T(slot);
const crops=[[0,20,0,16],[0,20,45,65],[100,125,0,20],[100,125,45,65]];
for(const [r0,r1,c0,c1] of crops){
  console.log('--- rows '+r0+'-'+r1+' cols '+c0+'-'+c1);
  for(let r=r0;r<=r1&&r<H;r++){let s='';for(let c=c0;c<c1;c++){const v=t[r*W+c];s+=(v===6?'#':'.');}
    console.log(String(r).padStart(3),s);}
}
