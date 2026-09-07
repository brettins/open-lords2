const fs=require('fs');
const t=fs.readFileSync('E:/dev/lords2/tools/maps/out/tiles.bin');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961,PL=4096;
const P=p=>b.slice(p*PL,(p+1)*PL);
const p0=P(0),p1=P(1),p2=P(2),p3=P(3),p4=P(4),p5=P(5);
const show=(label,pred)=>{const rows=[];
 for(let i=0;i<PL;i++) if(pred(i)) rows.push({i,x:i%64,y:(i/64)|0,
   f:'0x'+p0[i].toString(16),rf:'0x'+t[i*8+1].toString(16),
   bank:'0x'+p1[i].toString(16)+'->0x'+(t[i*8+2]&0x1c).toString(16),
   raw2:'0x'+t[i*8+2].toString(16),
   idx:p2[i]+'->'+t[i*8+3], p3:p3[i]+'->'+t[i*8+4], p4:p4[i]+'->'+t[i*8+5],
   cty:p5[i]+'/'+t[i*8+7], b0:t[i*8], b6:t[i*8+6]});
 console.log('### '+label+'  n='+rows.length);
 rows.slice(0,40).forEach(r=>console.log('  ',JSON.stringify(r)));};
show('bank 0x00 -> 0x10 (settlement)', i=>p1[i]===0&&(t[i*8+2]&0x1c)===0x10);
show('bank 0x00 -> 0x0c', i=>p1[i]===0&&(t[i*8+2]&0x1c)===0x0c);
show('castle tiles with index change', i=>p1[i]===0x0c&&p2[i]!==t[i*8+3]);
show('flags 0x20 (index change) first 20', i=>(p0[i]&0x20)&&p2[i]!==t[i*8+3]);
show('flags 0x01 exact, plane4 written', i=>p0[i]===1&&t[i*8+5]!==0);
