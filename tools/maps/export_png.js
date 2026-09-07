// Export a map slot as PNG. Usage: node export_png.js <slot> [outdir]
const fs=require('fs');const {png}=require('E:/dev/lords2/tools/maps/png.js');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961,PLANE=4096;
const P=(r,p)=>b.slice(r*REC+p*PLANE,r*REC+(p+1)*PLANE);
const CTY=[[70,110,180],[200,60,60],[60,160,90],[210,150,50],[150,90,190],[80,180,190],[220,120,170],[130,150,60],
           [180,80,40],[90,120,210],[110,190,120],[200,190,90],[160,60,120],[70,150,160],[210,110,80],[120,110,190],[95,175,205]];
function composite(r,S){
  const p0=P(r,0),p2=P(r,2),p5=P(r,5);
  const W=64*S,H=64*S,rgb=new Uint8Array(W*H*3);
  for(let y=0;y<H;y++)for(let x=0;x<W;x++){
    const tx=(x/S)|0, ty=(y/S)|0, i=ty*64+tx; const f=p0[i], c=p5[i];
    let col;
    if(c===0) col=[26,42,68];                       // no county (plane0 & 0x04) -> sea
    else if(c===32) col=[60,80,60];                 // county id 32
    else { col=CTY[(c-1)%CTY.length].slice(); }
    const sx=x%S, sy=y%S;
    if(f&0x40){ col=[20,20,20]; if(sx>0&&sy>0&&sx<S-1&&sy<S-1) col=[245,245,245]; }   // castle 2x2
    else if(f&0x80){ if(sx>=1&&sy>=1&&sx<=S-2&&sy<=S-2) col=[250,230,120]; }          // settlement
    else if(f&0x20){ if(sx===((S/2)|0)&&sy===((S/2)|0)) col=[255,255,255]; }           // dwelling
    else if(f&0x08){ col=col.map(v=>Math.round(v*0.65)); }                             // multi-tile object
    const o=(y*W+x)*3;rgb[o]=col[0];rgb[o+1]=col[1];rgb[o+2]=col[2];
  }
  return {rgb,W,H};
}
const OUT=process.argv[3]||'E:/dev/lords2/tools/maps/out';fs.mkdirSync(OUT,{recursive:true});
if(process.argv[2]==='all'){
  const REAL=[...Array(24).keys(),...Array(20).keys().map(x=>x+40)];
  const S=3,PW=64*S,G=6,cols=8,rows=Math.ceil(REAL.length/cols);
  const W=cols*PW+(cols+1)*G,H=rows*PW+(rows+1)*G,rgb=new Uint8Array(W*H*3).fill(30);
  REAL.forEach((r,k)=>{const c=composite(r,S);const cx=G+(k%cols)*(PW+G),cy=G+((k/cols)|0)*(PW+G);
    for(let y=0;y<PW;y++)for(let x=0;x<PW;x++){const s=(y*PW+x)*3,d=((cy+y)*W+cx+x)*3;
      rgb[d]=c.rgb[s];rgb[d+1]=c.rgb[s+1];rgb[d+2]=c.rgb[s+2];}});
  fs.writeFileSync(OUT+'/all_maps.png',png(W,H,rgb));console.log('wrote all_maps.png',W+'x'+H);
}else{
  const r=+(process.argv[2]||0),c=composite(r,8);
  fs.writeFileSync(`${OUT}/map${r}.png`,png(c.W,c.H,c.rgb));console.log('wrote map'+r+'.png');
}
