const fs=require('fs');const p=process.argv[2];const b=fs.readFileSync(p);
const pe=b.readUInt32LE(0x3c);
const nsec=b.readUInt16LE(pe+6), optOff=pe+24, optSize=b.readUInt16LE(pe+20);
console.log(p,'size',b.length,'PE at 0x'+pe.toString(16),'sections',nsec);
console.log('ImageBase 0x'+b.readUInt32LE(optOff+28).toString(16),'DllCharacteristics 0x'+b.readUInt16LE(optOff+70).toString(16),
  '(DYNAMIC_BASE='+((b.readUInt16LE(optOff+70)&0x40)?'set':'clear')+')');
const secs=[];
for(let i=0;i<nsec;i++){const o=optOff+optSize+i*40;
 const s={name:b.subarray(o,o+8).toString().replace(/\0/g,''),vsize:b.readUInt32LE(o+8),va:b.readUInt32LE(o+12),rsize:b.readUInt32LE(o+16),roff:b.readUInt32LE(o+20)};
 secs.push(s);console.log(' ',s.name,'VA 0x'+(0x400000+s.va).toString(16),'vsize',s.vsize,'raw',s.rsize,'off 0x'+s.roff.toString(16),'end VA 0x'+(0x400000+s.va+s.vsize).toString(16));}
const v2o=v=>{const r=v-0x400000;for(const s of secs)if(r>=s.va&&r<s.va+Math.max(s.vsize,s.rsize))return s.roff+(r-s.va);return null;};
// imports
const impRVA=b.readUInt32LE(optOff+96+8), impSize=b.readUInt32LE(optOff+96+12);
let o=v2o(0x400000+impRVA);
console.log('\nimports:');
while(o&&b.readUInt32LE(o+12)){const nameO=v2o(0x400000+b.readUInt32LE(o+12));const dll=b.subarray(nameO,b.indexOf(0,nameO)).toString();
 let t=v2o(0x400000+(b.readUInt32LE(o)||b.readUInt32LE(o+16)));const fns=[];
 while(true){const e=b.readUInt32LE(t);if(!e)break;
  if(e&0x80000000)fns.push('ord#'+(e&0xffff));else{const n=v2o(0x400000+e)+2;fns.push(b.subarray(n,b.indexOf(0,n)).toString());}t+=4;}
 console.log(' ',dll,fns.length,fns.length<=8?fns.join(','):fns.slice(0,6).join(',')+',...');
 o+=20;}
// resource table dump
if(process.argv[3]){const va=parseInt(process.argv[3],16);let q=v2o(va);
 console.log('\nresource table at 0x'+va.toString(16),'(file 0x'+q.toString(16)+'):');
 for(let i=0;i<Number(process.argv[4]||40);i++){const n=b.subarray(q+i*20,q+i*20+16).toString('latin1').replace(/\0.*/,'');
  console.log('  ',i,JSON.stringify(n),b.readUInt32LE(q+i*20+16));}}
