const fs=require('fs');const D='F:/games/Lords of the Realm II/';
const files=process.argv.slice(2);
for(const f of files){const b=fs.readFileSync(D+f);const n=b.readUInt16LE(2);
 console.log('=== '+f+'  mode='+b[0]+' zoom='+b[1]+' frames='+n);
 let line=[];
 for(let i=0;i<n;i++){const o=8+i*16;
  line.push(String(i).padStart(3)+' X='+String(b.readInt16LE(o+8)).padStart(4)+' Y='+String(b.readInt16LE(o+10)).padStart(4)+' s'+b[o+12]+' r'+String(b[o+13]).padStart(2));
  if(line.length===4){console.log(line.join(' | '));line=[];}}
 if(line.length)console.log(line.join(' | '));
}
