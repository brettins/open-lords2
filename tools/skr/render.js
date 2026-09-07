const fs=require("fs");
const b=fs.readFileSync("F:/games/Lords of the Realm II/USER.SKR");
const base=5420;
const origin=parseInt(process.argv[2]||"0");
const map=parseInt(process.argv[3]||"0");
const W=80,H=80;
const off=base+origin+map*W*H;
const chars={0:".",2:"#",4:"@",9:"o",10:"T",15:"F",16:"6",18:"8",21:"=",32:" "};
for(let y=0;y<H;y++){
  let s="";
  for(let x=0;x<W;x++){const v=b[off+y*W+x]; s+= (chars[v]!==undefined?chars[v]:"?");}
  console.log(String(y).padStart(2),s);
}
