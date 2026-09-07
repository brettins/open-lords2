// PL8 mode-2 / raw decoder per docs/formats/pl8-mode2.md section 7.
const fs=require('fs');
function open(path){
  const b=fs.readFileSync(path);
  const mode=b[0],zoom=b[1],n=b.readUInt16LE(2),recs=[];
  for(let i=0;i<n;i++){const o=8+i*16;
    recs.push({i,w:b.readUInt16LE(o),h:b.readUInt16LE(o+2),off:b.readUInt32LE(o+4),
      X:b.readInt16LE(o+8),Y:b.readInt16LE(o+10),shape:b[o+12],rows:b[o+13]});}
  return {b,mode,zoom,n,recs};
}
// returns {w,h,px} where px is Uint8Array of palette indices, 0 = transparent
function frame(f,i){
  const r=f.recs[i],b=f.b,w=r.w,h=r.h,shape=r.shape;
  const rows=(shape>=2)?r.rows:0, hh=h>>1;
  let p=r.off;
  if(shape===0){const px=new Uint8Array(w*h);b.copy(px,0,p,p+w*h);return {w,h,px,rows:0};}
  const H=h+rows,px=new Uint8Array(w*H);
  for(let rr=0;rr<h;rr++){const rw=(rr<hh)?2+4*rr:2+4*(h-1-rr),x0=(w-rw)>>1;
    for(let x=0;x<rw;x++){px[(rows+rr)*w+x0+x]=b[p++];}}
  const recLen=(shape===2)?w:h;
  for(let i2=0;i2<rows;i2++){const base=p;p+=recLen;
    let mlo,mhi;
    if(shape===2){mlo=0;mhi=(w>>1)-1;} else if(shape===3){mlo=0;mhi=hh-1;} else {mlo=hh-1;mhi=2*hh-2;}
    let k=0;
    for(let m=mlo;m<=mhi;m++){const row=Math.abs((hh-1)-m),cy=rows+row-(i2+1);
      for(let j=0;j<2;j++){const v=b[base+2*k+j];
        if(v!==0&&cy>=0&&cy<H&&2*m+j<w) px[cy*w+2*m+j]=v;}
      k++;}}
  return {w,h:H,px,rows};
}
function pal(path){const p=fs.readFileSync(path),o=new Uint8Array(768);
  for(let i=0;i<768;i++)o[i]=Math.min(255,Math.round(p[i]*255/63));return o;}
module.exports={open,frame,pal};
