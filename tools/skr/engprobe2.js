const fs=require("fs");
for(const p of process.argv.slice(2)){
  const b=fs.readFileSync(p);
  const first=b.readUInt32LE(12), n=(first-12)/4;
  const off=[];for(let i=0;i<n;i++)off.push(b.readUInt32LE(12+i*4));
  const groups=[];let bad=0,total=0;
  for(let g=0;g<n;g++){
    const s=off[g], e=(g+1<n)?off[g+1]:b.length;
    if(e<s){bad++;groups.push([]);continue;}
    const strs=[];let i=s;
    while(i<e){let j=i;while(j<e&&b[j]!==0)j++;
      if(j>=e){ // no terminator inside region
        strs.push(b.toString("latin1",i,e)); bad++; break;}
      strs.push(b.toString("latin1",i,j)); i=j+1;}
    groups.push(strs); total+=strs.length;
  }
  console.log("=== "+p+" groups="+n+" strings="+total+" unterminated-regions="+bad);
  const sizes={};groups.forEach(g=>sizes[g.length]=(sizes[g.length]||0)+1);
  console.log(" group-size histogram:",JSON.stringify(sizes));
  console.log(" group 41 (0x29):",JSON.stringify(groups[41]));
  console.log(" group 0:",JSON.stringify(groups[0]));
  console.log(" group 1:",JSON.stringify(groups[1]));
  console.log(" last group ["+(n-1)+"] count="+groups[n-1].length, JSON.stringify(groups[n-1].slice(0,8)));
  // empty groups
  const empt=[];groups.forEach((g,i)=>{if(g.length===0)empt.push(i)});
  console.log(" empty groups ("+empt.length+"):",empt.join(","));
  fs.writeFileSync(p.split("/").pop()+".groups.json",JSON.stringify(groups,null,0));
}
