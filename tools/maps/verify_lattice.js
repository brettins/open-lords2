const fs=require('fs');
const L=fs.readFileSync('E:/dev/lords2/tools/maps/out/lattice.bin');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const STRIDE=0x104,W=65,H=129,REC=32961,PL=4096;
const tail=slot=>b.slice(slot*REC+6*PL,(slot+1)*REC);
// 1. forward mapping check
let ok=0,bad=[];
for(let y=0;y<64;y++)for(let x=0;x<64;x++){
  const row=x+y+1, col=(x-y+64)>>1;
  const v=L.readUInt32LE(row*STRIDE+col*4);
  if(v===(y*64+x)*8) ok++; else if(bad.length<10) bad.push({x,y,row,col,v:'0x'+v.toString(16),want:'0x'+((y*64+x)*8).toString(16)});
}
console.log('forward map row=x+y+1, col=(x-y+64)>>1 :',ok+'/4096 exact', bad.length?JSON.stringify(bad):'');
// 2. reverse: which cells are background, and do they match the file tail of slot 0?
const t0=tail(0);
let bgTot=0,bgMatch=0,covTot=0,covMatch=0;
const covered=new Set();
for(let y=0;y<64;y++)for(let x=0;x<64;x++) covered.add((x+y+1)*W+((x-y+64)>>1));
for(let r=0;r<H;r++)for(let c=0;c<W;c++){
  const v=L.readUInt32LE(r*STRIDE+c*4);
  const fileByte=t0[r*W+c];
  const isCov=covered.has(r*W+c);
  if(v>=0x0fff0000){ bgTot++; if((v-0x0fff0000)===fileByte) bgMatch++; }
  else { covTot++; }
  if(isCov!==(v<0x0fff0000)) { /* mismatch of cover prediction */ covMatch++; }
}
console.log('background cells:',bgTot,' value == file tail byte:',bgMatch);
console.log('covered cells:',covTot,' cover-set prediction mismatches:',covMatch);
// 3. what the file tail says under covered vs uncovered
let c06=0,c16=0,u06=0,u16=0;
for(let r=0;r<H;r++)for(let c=0;c<W;c++){const f=t0[r*W+c];
  if(covered.has(r*W+c)){ f===6?c06++:c16++; } else { f===6?u06++:u16++; }}
console.log('file tail under covered cells: 0x06='+c06+' 0x16='+c16);
console.log('file tail under uncovered   : 0x06='+u06+' 0x16='+u16);
