const fs=require('fs');const {png}=require('/e/dev/lords2/tools/maps/png.js'.replace(/^\/e/,'E:'));
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961;
// distinct-color palette
function col(v){ if(v===0) return [0,0,0]; const h=(v*47)%360, s=0.75, l=0.55;
  const c=(1-Math.abs(2*l-1))*s, x=c*(1-Math.abs((h/60)%2-1)), m=l-c/2; let r,g,bb;
  const i=Math.floor(h/60)%6; [[c,x,0],[x,c,0],[0,c,x],[0,x,c],[x,0,c],[c,0,x]][i].forEach(()=>{});
  const t=[[c,x,0],[x,c,0],[0,c,x],[0,x,c],[x,0,c],[c,0,x]][i]; return [Math.round((t[0]+m)*255),Math.round((t[1]+m)*255),Math.round((t[2]+m)*255)];}
function render(data,w,h,scale,file,mode){
  const W=w*scale,H=h*scale;const rgb=new Uint8Array(W*H*3);
  let mx=0;for(const v of data) if(v>mx)mx=v;
  for(let y=0;y<H;y++)for(let x=0;x<W;x++){
    const v=data[Math.floor(y/scale)*w+Math.floor(x/scale)];
    let c = mode==='gray' ? [Math.round(255*v/(mx||1)),Math.round(255*v/(mx||1)),Math.round(255*v/(mx||1))] : col(v);
    const o=(y*W+x)*3;rgb[o]=c[0];rgb[o+1]=c[1];rgb[o+2]=c[2];}
  fs.writeFileSync(file,png(W,H,rgb));console.log('wrote',file,W+'x'+H,'max='+mx);
}
const rec=+(process.argv[2]||0), o=rec*REC;
const OUT='E:/dev/lords2/tools/maps/out';fs.mkdirSync(OUT,{recursive:true});
const P=[[0,'A'],[8192,'B'],[16384,'C']];
for(const [off,name] of P){ render(b.slice(o+off,o+off+8192),64,128,4,`${OUT}/r${rec}_plane${name}_64x128.png`, name==='C'?'gray':'idx'); }
render(b.slice(o+24576,o+24576+8385),65,129,4,`${OUT}/r${rec}_tail_65x129.png`,'idx');
