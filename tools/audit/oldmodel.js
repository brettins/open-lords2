const fs=require('fs'),path=require('path');const D='F:/games/Lords of the Realm II';
const files=fs.readdirSync(D).filter(f=>/\.pl8$/i.test(f)).sort();
function rle(b,p,w,rows){for(let y=0;y<rows;y++){let x=0;while(x<w){if(p>=b.length)return null;const op=b[p++];if(op===0){const s=b[p++];if(!s)return null;x+=s;}else{p+=op;x+=op;}}if(x!==w)return null;}return p;}
const fam0WithIso=[], failOld=[], frames={};
let framesInFailing=0, totalFrames=0;
for(const f of files){const b=fs.readFileSync(path.join(D,f));const fam=b[0],n=b.readUInt16LE(2);totalFrames+=n;
 const r=[];for(let i=0;i<n;i++){const o=8+i*16;r.push({w:b.readUInt16LE(o),h:b.readUInt16LE(o+2),off:b.readUInt32LE(o+4),sh:b[o+12],rows:b[o+13]});}
 const hasIso=r.some(x=>x.sh>=1&&x.sh<=4);
 if(fam!==2&&hasIso)fam0WithIso.push(f+'(fam'+fam+',n='+n+')');
 // OLD model: dispatch on family byte; iso only inside family 2; no rect overhang; no Font_c2 fallback
 let ok=true, firstErr=null;
 for(let i=0;i<n;i++){const x=r[i];const nx=i+1<n?r[i+1].off:b.length;let end=null;
  if(fam===2&&x.sh>=1&&x.sh<=4){let need=x.h*x.h;if(x.sh===2)need+=x.rows*x.w;if(x.sh===3||x.sh===4)need+=x.rows*x.h;end=x.off+need;}
  else if(fam===1){end=rle(b,x.off,x.w,x.h);}
  else {end=x.off+x.w*x.h; if(x.w>=8&&x.h>=8&&nx-x.off===(x.w>>3)*(x.h>>3))end=x.off+(x.w>>3)*(x.h>>3);}
  if(end!==nx){ok=false;if(firstErr===null)firstErr=(end===null?'rle fail':(end-nx));break;}}
 if(!ok){failOld.push([f,n,firstErr]);framesInFailing+=n;}
}
console.log('family != 2 files that contain iso frames:',fam0WithIso.length);console.log(' ',fam0WithIso.join('\n  '));
console.log('\nfiles failing under the OLD (family-byte) model:',failOld.length);
for(const x of failOld)console.log('  ',x[0],'frames',x[1],'first residual',x[2]);
console.log('\ntotal frames',totalFrames,'frames in failing files',framesInFailing,'frames in passing files',totalFrames-framesInFailing);

// how many frames the old validator would have counted, under two counting rules
let framesBeforeFail=0, framesPassingFiles=0;
for(const f of files){const b=fs.readFileSync(path.join(D,f));const fam=b[0],n=b.readUInt16LE(2);
 const r=[];for(let i=0;i<n;i++){const o=8+i*16;r.push({w:b.readUInt16LE(o),h:b.readUInt16LE(o+2),off:b.readUInt32LE(o+4),sh:b[o+12],rows:b[o+13]});}
 let ok=true,cnt=0;
 for(let i=0;i<n;i++){const x=r[i];const nx=i+1<n?r[i+1].off:b.length;let end=null;
  if(fam===2&&x.sh>=1&&x.sh<=4){let need=x.h*x.h;if(x.sh===2)need+=x.rows*x.w;if(x.sh===3||x.sh===4)need+=x.rows*x.h;end=x.off+need;}
  else if(fam===1){end=rle(b,x.off,x.w,x.h);}
  else {end=x.off+x.w*x.h;if(x.w>=8&&x.h>=8&&nx-x.off===(x.w>>3)*(x.h>>3))end=x.off+(x.w>>3)*(x.h>>3);}
  if(end!==nx){ok=false;break;} cnt++;}
 framesBeforeFail+=cnt; if(ok)framesPassingFiles+=n;}
console.log('\nframes in passing files:',framesPassingFiles,'| frames decoded before first failure (all files):',framesBeforeFail);
// no-grid-detection variant (older still): treat grids as w*h
