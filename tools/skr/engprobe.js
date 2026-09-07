const fs=require("fs");
for(const p of process.argv.slice(2)){
  const b=fs.readFileSync(p);
  console.log("=== "+p+"  size="+b.length);
  const magic=b.toString("latin1",0,8);
  const w8=b.readUInt32LE(8);
  const first=b.readUInt32LE(12);
  const n=(first-12)/4;
  console.log("magic="+JSON.stringify(magic)+" dword@8="+w8+" first="+first+" derivedCount="+n);
  if(!Number.isInteger(n)){console.log("  NOT INTEGER"); continue;}
  const off=[];for(let i=0;i<n;i++)off.push(b.readUInt32LE(12+i*4));
  let nonmono=0,oob=0,dup=0;
  for(let i=1;i<n;i++){if(off[i]<off[i-1])nonmono++;if(off[i]===off[i-1])dup++;}
  for(const o of off) if(o>b.length) oob++;
  console.log("  entries="+n+" nonmonotonic="+nonmono+" duplicates="+dup+" out-of-range="+oob);
  console.log("  last offset="+off[n-1]+"  file size="+b.length);
  // decode strings: from off[i] to next NUL
  let terminatorNul=0, term=0;
  const strs=[];
  for(let i=0;i<n;i++){
    const s=off[i]; let e=s; while(e<b.length&&b[e]!==0)e++;
    strs.push(b.toString("latin1",s,e));
    if(e<b.length&&b[e]===0)terminatorNul++;
    // check that e+1 == off[i+1] where offsets differ
  }
  // contiguity check
  let contig=0, gapHist={};
  for(let i=0;i<n-1;i++){
    if(off[i+1]===off[i])continue;
    const end=off[i]+strs[i].length+1;
    const gap=off[i+1]-end;
    gapHist[gap]=(gapHist[gap]||0)+1;
    if(gap===0)contig++;
  }
  console.log("  NUL-terminated="+terminatorNul+"/"+n+"  gaps histogram:",JSON.stringify(gapHist));
  const lastEnd=off[n-1]+strs[n-1].length+1;
  console.log("  end of last string="+lastEnd+" (file "+b.length+", diff "+(b.length-lastEnd)+")");
  console.log("  first 12 strings:");
  for(let i=0;i<12;i++)console.log("   ["+i+"] @"+off[i]+" "+JSON.stringify(strs[i].slice(0,80)));
  console.log("  last 6 strings:");
  for(let i=n-6;i<n;i++)console.log("   ["+i+"] @"+off[i]+" "+JSON.stringify(strs[i].slice(0,80)));
}
