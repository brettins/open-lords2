const fs=require('fs');const SLOT=6*4096+65*129;
const w=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const slot=i=>w.subarray(i*SLOT,(i+1)*SLOT);const plane=(s,p)=>s.subarray(p*4096,(p+1)*4096);
const U=[...Array(24).keys()].concat([...Array(20).keys()].map(i=>i+40));
const cc=[],c32=[];
for(const i of U){const P5=plane(slot(i),5);const s=new Set();let n32=0;
 for(const v of P5){if(v>=1&&v<=16)s.add(v);if(v===32)n32++;}
 cc.push(s.size);c32.push(n32);}
console.log('counties per map: min',Math.min(...cc),'max',Math.max(...cc),'sum',cc.reduce((a,b)=>a+b,0));
console.log('  distribution:',cc.slice().sort((a,b)=>a-b).join(','));
console.log('county-32 tiles per map: min',Math.min(...c32),'max',Math.max(...c32),'maps with any:',c32.filter(x=>x).length);
