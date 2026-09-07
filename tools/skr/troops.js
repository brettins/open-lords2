// Parse a TROOPS*.ENG exactly as Lords2.exe FUN_0042ac0c does.
const fs=require("fs");
const TYPES=["Pe","Xb","Ma","Sw","Pi","Ar","Kn","Ca","To","Ra","Oi"];
function parse(p){
  const b=fs.readFileSync(p); const s=b.toString("latin1");
  let i=s.indexOf("*");
  if(i<0) return null;
  const adv=[]; const tab=[]; // tab[row][group][side][type]
  let row=0,group=0,side=0,type=0,seenAdv=false;
  const nums=[];
  while(i<s.length){
    const c=s.charCodeAt(i);
    if(c<0x30||c>0x39){i++;continue;}
    let j=i; while(j<s.length&&s.charCodeAt(j)>=0x30&&s.charCodeAt(j)<=0x39)j++;
    const v=parseInt(s.slice(i,j),10);
    nums.push(v);
    if(type!==0||side!==0||group!==0||seenAdv){
      (tab[row]=tab[row]||[])[group]=(tab[row][group]||[]);
      (tab[row][group][side]=tab[row][group][side]||[])[type]=v;
      type++;
      if(type>10){type=0;side++;if(side>1){side=0;group++;if(group>4){group=0;row++;}}}
      seenAdv=false;
    } else {
      adv[row]=Math.max(0,Math.min(10,v)); seenAdv=true;
    }
    if(row>0x22) break;
    i=j;
  }
  return {file:p,size:b.length,nums:nums.length,rows:row,adv,tab};
}
for(const p of process.argv.slice(2)){
  const r=parse(p);
  console.log("=== "+r.file+" size="+r.size+" numbers-consumed="+r.nums+" rows-completed="+r.rows);
  console.log("  expected numbers for 35 rows = 35*(1+5*2*11) = "+(35*111));
  console.log("  defensive advantage per row:", r.adv.join(","));
  console.log("  row0 group0 side0:", r.tab[0][0][0].join(" "), " side1:", r.tab[0][0][1].join(" "));
  console.log("  row0 group4 side0:", r.tab[0][4][0].join(" "), " side1:", r.tab[0][4][1].join(" "));
  console.log("  row34 group2 side0:", r.tab[34]?r.tab[34][2][0].join(" "):"(missing)");
  // siege values >9 present?
  let over=0,cells=0;
  for(let rr=0;rr<35;rr++)for(let g=0;g<5;g++)for(let sd=0;sd<2;sd++)for(let t=7;t<11;t++){cells++;if((r.tab[rr]&&r.tab[rr][g]&&r.tab[rr][g][sd]?r.tab[rr][g][sd][t]:0)>9)over++;}
  console.log("  siege slots (types 7..10) with value >9: "+over+" of "+cells);
}
