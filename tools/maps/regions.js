const fs=require('fs');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
// classify each 1024-byte block by distinct byte count
const B=1024;let prev=null,start=0;
const rows=[];
for(let o=0;o<b.length;o+=B){
  const s=new Set();for(let i=o;i<Math.min(o+B,b.length);i++)s.add(b[i]);
  const cls = s.size<=6 ? 'SPARSE' : 'RICH';
  if(cls!==prev){ if(prev!==null) rows.push([start,o,prev]); start=o; prev=cls; }
}
rows.push([start,b.length,prev]);
for(const [a,c,k] of rows) console.log(k.padEnd(6), '0x'+a.toString(16).padStart(6,'0'), '..', '0x'+c.toString(16).padStart(6,'0'), 'len',c-a, '('+a+'..'+c+')');
