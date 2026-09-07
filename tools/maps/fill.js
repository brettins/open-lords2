const fs=require('fs');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
// find longest constant runs
let runs=[];let s=0;
for(let i=1;i<=b.length;i++){ if(i===b.length||b[i]!==b[s]){ if(i-s>=4096) runs.push([s,i,b[s],i-s]); s=i; } }
console.log('constant runs >=4096:');
for(const [a,c,v,l] of runs) console.log('  0x'+a.toString(16)+' .. 0x'+c.toString(16), 'val=0x'+v.toString(16), 'len',l, `(${a}..${c})`);
const H=1318440;
console.log('\nhalf boundary', H, '= 0x'+H.toString(16));
for(const [a,c,v,l] of runs){ console.log('  run start rel to half:', a<H? 'H1 +'+a : 'H2 +'+(a-H), ' end rel:', c<=H? 'H1 +'+c : 'H2 +'+(c-H), ' H1end-start='+(H-a)); }
