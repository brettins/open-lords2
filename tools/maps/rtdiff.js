const fs=require('fs');
const t=fs.readFileSync('E:/dev/lords2/tools/maps/out/tiles.bin');
const L=fs.readFileSync('E:/dev/lords2/tools/maps/out/lattice.bin');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961,PL=4096,W=65,H=129,STRIDE=0x104;
const P=p=>b.slice(0*REC+p*PL,0*REC+(p+1)*PL);
const p0=P(0),p1=P(1),p2=P(2),p3=P(3),p4=P(4),p5=P(5),tl=b.slice(6*PL,REC);
// bank census file vs runtime
const fb={},rb={};
for(let i=0;i<PL;i++){fb[p1[i]]=(fb[p1[i]]||0)+1;const rbk=t[i*8+2]&0x1c;rb[rbk]=(rb[rbk]||0)+1;}
console.log('bank census  file:',JSON.stringify(fb),' runtime:',JSON.stringify(rb));
// where bank changed
const ch={};
for(let i=0;i<PL;i++){const f=p1[i],r=t[i*8+2]&0x1c;if(f!==r){const k='0x'+f.toString(16)+'->0x'+r.toString(16)+' flags0x'+p0[i].toString(16);ch[k]=(ch[k]||0)+1;}}
console.log('bank changes:',JSON.stringify(ch));
// index changes, grouped by (bank, flags)
const ic={};
for(let i=0;i<PL;i++){const fb1=p1[i],rb1=t[i*8+2]&0x1c;
 if(p2[i]!==t[i*8+3]){const k='bank0x'+fb1.toString(16)+(fb1!==rb1?'->0x'+rb1.toString(16):'')+' flags0x'+p0[i].toString(16);ic[k]=(ic[k]||0)+1;}}
console.log('index changes by class:'); for(const k of Object.keys(ic).sort())console.log('   ',k,ic[k]);
// index change detail for unchanged-bank base tiles
const det={};
for(let i=0;i<PL;i++){if(p1[i]===0&&(t[i*8+2]&0x1c)===0&&p2[i]!==t[i*8+3]){const k=p2[i]+'->'+t[i*8+3];det[k]=(det[k]||0)+1;}}
const dd=Object.entries(det).sort((a,c)=>c[1]-a[1]).slice(0,25);
console.log('base-bank index remaps (top 25):',JSON.stringify(dd));
// plane4 vs byte+5
const p4d={};for(let i=0;i<PL;i++)if(p4[i]!==t[i*8+5]){const k=p4[i]+'->'+t[i*8+5]+' flags0x'+p0[i].toString(16);p4d[k]=(p4d[k]||0)+1;}
console.log('plane4 vs runtime +5 diffs:',JSON.stringify(p4d));
// plane0 vs byte+1
const p0d={};for(let i=0;i<PL;i++)if(p0[i]!==t[i*8+1]){const k='0x'+p0[i].toString(16)+'->0x'+t[i*8+1].toString(16);p0d[k]=(p0d[k]||0)+1;}
console.log('plane0 vs runtime +1 diffs:',JSON.stringify(p0d));
// byte+0 and byte+6 census vs county
const b0={},b6={};for(let i=0;i<PL;i++){b0[t[i*8]]=(b0[t[i*8]]||0)+1;b6[t[i*8+6]]=(b6[t[i*8+6]]||0)+1;}
console.log('runtime +0 census:',JSON.stringify(b0));
console.log('runtime +6 census:',JSON.stringify(b6));
let same=0;for(let i=0;i<PL;i++)if(t[i*8+6]===p5[i])same++;console.log('runtime +6 == county:',same+'/4096');
// background lattice values runtime vs file
const bgr={},bgf={};
const cov=new Set();for(let y=0;y<64;y++)for(let x=0;x<64;x++)cov.add((x+y+1)*W+((x-y+64)>>1));
for(let r=0;r<H;r++)for(let c=0;c<W;c++){if(cov.has(r*W+c))continue;
 const v=L.readUInt32LE(r*STRIDE+c*4)-0x0fff0000;bgr[v]=(bgr[v]||0)+1;const f=tl[r*W+c];bgf[f]=(bgf[f]||0)+1;}
console.log('background runtime value census:',JSON.stringify(bgr));
console.log('background file    value census:',JSON.stringify(bgf));
