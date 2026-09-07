const fs=require('fs');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961;
const r=+(process.argv[2]||0), o=r*REC, B=512;
console.log('record',r,'fine profile, block',B);
let prevsig='';
for(let x=0;x<REC;x+=B){
  const len=Math.min(B,REC-x);const s=new Set();let mx=0;
  for(let i=o+x;i<o+x+len;i++){s.add(b[i]);if(b[i]>mx)mx=b[i];}
  // signature: is it "bitfield-like" (all values are 0 or single-bit-ish)?
  let bits=0;for(const v of s){ if(v!==0 && (v&(v-1))!==0) bits++; }
  const sig=`d=${String(s.size).padStart(3)} max=${mx.toString(16).padStart(2,'0')} nonpow2=${String(bits).padStart(2)}`;
  console.log(String(x).padStart(6),sig, x===0||true?'':'');
}
