// Test the isometric tile->lattice mapping over all used slots.
const fs=require('fs');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961,PL=4096,LW=65,LH=129,N=b.length/REC;
const P=(r,p)=>b.slice(r*REC+p*PL,r*REC+(p+1)*PL);
const T=r=>b.slice(r*REC+6*PL,(r+1)*REC);
const used=[];for(let r=0;r<N;r++){let nc=false;for(let p=0;p<6&&!nc;p++){const d=P(r,p);for(let i=1;i<PL;i++)if(d[i]!==d[0]){nc=true;break;}}if(nc)used.push(r);}
// 8 dihedral variants of (x,y)
const ORI=[
 ['x,y',      (x,y)=>[x,y]],
 ['y,x',      (x,y)=>[y,x]],
 ['63-x,y',   (x,y)=>[63-x,y]],
 ['x,63-y',   (x,y)=>[x,63-y]],
 ['63-x,63-y',(x,y)=>[63-x,63-y]],
 ['y,63-x',   (x,y)=>[y,63-x]],
 ['63-y,x',   (x,y)=>[63-y,x]],
 ['63-y,63-x',(x,y)=>[63-y,63-x]],
];
const LAND=(p0,p5,i)=>p5[i]!==0;      // in a county
let best=[];
for(const [nm,f] of ORI) for(let R0=-4;R0<=6;R0++) for(let C0=-4;C0<=6;C0++){
  let ok=0,tot=0,oob=0;
  for(const r of used){const p0=P(r,0),p5=P(r,5),t=T(r);
    for(let y=0;y<64;y++)for(let x=0;x<64;x++){
      const [u,v]=f(x,y);
      const row=u+v+R0, col=((u-v+64)>>1)+C0;
      if(row<0||row>=LH||col<0||col>=LW){oob++;continue;}
      tot++; const isLand=LAND(p0,p5,y*64+x), is06=t[row*LW+col]===6;
      if(isLand===is06) ok++;
    }}
  best.push({nm,R0,C0,pct:100*ok/tot,tot,oob});
}
best.sort((a,c)=>c.pct-a.pct);
for(const e of best.slice(0,12)) console.log(e.nm.padEnd(11),'R0='+String(e.R0).padStart(2),'C0='+String(e.C0).padStart(2),e.pct.toFixed(4)+'%','n='+e.tot,'oob='+e.oob);
