const fs=require('fs');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961,PL=4096,W=65,H=129;
const T=r=>b.slice(r*REC+6*PL,(r+1)*REC);
const slot=+(process.argv[2]||24);
const t=T(slot);
console.log('slot',slot,'tail 65x129 ("." = 0x06, "#" = 0x16, other = hex)');
for(let r=0;r<H;r++){let s='';for(let c=0;c<W;c++){const v=t[r*W+c];s+= v===6?'.':v===0x16?'#':v.toString(16);}
  console.log(String(r).padStart(3),s);}
const cnt={};for(const v of t)cnt[v]=(cnt[v]||0)+1;console.log('counts',cnt);
