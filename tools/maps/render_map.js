// Render one L2_maps.dat slot with the real isometric tile sets. File-only.
// usage: node render_map.js <slot> [season a|b|c|d] [scale]
const fs=require('fs');
const {png}=require('E:/dev/lords2/tools/maps/png.js');
const {open,frame,pal}=require('E:/dev/lords2/tools/maps/pl8.js');
const D='F:/games/Lords of the Realm II/';
const slot=+(process.argv[2]||0), S=(process.argv[3]||'a');
const b=fs.readFileSync(D+'L2_maps.dat');
const REC=32961,PL=4096,LW=65,LH=129;
const P=p=>b.slice(slot*REC+p*PL,slot*REC+(p+1)*PL);
const p0=P(0),p1=P(1),p2=P(2),p5=P(5);
const tail=b.slice(slot*REC+6*PL,(slot+1)*REC);
// bank -> tile set, in the order of the resource table at Lords2.exe:0x004DA050
const BANKS={0:'Base1'+S,4:'Mtns1'+S,8:'Roads1'+S,12:'Town1'+S,16:'Castle1'+S};
const sets={};for(const k of Object.keys(BANKS)) sets[k]=open(D+BANKS[k]+'.pl8');
const PAL=pal(D+'Base1a.256');
const TW=58,TH=30,HW=29,HH=15;
const W=LW*TW, H=LH*HH+TH;
const rgb=new Uint8Array(W*H*3);
// paint the whole canvas with the palette's colour 0
for(let i=0;i<W*H;i++){rgb[i*3]=0;rgb[i*3+1]=0;rgb[i*3+2]=0;}
function blit(set,idx,sx,sy){
  if(idx>=set.n) return false;
  const f=frame(set,idx);
  // frame is width x (height+rows); the diamond occupies the bottom `height` rows
  const oy=sy-(f.h-TH);
  for(let y=0;y<f.h;y++)for(let x=0;x<f.w;x++){const v=f.px[y*f.w+x];if(!v)continue;
    const px=sx+x,py=oy+y;if(px<0||py<0||px>=W||py>=H)continue;
    const o=(py*W+px)*3;rgb[o]=PAL[v*3];rgb[o+1]=PAL[v*3+1];rgb[o+2]=PAL[v*3+2];}
  return true;
}
// covered cells -> tile index
const cover=new Map();
for(let y=0;y<64;y++)for(let x=0;x<64;x++) cover.set((x+y+1)*LW+((x-y+64)>>1), y*64+x);
let drawn=0,missing=0;
for(let row=0;row<LH;row++)for(let col=0;col<LW;col++){
  const sx=col*TW + ((row&1)?0:HW) - HW, sy=row*HH;
  const key=row*LW+col;
  if(cover.has(key)){const i=cover.get(key);
    const set=sets[p1[i]]; if(!set){missing++;continue;}
    if(blit(set,p2[i],sx,sy))drawn++;else missing++;
  }else{
    blit(sets[0],tail[key],sx,sy);
  }
}
console.log('tiles drawn',drawn,'missing',missing);
const out='E:/dev/lords2/tools/maps/out/render_slot'+slot+'_'+S+'.png';
fs.mkdirSync('E:/dev/lords2/tools/maps/out',{recursive:true});
fs.writeFileSync(out,png(W,H,rgb));console.log('wrote',out,W+'x'+H);
