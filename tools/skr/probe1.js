const fs=require("fs");
const b=fs.readFileSync(process.argv[2]||"F:/games/Lords of the Realm II/USER.SKR");
console.log("size",b.length);
// find all offsets where an ASCII printable run of >=3 starts after a NUL/BOF
let runs=[];
let i=0;
while(i<b.length){
  if(b[i]>=32&&b[i]<127){
    let j=i; while(j<b.length&&b[j]>=32&&b[j]<127) j++;
    if(j-i>=4) runs.push([i,j-i,b.toString("latin1",i,j)]);
    i=j;
  } else i++;
}
console.log("printable runs>=4:",runs.length);
for(const r of runs.slice(0,80)) console.log("0x"+r[0].toString(16), r[1], JSON.stringify(r[2]));
console.log("...last:");
for(const r of runs.slice(-10)) console.log("0x"+r[0].toString(16), r[1], JSON.stringify(r[2]));
