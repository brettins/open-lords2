const fs=require('fs');
const b=fs.readFileSync(process.argv[2]);
const min=+(process.argv[3]||4);let out=[],cur=[],start=0;
for(let i=0;i<b.length;i++){const c=b[i];
 if(c>=0x20&&c<0x7f){if(!cur.length)start=i;cur.push(c);}
 else{if(cur.length>=min)out.push(start.toString(16).padStart(6,'0')+'  '+Buffer.from(cur).toString('latin1'));cur=[];}}
if(cur.length>=min)out.push(start.toString(16)+'  '+Buffer.from(cur).toString('latin1'));
fs.writeFileSync(process.argv[4]||'strings.txt',out.join('\n'));
console.log(out.length,'strings');
