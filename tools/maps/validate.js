// Validate the L2_maps.dat container structure. Usage: node validate.js [path]
const fs=require('fs');
const PATH=process.argv[2]||'F:/games/Lords of the Realm II/L2_maps.dat';
const REC=32961, PLANE=4096, NPLANE=6, TAIL_W=65, TAIL_H=129, TAIL=TAIL_W*TAIL_H;
const b=fs.readFileSync(PATH);
console.log('file:',PATH);
console.log('size:',b.length);
console.log('record size:',REC,'= 6*'+PLANE+' + '+TAIL_W+'*'+TAIL_H+' =',NPLANE*PLANE+TAIL);
if(b.length%REC){console.log('FAIL: not a multiple of',REC);process.exit(1);}
const N=b.length/REC;
console.log('slots:',N,' (arithmetic closes exactly)');
const P=(r,p)=>b.slice(r*REC+p*PLANE,r*REC+(p+1)*PLANE);
const T=r=>b.slice(r*REC+NPLANE*PLANE,(r+1)*REC);
let used=[],empty=[];
for(let r=0;r<N;r++){
  const p5=P(r,5); let cty=new Set();
  for(const v of p5) if(v>0&&v<=16) cty.add(v);
  // a slot is "used" if any of the six planes is non-constant
  let nonconst=false;
  for(let p=0;p<NPLANE;p++){const d=P(r,p);for(let i=1;i<PLANE;i++) if(d[i]!==d[0]){nonconst=true;break;} if(nonconst)break;}
  (nonconst?used:empty).push(r);
  if(nonconst){
    const p0=P(r,0);let b40=0,b80=0,b20=0;
    for(let i=0;i<PLANE;i++){if(p0[i]&0x40)b40++;if(p0[i]&0x80)b80++;if(p0[i]&0x20)b20++;}
    console.log(' slot',String(r).padStart(2),'USED  counties='+String(cty.size).padStart(2),
      ' castle(0x40) tiles='+String(b40).padStart(3)+(b40===4*cty.size?' (=4x counties OK)':' (MISMATCH)'),
      ' settle(0x80)='+String(b80).padStart(3),' dwell(0x20)='+String(b20).padStart(3));
  }
}
console.log('\nUSED slots ('+used.length+'):',used.join(','));
console.log('EMPTY slots ('+empty.length+'):',empty.join(','));
// empty slot fingerprints
const seen={};
for(const r of empty){const k=[...Array(NPLANE).keys()].map(p=>P(r,p)[0]).join(',')+' | tail:'+[...new Set(T(r))].join(',');
  (seen[k]=seen[k]||[]).push(r);}
console.log('\nempty-slot templates:');
for(const [k,v] of Object.entries(seen)) console.log('  planes const ['+k+']  slots '+v[0]+'..'+v[v.length-1]+' (n='+v.length+')');
// tail alphabet
const tv=new Set(); for(const r of used) for(const v of T(r)) tv.add(v);
console.log('\ntail (65x129) value alphabet over used slots:',[...tv].sort((a,c)=>a-c).map(v=>'0x'+v.toString(16)).join(','));
// plane alphabets
for(let p=0;p<NPLANE;p++){const s=new Set();for(const r of used) for(const v of P(r,p)) s.add(v);
  const a=[...s].sort((x,y)=>x-y);
  console.log('plane',p,'alphabet n='+a.length, a.length<=20?('['+a.map(v=>'0x'+v.toString(16)).join(',')+']'):('0x'+a[0].toString(16)+'..0x'+a[a.length-1].toString(16)));}
// invariant: plane0 bit 0x04 <=> county==0
let tot=0,ag=0;
for(const r of used){const p0=P(r,0),p5=P(r,5);for(let i=0;i<PLANE;i++){tot++;if(((p0[i]&4)!==0)===(p5[i]===0))ag++;}}
console.log('\nINVARIANT (plane0 & 0x04) <=> (plane5 == 0):',(100*ag/tot).toFixed(4)+'% over '+tot+' tiles');
// invariant: 0x40 tiles form complete 2x2 blocks
let blocks=0,left=0;
for(const r of used){const p0=P(r,0);const set=new Set();for(let i=0;i<PLANE;i++)if(p0[i]&0x40)set.add(i);
  const seen2=new Set();
  for(const i of [...set].sort((a,c)=>a-c)){if(seen2.has(i))continue;const x=i%64,y=(i/64)|0;
    if(x<63&&y<63&&set.has(i+1)&&set.has(i+64)&&set.has(i+65)){[i,i+1,i+64,i+65].forEach(k=>seen2.add(k));blocks++;}}
  left+=set.size-seen2.size;}
console.log('INVARIANT 0x40 tiles form 2x2 blocks: blocks='+blocks+' leftover tiles='+left);
