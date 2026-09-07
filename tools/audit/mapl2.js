const fs=require('fs');const b=fs.readFileSync('F:/games/Lords of the Realm II/mapl2.exe');
const pe=b.readUInt32LE(0x3c),nsec=b.readUInt16LE(pe+6),optOff=pe+24,optSize=b.readUInt16LE(pe+20);
const base=b.readUInt32LE(optOff+28);const secs=[];
for(let i=0;i<nsec;i++){const o=optOff+optSize+i*40;secs.push({name:b.subarray(o,o+8).toString().replace(/\0/g,''),va:b.readUInt32LE(o+12),vsize:b.readUInt32LE(o+8),rsize:b.readUInt32LE(o+16),roff:b.readUInt32LE(o+20)});}
console.log('mapl2.exe size',b.length,'ImageBase 0x'+base.toString(16));
const v2o=v=>{const r=v-base;for(const s of secs)if(r>=s.va&&r<s.va+Math.max(s.vsize,s.rsize))return s.roff+(r-s.va);return null;};
let o=v2o(0x00433D88);console.log('offset table at 0x433D88 (file 0x'+o.toString(16)+'):');
const t=[];for(let i=0;i<20;i++)t.push(b.readUInt32LE(o+i*4));
console.log(' ',t.map(v=>'0x'+v.toString(16)).join(', '));
console.log('  first',t[0],'stride',t[1]-t[0],'last+6400',t[19]+6400);
o=v2o(0x00434040);console.log('default army record at 0x434040:',[...Array(11)].map((_,i)=>b.readUInt32LE(o+i*4)).join(','));
// strings
for(const s of ['l2_maps','*.skr','my_maps1.skr','l2map.inf','l2.sg2','L2 Battlemap editor','Peasants','Crossbowmen','Voller Name','Kurzbeschreibung'])
  console.log('  string',JSON.stringify(s)+':',(b.toString('latin1').split(s).length-1));
