// Render a PL8's frames as a labelled contact sheet. usage: node sheet.js <file.pl8> <pal.256> [cols]
const fs=require('fs'),path=require('path');
const {png}=require('E:/dev/lords2/tools/maps/png.js');
const {open,frame,pal}=require('E:/dev/lords2/tools/maps/pl8.js');
const D='F:/games/Lords of the Realm II/';
const f=open(D+process.argv[2]),P=pal(D+(process.argv[3]||'Base1a.256'));
const cols=+(process.argv[4]||10);
const CW=62,CH=64,G=2;
const rows=Math.ceil(f.n/cols),W=cols*(CW+G)+G,H=rows*(CH+G)+G;
const rgb=new Uint8Array(W*H*3).fill(20);
for(let i=0;i<f.n;i++){const fr=frame(f,i);
  const cx=G+(i%cols)*(CW+G),cy=G+((i/cols)|0)*(CH+G);
  const ox=cx+((CW-fr.w)>>1), oy=cy+(CH-fr.h);
  for(let y=0;y<fr.h;y++)for(let x=0;x<fr.w;x++){const v=fr.px[y*fr.w+x];if(!v)continue;
    const px=ox+x,py=oy+y;if(px<0||py<0||px>=W||py>=H)continue;
    const o=(py*W+px)*3;rgb[o]=P[v*3];rgb[o+1]=P[v*3+1];rgb[o+2]=P[v*3+2];}}
const out='E:/dev/lords2/tools/maps/out/'+path.basename(process.argv[2],'.pl8')+'_sheet.png';
fs.mkdirSync('E:/dev/lords2/tools/maps/out',{recursive:true});
fs.writeFileSync(out,png(W,H,rgb));console.log('wrote',out,W+'x'+H,'frames',f.n);
