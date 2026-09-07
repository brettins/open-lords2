const fs=require('fs');
function load(p){const b=fs.readFileSync(p);const off=g=>b[8+g*4]|(b[9+g*4]<<8)|(b[10+g*4]<<16);const N=(off(1)-8)/4;
 const S=g=>{const s=off(g),e=(g+1<N)?off(g+1):b.length;if(e<=s)return[];const x=b.subarray(s,e).toString('latin1').split('\0');x.pop();return x;};return{S,N,off,b};}
for(const p of ['F:/games/Lords of the Realm II/L2.eng','F:/games/LORDS2/L2.ENG']){
 const L=load(p);
 for(const g of [40,41,42]){const x=L.S(g);console.log(p.slice(-12),'group',g,'count',x.length,JSON.stringify(x.slice(0,32)));}
 console.log('  group 41 idx 27..29:',JSON.stringify(L.S(41).slice(27,30)));
 const all=L.b.toString('latin1');
 for(const s of ['My map','My battle map','A short description'])console.log('  contains',JSON.stringify(s)+':',all.includes(s));
 console.log('  empty group ids:',[...Array(L.N).keys()].slice(1).filter(g=>L.S(g).length===0).join(','));
}
