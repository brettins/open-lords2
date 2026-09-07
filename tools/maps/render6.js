const fs=require('fs');const {png}=require('E:/dev/lords2/tools/maps/png.js');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961;
function col(v){ if(v===0) return [0,0,0]; const h=(v*47)%360;const c=0.75,x=c*(1-Math.abs((h/60)%2-1)),m=0.55-c/2;
  const t=[[c,x,0],[x,c,0],[0,c,x],[0,x,c],[x,0,c],[c,0,x]][Math.floor(h/60)%6];
  return [Math.round((t[0]+m)*255),Math.round((t[1]+m)*255),Math.round((t[2]+m)*255)];}
function render(data,w,h,scale,file,mode){
  const W=w*scale,H=h*scale;const rgb=new Uint8Array(W*H*3);let mx=0;for(const v of data)if(v>mx)mx=v;
  for(let y=0;y<H;y++)for(let x=0;x<W;x++){const v=data[Math.floor(y/scale)*w+Math.floor(x/scale)];
    const g=Math.round(255*v/(mx||1));const c= mode==='gray'?[g,g,g]:col(v);
    const o=(y*W+x)*3;rgb[o]=c[0];rgb[o+1]=c[1];rgb[o+2]=c[2];}
  fs.writeFileSync(file,png(W,H,rgb));}
const rec=+(process.argv[2]||0), o=rec*REC, OUT='E:/dev/lords2/tools/maps/out';
fs.mkdirSync(OUT,{recursive:true});
// 6 planes 64x64 side by side in one contact sheet, 2 rows x 3 cols, scale 3
const S=3, PW=64*S, PH=64*S, GAP=8;
const W=3*PW+4*GAP, H=2*PH+3*GAP; const rgb=new Uint8Array(W*H*3).fill(40);
for(let p=0;p<6;p++){
  const d=b.slice(o+p*4096,o+p*4096+4096); let mx=0;for(const v of d)if(v>mx)mx=v;
  const cx=GAP+(p%3)*(PW+GAP), cy=GAP+Math.floor(p/3)*(PH+GAP);
  for(let y=0;y<PH;y++)for(let x=0;x<PW;x++){const v=d[Math.floor(y/S)*64+Math.floor(x/S)];const c=col(v);
    const oo=((cy+y)*W+cx+x)*3;rgb[oo]=c[0];rgb[oo+1]=c[1];rgb[oo+2]=c[2];}
}
fs.writeFileSync(`${OUT}/r${rec}_6planes.png`,png(W,H,rgb));
console.log('wrote 6planes contact sheet',W+'x'+H);
// individual grayscale too
for(let p=0;p<6;p++) render(b.slice(o+p*4096,o+p*4096+4096),64,64,6,`${OUT}/r${rec}_p${p}.png`,'gray');
render(b.slice(o+24576,o+24576+8385),65,129,4,`${OUT}/r${rec}_tail65.png`,'idx');
render(b.slice(o+24576,o+24576+8385-1),129,65,4,`${OUT}/r${rec}_tail129.png`,'idx');
console.log('done');
