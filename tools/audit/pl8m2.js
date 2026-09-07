const fs=require('fs'),path=require('path');const D=process.argv[2];
const all=fs.readdirSync(D).filter(f=>/\.pl8$/i.test(f)).sort();
const sh={},h6=[];
for(const f of all){const b=fs.readFileSync(path.join(D,f));if(b[6]!==0)h6.push([f,b[6]]);if(b[0]!==2)continue;
 const n=b.readUInt16LE(2);for(let i=0;i<n;i++)sh[b[8+i*16+12]]=(sh[b[8+i*16+12]]||0)+1;}
console.log('mode-2 shape histogram',sh, 'total', Object.values(sh).reduce((a,c)=>a+c,0));
console.log('files with header byte 6 != 0:',h6);
