const fs=require('fs');
const p='F:/games/Lords of the Realm II/USER.SKR';const b=fs.readFileSync(p);
console.log('size',b.length,'expected',20*2*44+20*183+328+20*6400, '= 1760+3660+328+128000');
console.log('blocks: army',20*2*44,'text',20*183,'terrain',328+20*6400,'sum',20*2*44+20*183+328+20*6400);
console.log('0x152C =',0x152C,' 0x1674 =',0x1674,' 0x1900 =',0x1900,' 0x20A74 =',0x20A74,' 0xB7 =',0xB7,' 0x6E0 =',0x6E0,' 0xE4C =',0xE4C,' 0x1F548 =',0x1F548);
// pad
let nz=0;for(let i=0x152C;i<0x1674;i++)if(b[i])nz++;console.log('328-byte pad at',0x152C,'nonzero bytes:',nz);
// armies
const arm=[];for(let i=0;i<40;i++){const a=[];for(let j=0;j<11;j++)a.push(b.readUInt32LE(i*44+j*4));arm.push(a);}
console.log('map0 attacker',arm[0],'\nmap0 defender',arm[1]);
const def=[50,0,0,50,0,50,0,0,0,0,0];
let defaults=0;for(let i=2;i<40;i++)if(JSON.stringify(arm[i])===JSON.stringify(def))defaults++;
console.log('records 2..39 equal to the default template:',defaults,'/38');
let slots710=0;for(const a of arm)for(let j=7;j<11;j++)if(a[j])slots710++;
console.log('non-zero values in fields 7-10 across all 40 records:',slots710);
// text
for(let m=0;m<2;m++){const o=0x6E0+m*183;
 const f=[b.subarray(o,o+13),b.subarray(o+13,o+13+29),b.subarray(o+42,o+42+141)];
 console.log('text',m,f.map(x=>JSON.stringify(x.toString('latin1').replace(/\0/g,'.'))).join(' | '));}
let termOK=0;for(let m=0;m<20;m++){const o=0x6E0+m*183;
 for(const [s,l] of [[0,13],[13,29],[42,141]]) if(b.subarray(o+s,o+s+l).includes(0))termOK++;}
console.log('text fields NUL-terminated inside slot:',termOK+'/60');
// terrain
const lay=m=>b.subarray(0x1674+m*6400,0x1674+(m+1)*6400);
const blank=Buffer.alloc(6400);blank[20*80+40]=0x04;blank[60*80+40]=0x0F;
let ident=[];for(let m=0;m<20;m++)if(Buffer.compare(lay(m),blank)===0)ident.push(m);
console.log('maps byte-identical to editor blank template (x=40,y=20)=0x04,(x=40,y=60)=0x0F:',ident.length,ident.join(','));
const blankT=Buffer.alloc(6400);blankT[40*80+20]=0x04;blankT[40*80+60]=0x0F; // transposed reading
let identT=[];for(let m=0;m<20;m++)if(Buffer.compare(lay(m),blankT)===0)identT.push(m);
console.log('  (transposed reading):',identT.length);
const alpha=new Set();for(let m=0;m<20;m++)for(const v of lay(m))alpha.add(v);
console.log('terrain alphabet:',[...alpha].sort((a,c)=>a-c).map(v=>'0x'+v.toString(16).padStart(2,'0')).join(','));
let markOK=0;for(let m=0;m<20;m++){let a=0,f=0;for(const v of lay(m)){if(v===0x04)a++;if(v===0x0F)f++;}if(a===1&&f===1)markOK++;}
console.log('maps with exactly one 0x04 and one 0x0F:',markOK+'/20');
// bridges in map 0
const L=lay(0);const at=(x,y)=>L[y*80+x];
const p12=[],p10=[];for(let y=0;y<80;y++)for(let x=0;x<80;x++){if(at(x,y)===0x12)p12.push([x,y]);if(at(x,y)===0x10)p10.push([x,y]);}
console.log('map0 0x12 cells:',p12.length,'0x10 cells:',p10.length);
const xs=p12.map(c=>c[0]),ys=p12.map(c=>c[1]);
console.log('  0x12 x range',Math.min(...xs),'-',Math.max(...xs),'y range',Math.min(...ys),'-',Math.max(...ys));
const xs2=p10.map(c=>c[0]),ys2=p10.map(c=>c[1]);
console.log('  0x10 x range',Math.min(...xs2),'-',Math.max(...xs2),'y range',Math.min(...ys2),'-',Math.max(...ys2));
// L2MAP.INF
const inf=fs.readFileSync('F:/games/Lords of the Realm II/L2MAP.INF');
console.log('L2MAP.INF size',inf.length,'u32[0..2]',inf.readUInt32LE(0),inf.readUInt32LE(4),inf.readUInt32LE(8),
 'w',inf.readUInt32LE(0x18),'h',inf.readUInt32LE(0x1C),'name',inf.subarray(0xC8,0xC8+14).toString('latin1').replace(/\0.*/,''));
