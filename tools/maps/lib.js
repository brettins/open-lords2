const fs=require('fs');const {png}=require('E:/dev/lords2/tools/maps/png.js');
const B=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961;
function col(v){ if(v===0) return [0,0,0]; const h=((v*47)%360+360)%360;const c=0.75,x=c*(1-Math.abs((h/60)%2-1)),m=0.55-c/2;
  const t=[[c,x,0],[x,c,0],[0,c,x],[0,x,c],[x,0,c],[c,0,x]][Math.floor(h/60)%6]||[0,0,0];
  return [Math.round((t[0]+m)*255),Math.round((t[1]+m)*255),Math.round((t[2]+m)*255)];}
function gray(v,mx){const g=Math.round(255*v/(mx||1));return [g,g,g];}
function sheet(cells,cw,ch,cols,scale,file,gap=8){
  const rows=Math.ceil(cells.length/cols);
  const PW=cw*scale,PH=ch*scale,W=cols*PW+(cols+1)*gap,H=rows*PH+(rows+1)*gap;
  const rgb=new Uint8Array(W*H*3).fill(35);
  cells.forEach((cell,i)=>{const {data,w,h,mode}=cell;let mx=0;for(const v of data)if(v>mx)mx=v;
    const cx=gap+(i%cols)*(PW+gap),cy=gap+Math.floor(i/cols)*(PH+gap);
    for(let y=0;y<PH;y++)for(let x=0;x<PW;x++){const sx=Math.floor(x/scale),sy=Math.floor(y/scale);
      if(sx>=w||sy>=h)continue;const v=data[sy*w+sx]; if(v===undefined)continue;
      const c= mode==='gray'?gray(v,mx):col(v); const o=((cy+y)*W+cx+x)*3;rgb[o]=c[0];rgb[o+1]=c[1];rgb[o+2]=c[2];}});
  fs.writeFileSync(file,png(W,H,rgb));console.log('wrote',file,W+'x'+H);
}
function plane(rec,p){return B.slice(rec*REC+p*4096, rec*REC+(p+1)*4096);}
function tail(rec){return B.slice(rec*REC+24576, rec*REC+32961);}
module.exports={B,REC,col,gray,sheet,plane,tail,png};
