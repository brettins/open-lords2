const zlib=require('zlib');
function crc32(buf){let c,t=[];for(let n=0;n<256;n++){c=n;for(let k=0;k<8;k++)c=c&1?0xEDB88320^(c>>>1):c>>>1;t[n]=c>>>0;}
 let x=0xFFFFFFFF;for(const b of buf)x=t[(x^b)&0xff]^(x>>>8);return (x^0xFFFFFFFF)>>>0;}
function chunk(type,data){const len=Buffer.alloc(4);len.writeUInt32BE(data.length);const td=Buffer.concat([Buffer.from(type,'ascii'),data]);
 const c=Buffer.alloc(4);c.writeUInt32BE(crc32(td));return Buffer.concat([len,td,c]);}
// rgb: Uint8Array w*h*3
function png(w,h,rgb){const ihdr=Buffer.alloc(13);ihdr.writeUInt32BE(w,0);ihdr.writeUInt32BE(h,4);ihdr[8]=8;ihdr[9]=2;
 const raw=Buffer.alloc(h*(w*3+1));for(let y=0;y<h;y++){raw[y*(w*3+1)]=0;for(let x=0;x<w*3;x++)raw[y*(w*3+1)+1+x]=rgb[y*w*3+x];}
 return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',ihdr),chunk('IDAT',zlib.deflateSync(raw)),chunk('IEND',Buffer.alloc(0))]);}
module.exports={png};
